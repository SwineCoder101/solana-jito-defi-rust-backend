use crate::config::{DEFAULT_SLIPPAGE_BPS, JUPITER_BASE_URL, JUPITER_USER_PUBKEY};
use anyhow::{Result, anyhow};
use reqwest::Client;
use serde_json::Value;

#[derive(Clone)]
pub struct JupiterClient {
    http: Client,
}

impl JupiterClient {
    pub fn new() -> Result<Self> {
        let http = Client::builder().build()?;
        Ok(Self { http })
    }

    pub async fn quote_exact_in(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
    ) -> Result<JupiterQuote> {
        let url = format!("{}/quote", JUPITER_BASE_URL);
        let response = self
            .http
            .get(url)
            .query(&[
                ("inputMint", input_mint),
                ("outputMint", output_mint),
                ("amount", &amount.to_string()),
                ("swapMode", "ExactIn"),
                ("slippageBps", &DEFAULT_SLIPPAGE_BPS.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?;

        let raw: Value = response.json().await?;
        let in_amount = parse_amount(&raw, "inAmount")?;
        let out_amount = parse_amount(&raw, "outAmount")?;
        let price_impact_pct = raw
            .get("priceImpactPct")
            .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()));

        Ok(JupiterQuote {
            raw,
            in_amount,
            out_amount,
            price_impact_pct,
        })
    }

    pub async fn simulate_swap(
        &self,
        quote: &JupiterQuote,
        wrap_and_unwrap_sol: bool,
    ) -> Result<JupiterSimulation> {
        let url = format!("{}/swap", JUPITER_BASE_URL);
        let body = serde_json::json!({
            "quoteResponse": quote.raw,
            "userPublicKey": JUPITER_USER_PUBKEY,
            "wrapAndUnwrapSol": wrap_and_unwrap_sol,
            "simulate": true,
            "asLegacyTransaction": true,
        });

        let response = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let raw: Value = response.json().await?;

        if raw
            .get("simulationError")
            .and_then(|value| value.as_str())
            .is_some()
        {
            return Err(anyhow!(
                "jupiter simulation returned error: {}",
                raw["simulationError"]
            ));
        }

        let lamports_fee = raw
            .get("lamportsFee")
            .and_then(|value| value.as_u64())
            .unwrap_or(5000);
        let prioritization_fee = raw
            .get("prioritizationFeeLamports")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let gas_lamports = lamports_fee + prioritization_fee;

        Ok(JupiterSimulation {
            gas_lamports: Some(gas_lamports),
        })
    }
}

#[derive(Debug, Clone)]
pub struct JupiterQuote {
    pub raw: Value,
    pub in_amount: u64,
    pub out_amount: u64,
    pub price_impact_pct: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct JupiterSimulation {
    pub gas_lamports: Option<u64>,
}

fn parse_amount(value: &Value, key: &str) -> Result<u64> {
    let raw_value = value
        .get(key)
        .ok_or_else(|| anyhow!("jupiter response missing field `{}`", key))?;

    if let Some(number) = raw_value.as_u64() {
        return Ok(number);
    }

    if let Some(text) = raw_value.as_str() {
        return text
            .parse::<u64>()
            .map_err(|err| anyhow!("failed to parse `{}`: {err}", key));
    }

    Err(anyhow!("unexpected type for `{}` in jupiter response", key))
}
