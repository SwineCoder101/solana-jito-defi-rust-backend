const HERMES_STREAM_BASE: &str = "https://hermes.pyth.network/v2/updates/price/stream";
pub const SOL_TO_USDC_SWAP_AMOUNT: f64 = 0.1;
pub const USDC_TO_SOL_SWAP_AMOUNT: f64 = 20.0;
pub const MAX_HISTORY_ENTRIES: usize = 200;

#[derive(Clone, Copy, Debug)]
pub enum PriceFeed {
    SolUsd,
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
