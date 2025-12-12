use axum::{
    http::header,
    response::{Html, IntoResponse, Response},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

// Embed static files at compile time
const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLE_CSS: &str = include_str!("../static/style.css");
const FAVICON_SVG: &str = include_str!("../static/favicon.svg");
const MAP_HTML: &str = include_str!("../static/map.html");
const MAP_JS: &str = include_str!("../static/map.js");
const MAP_CSS: &str = include_str!("../static/map.css");

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn serve_js() -> Response {
    ([(header::CONTENT_TYPE, "application/javascript")], APP_JS).into_response()
}

async fn serve_css() -> Response {
    ([(header::CONTENT_TYPE, "text/css")], STYLE_CSS).into_response()
}

async fn serve_favicon() -> Response {
    ([(header::CONTENT_TYPE, "image/svg+xml")], FAVICON_SVG).into_response()
}

async fn serve_map_html() -> Html<&'static str> {
    Html(MAP_HTML)
}

async fn serve_map_js() -> Response {
    ([(header::CONTENT_TYPE, "application/javascript")], MAP_JS).into_response()
}

async fn serve_map_css() -> Response {
    ([(header::CONTENT_TYPE, "text/css")], MAP_CSS).into_response()
}

pub async fn run_panel(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Starting web panel on port {}", port);

    let app = Router::new()
        .route("/", axum::routing::get(serve_index))
        .route("/index.html", axum::routing::get(serve_index))
        .route("/app.js", axum::routing::get(serve_js))
        .route("/style.css", axum::routing::get(serve_css))
        .route("/favicon.svg", axum::routing::get(serve_favicon))
    .route("/map", axum::routing::get(serve_map_html))
    .route("/map.js", axum::routing::get(serve_map_js))
    .route("/map.css", axum::routing::get(serve_map_css))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Web panel listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
