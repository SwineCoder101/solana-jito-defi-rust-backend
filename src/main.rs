mod config;
mod model;
mod price_stream;
mod simulation;
mod web;

use axum::{Router, routing::get};
use config::PriceFeed;
use model::{AppData, AppState, SwapDirection, WalletState};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state: AppState = Arc::new(Mutex::new(AppData {
        latest_price: None,
        wallet: WalletState {
            sol: 5.0,
            usdc: 0.0,
        },
        history: Vec::new(),
        next_swap: SwapDirection::ToUsdc,
    }));

    tokio::spawn(price_stream::run(state.clone(), PriceFeed::SolUsd));

    let app = Router::new().route("/", get(web::index)).with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("failed to bind server port");

    axum::serve(listener, app).await?;
    Ok(())
}
