use anyhow::Result;
use std::sync::Arc;
use tracing::info;

mod api;
mod config;
mod crypto;
mod error;
mod integrations;
mod pipeline;
mod store;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let config = config::load()?;

    // Initialize logging
    config::init_logging(&config);

    info!(
        environment = %config.environment,
        display_name = %config.display_name,
        "Starting CRISPR"
    );

    // Connect to database
    let db = store::postgres::connect(&config.database).await?;

    // Run migrations
    info!("Running database migrations");
    sqlx::migrate!("./migrations").run(&db).await?;
    info!("Migrations complete");

    // Initialize storage
    let storage = store::object::init(&config.storage, db.clone()).await?;
    info!(mode = %config.storage.mode, "Storage initialized");

    // Initialize integrations
    let github = integrations::github::init(&config.github, &db).await?;
    let slack = integrations::slack::init(&config.slack, &db).await?;
    let llm = integrations::llm::init(&config.llm, &db).await?;

    // Build application state
    let state = api::AppState {
        config: config.clone(),
        db,
        storage,
        github,
        slack,
        llm,
    };

    // Start background worker
    let worker_state = state.clone();
    tokio::spawn(async move {
        worker::run(worker_state).await;
    });

    // Start gRPC server (OTLP) in background
    let grpc_state = state.clone();
    let grpc_addr = format!("{}:{}", config.server.host, config.server.grpc_port);
    tokio::spawn(async move {
        if let Err(e) = api::ingest::serve_grpc(grpc_state, &grpc_addr).await {
            tracing::error!(error = %e, "gRPC server error");
        }
    });

    // Start HTTP server
    let http_addr = format!("{}:{}", config.server.host, config.server.http_port);
    info!(addr = %http_addr, "HTTP server starting");

    let app = api::router::build(state);
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;

    info!(
        http = %http_addr,
        grpc = %format!("{}:{}", config.server.host, config.server.grpc_port),
        "CRISPR ready"
    );

    axum::serve(listener, app).await?;

    Ok(())
}
