use std::env;

const HERMES_STREAM_BASE: &str = "https://hermes.pyth.network/v2/updates/price/stream";
pub const MAX_HISTORY_ENTRIES: usize = 200;
pub const JUPITER_BASE_URL: &str = "https://quote-api.jup.ag/v6";
pub const JUPITER_USER_PUBKEY: &str = "11111111111111111111111111111111";
pub const DEFAULT_SLIPPAGE_BPS: u16 = 50;

pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const SOL_DECIMALS: u8 = 9;
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeMqejdX3tqZZzcny9qE8P4AQV7B7";
pub const USDC_DECIMALS: u8 = 6;

#[derive(Clone, Copy, Debug)]
pub enum PriceFeed {
    SolUsd,
}

pub fn jupiter_enabled() -> bool {
    match env::var("ENABLE_JUPITER") {
        Ok(value) => {
            let normalized = value.trim().to_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

impl PriceFeed {
    pub fn id(self) -> &'static str {
        match self {
            PriceFeed::SolUsd => {
                "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d"
            }
        }
    }

    pub fn stream_url(self) -> String {
        format!("{HERMES_STREAM_BASE}?ids[]={}", self.id())
    }
}
