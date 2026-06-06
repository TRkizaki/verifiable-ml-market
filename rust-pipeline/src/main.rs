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
    println!("  POST /api/ensemble/predict    - Make ensemble prediction");
    println!("  POST /api/ensemble/optimize   - Optimise ensemble weights");
    println!("  POST /api/provenance/hash     - Hash input data");
    println!("  POST /api/provenance/commit   - Create prediction commitment");
    println!("  POST /api/provenance/verify   - Verify commitment");
    println!("  POST /api/evaluate            - Evaluate predictions");

    #[cfg(feature = "ingestion")]
    {
        println!();
        println!("Ingestion endpoints:");
        println!("  POST /api/ingest/prices         - Fetch price history & generate features");
    }

    #[cfg(feature = "substrate")]
    {
        println!();
        println!("Chain endpoints (substrate):");
        println!("  POST /api/chain/register-model      - Register ML model");
        println!("  POST /api/chain/create-market        - Create prediction market");
        println!("  POST /api/chain/submit-commitment    - Submit commitment on-chain");
        println!("  POST /api/chain/reveal               - Reveal prediction");
        println!("  POST /api/chain/ground-truth         - Submit ground truth");
        println!("  POST /api/chain/stake                - Stake on prediction");
        println!("  POST /api/chain/settle               - Settle market");
        println!("  GET  /api/chain/market/:id           - Query market state");
    }

    println!();

    #[cfg(feature = "substrate")]
    let substrate_url = std::env::var("SUBSTRATE_URL").ok();

    vml_pipeline::api::start_server(
        &host,
        port,
        #[cfg(feature = "substrate")]
        substrate_url.as_deref(),
    )
    .await
}
