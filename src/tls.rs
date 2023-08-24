use std::{
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use rustls::{server::ResolvesServerCert, sign::CertifiedKey};
use tailscale_localapi::{Certificate, LocalApi, LocalApiClient, PrivateKey};
use tracing::{debug, error, instrument, trace};

const CERTIFICATE_LIFETIME: Duration = Duration::from_secs(60 * 60 * 24);

struct CachedCertificate {
    cert_and_key: CertifiedKey,
    last_update: Instant,
}

struct TailscaleCertResolver<T: LocalApiClient + Clone> {
    domain: String,
    localapi: Arc<LocalApi<T>>,
    cached_certificate: RwLock<Option<CachedCertificate>>,
}

pub fn create_config<T, S>(
    localapi: Arc<LocalApi<T>>,
    domain: S,
) -> anyhow::Result<Arc<rustls::ServerConfig>>
where
    T: LocalApiClient + Send + Sync + 'static,
    S: Into<String>,
{
    let domain = domain.into();
    let cert_resolver = Arc::new(TailscaleCertResolver {
        domain,
        localapi,
        cached_certificate: RwLock::new(None),
    });

    let mut config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_cert_resolver(cert_resolver);
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

#[instrument(skip(localapi))]
fn request_certificate<T>(localapi: Arc<LocalApi<T>>, domain: &str) -> anyhow::Result<CertifiedKey>
where
    T: LocalApiClient + Send + Sync + 'static,
{
    debug!("requesting a certificate from tailscale");

    let domain = domain.to_string();
    let (PrivateKey(key), certs) = thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()?;
        rt.block_on(localapi.certificate_pair(&domain))
    })
    .join()
    .map_err(|e| anyhow!("unable to fetch certificate: {:?}", e))??;

    let certs = certs
        .into_iter()
        .map(|Certificate(data)| rustls::Certificate(data))
        .collect();
    let key = rustls::PrivateKey(key);
    let key = rustls::sign::any_supported_type(&key)?;
    Ok(CertifiedKey::new(certs, key))
}

impl<T> ResolvesServerCert for TailscaleCertResolver<T>
where
    T: LocalApiClient + Clone + Send + Sync + 'static,
{
    #[instrument(skip_all, fields(domain = self.domain))]
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello.server_name()?;
        if sni != self.domain {
            debug!(domain = self.domain, sni, "domain and sni mismatch");
            return None;
        }

        {
            let cached_certificate = self.cached_certificate.read().unwrap();
            if let Some(cached_certificate) = cached_certificate.as_ref() {
                if cached_certificate.last_update.elapsed() < CERTIFICATE_LIFETIME {
                    trace!("cache hit");
                    return Some(Arc::new(cached_certificate.cert_and_key.clone()));
                }
            }
        }

        {
            trace!("cache miss");
            let mut cached_certificate = self.cached_certificate.write().unwrap();
            match request_certificate(self.localapi.clone(), &self.domain) {
                Ok(cert_and_key) => {
                    *cached_certificate = Some(CachedCertificate {
                        cert_and_key: cert_and_key.clone(),
                        last_update: Instant::now(),
                    });
                    Some(Arc::new(cert_and_key))
                }
                Err(e) => {
                    error!("unable to fetch certificate: {:?}", e);
                    None
                }
            }
        }
    }
}
