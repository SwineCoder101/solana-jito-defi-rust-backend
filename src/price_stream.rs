use crate::config::PriceFeed;
use crate::jupiter::JupiterClient;
use crate::model::{AppState, HermesResponse, ParsedPriceData, PriceInfo};
use crate::simulation::apply_price_update;
use anyhow::Result;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub async fn run(state: AppState, jupiter: Option<Arc<JupiterClient>>, feed: PriceFeed) {
    loop {
        if let Err(err) = stream_prices(state.clone(), jupiter.clone(), feed).await {
            eprintln!("price stream error: {err:?}");
            sleep(Duration::from_secs(3)).await;
        }
    }
}

async fn stream_prices(
    state: AppState,
    jupiter: Option<Arc<JupiterClient>>,
    feed: PriceFeed,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get(feed.stream_url())
        .header("Accept", "text/event-stream")
        .send()
        .await?;

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.extend_from_slice(&chunk);

        while let Some(position) = buffer.iter().position(|&byte| byte == b'\n') {
            let line = buffer.drain(..=position).collect::<Vec<u8>>();
            let line = String::from_utf8_lossy(&line).trim().to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(payload) = line.strip_prefix("data:") {
                if let Err(err) =
                    handle_payload(payload.trim(), state.clone(), jupiter.clone()).await
                {
                    eprintln!("failed to handle price payload: {err:?}");
                }
            }
        }
    }

    Ok(())
}

async fn handle_payload(
    payload: &str,
    state: AppState,
    jupiter: Option<Arc<JupiterClient>>,
) -> Result<()> {
    let parsed: HermesResponse = serde_json::from_str(payload)?;

    for price in parsed.parsed {
        if let Some(price_info) = to_price_info(price.price) {
            apply_price_update(&state, price_info, jupiter.clone()).await;
        }
    }

    Ok(())
}

fn to_price_info(price: ParsedPriceData) -> Option<PriceInfo> {
    let raw_price = price.price.parse::<i128>().ok()?;
    let scale = 10f64.powi(price.expo);
    let value = (raw_price as f64) * scale;

    Some(PriceInfo {
        value,
        publish_time: price.publish_time,
    })
}
