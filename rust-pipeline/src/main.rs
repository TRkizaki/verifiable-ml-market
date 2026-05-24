use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    println!("Verifiable ML Pipeline Server");
    println!("  Version: {}", env!("CARGO_PKG_VERSION"));
    println!("  Server:  http://{}:{}", host, port);
    println!();
    println!("Endpoints:");
    println!("  GET  /health                  - Health check");
    println!("  POST /api/features/rolling    - Compute rolling features");
    println!("  POST /api/features/lag        - Compute lag features");
    println!("  POST /api/features/growth     - Compute growth rates");
    println!("  POST /api/features/full       - Generate all features");
    println!("  POST /api/ensemble/predict    - Make ensemble prediction");
    println!("  POST /api/ensemble/optimize   - Optimise ensemble weights");
    println!("  POST /api/provenance/hash     - Hash input data");
    println!("  POST /api/provenance/commit   - Create prediction commitment");
    println!("  POST /api/provenance/verify   - Verify commitment");
    println!("  POST /api/evaluate            - Evaluate predictions");
    println!();

    vml_pipeline::api::start_server(&host, port).await
}
