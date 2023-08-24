mod server;
mod tls;

use std::{convert::Infallible, env, net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{handler::HandlerWithoutStateExt, response::Redirect};
use axum_server::tls_rustls::RustlsConfig;
use futures::FutureExt;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::server::create_router;

fn setup_tracing() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing();

    let port: u16 = env::var("HTTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let tls_port: u16 = env::var("HTTPS_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3443);
    let localapi_socket =
        env::var("TS_SOCKET").unwrap_or_else(|_| "/var/run/tailscale/tailscaled.sock".into());

    let localapi = Arc::new(tailscale_localapi::LocalApi::new_with_socket_path(
        localapi_socket,
    ));

    let status = localapi.status().await?;
    let domain = status
        .cert_domains
        .first()
        .context("unable to get cert domain")?
        .to_owned();
    info!(domain, "using domain for certificates");

    let tls_config = tls::create_config(localapi.clone(), domain.clone())?;
    let servers = status
        .tailscale_ips
        .iter()
        .flat_map(|&ip| {
            let addr = SocketAddr::from((ip, port));
            let tls_addr = SocketAddr::from((ip, tls_port));

            info!("listening on http://{addr}");
            info!("listening on https://{tls_addr}");

            let domain = domain.clone();
            let redirect = move || async move {
                let mut redirect = format!("https://{domain}");
                if tls_port != 443 {
                    redirect = format!("{redirect}:{}", tls_port);
                }

                Ok::<_, Infallible>(Redirect::permanent(&redirect))
            };

            let server = axum_server::bind(addr).serve(redirect.into_make_service());

            let router = create_router(localapi.clone());
            let tls_server =
                axum_server::bind_rustls(tls_addr, RustlsConfig::from_config(tls_config.clone()))
                    .serve(router.into_make_service_with_connect_info::<SocketAddr>());

            [server.boxed(), tls_server.boxed()]
        })
        .collect::<Vec<_>>();

    futures::future::join_all(servers).await;

    Ok(())
}
