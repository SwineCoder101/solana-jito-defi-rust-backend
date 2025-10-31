use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct PriceInfo {
    pub value: f64,
    pub publish_time: Option<i64>,
}

#[derive(Debug)]
pub struct WalletState {
    pub sol: f64,
    pub usdc: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum SwapDirection {
    ToUsdc,
    ToSol,
}

#[derive(Debug)]
pub struct SwapRecord {
    pub timestamp: String,
    pub direction: String,
    pub price: f64,
    pub amount_in: f64,
    pub amount_out: f64,
}

#[derive(Debug)]
pub struct AppData {
    pub latest_price: Option<PriceInfo>,
    pub wallet: WalletState,
    pub history: Vec<SwapRecord>,
    pub next_swap: SwapDirection,
}

pub type AppState = Arc<Mutex<AppData>>;

#[derive(Debug, Deserialize)]
pub struct HermesResponse {
    #[serde(default)]
    pub parsed: Vec<ParsedPrice>,
}

#[derive(Debug, Deserialize)]
pub struct ParsedPrice {
    pub price: ParsedPriceData,
}

#[derive(Debug, Deserialize)]
pub struct ParsedPriceData {
    pub price: String,
    pub expo: i32,
    #[serde(default)]
    pub publish_time: Option<i64>,
}
