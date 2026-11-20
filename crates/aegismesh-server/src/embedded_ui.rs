use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../frontend/dist"]
struct FrontendAssets;

pub async fn static_handler(uri: Uri) -> Response {
    let raw_path = uri.path().trim_start_matches('/');
    let path = if raw_path.is_empty() {
        "index.html"
    } else {
        raw_path
    };

    if let Some(content) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        let cache = if path.starts_with("assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache, no-store, must-revalidate"
        };
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime.as_ref().to_string()),
                (header::CACHE_CONTROL, cache.to_string()),
            ],
            content.data.into_owned(),
        )
            .into_response();
    }

    // SPA fallback for client routes
    if !path.starts_with("api/") && !path.starts_with("v1/") && !path.contains('.') {
        if let Some(index) = FrontendAssets::get("index.html") {
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
                    (header::CACHE_CONTROL, "no-cache".to_string()),
                ],
                index.data.into_owned(),
            )
                .into_response();
        }
    }

    (StatusCode::NOT_FOUND, "404 Not Found").into_response()
}
