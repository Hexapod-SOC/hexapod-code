use axum::Router;
use std::net::SocketAddr;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
};

pub async fn run_panel(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Starting web panel on port {}", port);

    // Serve static files from the static directory
    let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("static");
    
    tracing::info!("Serving static files from: {:?}", static_dir);

    let app = Router::new()
        .nest_service("/", ServeDir::new(static_dir))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Web panel listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
