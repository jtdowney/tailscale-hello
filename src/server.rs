use std::{net::SocketAddr, sync::Arc};

use askama::Template;
use axum::{
    extract::{ConnectInfo, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use hyper::{header::CONTENT_TYPE, StatusCode, Uri};
use rust_embed::RustEmbed;
use tailscale_localapi::{LocalApiClient, Status, Whois};

#[derive(RustEmbed)]
#[folder = "static/"]
struct Asset;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    information: Option<(Whois, Status)>,
}

struct StaticFile<T>(T);

impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let StaticFile(path) = self;
        let path = path.into();

        match Asset::get(path.as_str()) {
            Some(content) => {
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                ([(CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}

async fn index_handler<T>(
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    State(localapi): State<Arc<tailscale_localapi::LocalApi<T>>>,
) -> impl IntoResponse
where
    T: LocalApiClient + Send + Sync + 'static,
{
    let whois = localapi.whois(address).await.ok();
    let status = localapi.status().await.ok();
    let information = whois.zip(status);
    IndexTemplate { information }
}

async fn fallback_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();
    StaticFile(path)
}

pub fn create_router<T>(state: Arc<tailscale_localapi::LocalApi<T>>) -> Router
where
    T: LocalApiClient + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(index_handler))
        .fallback_service(get(fallback_handler))
        .with_state(state)
}
