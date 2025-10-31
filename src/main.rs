mod config;
mod jupiter;
mod model;
mod price_stream;
mod simulation;
mod web;

use axum::{Router, routing::get};
use config::{PriceFeed, jupiter_enabled};
use jupiter::JupiterClient;
use model::{AppData, AppState, StrategyData};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let jupiter = if jupiter_enabled() {
        match JupiterClient::new() {
            Ok(client) => {
                eprintln!("Jupiter integration enabled; using live quotes.");
                Some(Arc::new(client))
            }
            Err(err) => {
                eprintln!(
                    "Failed to initialize Jupiter client, falling back to local pricing: {err:?}"
                );
                None
            }
        }
    } else {
        eprintln!("Jupiter integration disabled; using local pricing for swaps.");
        None
    };
    let state: AppState = Arc::new(Mutex::new(AppData {
        latest_price: None,
        strategies: vec![
            StrategyData::alternating(),
            StrategyData::trend_follow(),
            StrategyData::range_trader(),
        ],
        history: Vec::new(),
    }));

    tokio::spawn(price_stream::run(
        state.clone(),
        jupiter.clone(),
        PriceFeed::SolUsd,
    ));

    let app = Router::new().route("/", get(web::index)).with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("failed to bind server port");

    axum::serve(listener, app).await?;
    Ok(())
}
