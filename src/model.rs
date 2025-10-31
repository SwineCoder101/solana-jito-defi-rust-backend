use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct PriceInfo {
    pub value: f64,
    pub publish_time: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct WalletState {
    pub sol: f64,
    pub usdc: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum Token {
    Sol,
    Usdc,
}

impl Token {
    pub fn symbol(self) -> &'static str {
        match self {
            Token::Sol => "SOL",
            Token::Usdc => "USDC",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SwapDirection {
    ToUsdc,
    ToSol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyId {
    Alternating,
    TrendFollow,
    RangeTrader,
}

impl StrategyId {
    pub fn label(self) -> &'static str {
        match self {
            StrategyId::Alternating => "Alternating",
            StrategyId::TrendFollow => "Trend Follow",
            StrategyId::RangeTrader => "Range Trader",
        }
    }
}

#[derive(Debug, Clone)]
pub enum StrategyState {
    Alternating { next_swap: SwapDirection },
    TrendFollow { last_price: Option<f64> },
    RangeTrader { last_price: Option<f64> },
}

#[derive(Debug, Clone)]
pub struct StrategyData {
    pub id: StrategyId,
    pub wallet: WalletState,
    pub state: StrategyState,
}

impl StrategyData {
    pub fn alternating() -> Self {
        Self {
            id: StrategyId::Alternating,
            wallet: WalletState {
                sol: 1.0,
                usdc: 0.0,
            },
            state: StrategyState::Alternating {
                next_swap: SwapDirection::ToUsdc,
            },
        }
    }

    pub fn trend_follow() -> Self {
        Self {
            id: StrategyId::TrendFollow,
            wallet: WalletState {
                sol: 1.0,
                usdc: 0.0,
            },
            state: StrategyState::TrendFollow { last_price: None },
        }
    }

    pub fn range_trader() -> Self {
        Self {
            id: StrategyId::RangeTrader,
            wallet: WalletState {
                sol: 1.0,
                usdc: 0.0,
            },
            state: StrategyState::RangeTrader { last_price: None },
        }
    }
}

#[derive(Debug)]
pub struct SwapRecord {
    pub timestamp: String,
    pub direction: String,
    pub price: f64,
    pub amount_in: f64,
    pub amount_out: f64,
    pub strategy: StrategyId,
    pub input_token: Token,
    pub output_token: Token,
    pub gas_lamports: Option<u64>,
    pub price_impact_pct: Option<f64>,
}

#[derive(Debug)]
pub struct AppData {
    pub latest_price: Option<PriceInfo>,
    pub strategies: Vec<StrategyData>,
    pub history: Vec<SwapRecord>,
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
