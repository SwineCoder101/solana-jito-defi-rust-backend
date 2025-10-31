use crate::config::{MAX_HISTORY_ENTRIES, SOL_TO_USDC_SWAP_AMOUNT, USDC_TO_SOL_SWAP_AMOUNT};
use crate::model::{AppData, AppState, PriceInfo, SwapDirection, SwapRecord};
use chrono::{DateTime, Utc};

pub async fn apply_price_update(state: &AppState, price_info: PriceInfo) {
    let mut data = state.lock().await;
    data.latest_price = Some(price_info.clone());

    if let Some(record) = perform_swap(&mut data, &price_info) {
        data.history.push(record);
        if data.history.len() > MAX_HISTORY_ENTRIES {
            let excess = data.history.len() - MAX_HISTORY_ENTRIES;
            data.history.drain(0..excess);
        }
    }
}

pub fn publish_time_to_string(ts: Option<i64>) -> String {
    ts.and_then(|value| DateTime::<Utc>::from_timestamp(value, 0).map(format_publish_time))
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_publish_time(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn perform_swap(data: &mut AppData, price: &PriceInfo) -> Option<SwapRecord> {
    if price.value <= 0.0 {
        return None;
    }

    match data.next_swap {
        SwapDirection::ToUsdc => swap_to_usdc(data, price),
        SwapDirection::ToSol => swap_to_sol(data, price),
    }
}

fn swap_to_usdc(data: &mut AppData, price: &PriceInfo) -> Option<SwapRecord> {
    if data.wallet.sol <= 0.0 {
        data.next_swap = SwapDirection::ToSol;
        return None;
    }

    let amount_sol = data.wallet.sol.min(SOL_TO_USDC_SWAP_AMOUNT);
    let received_usdc = amount_sol * price.value;

    data.wallet.sol -= amount_sol;
    data.wallet.usdc += received_usdc;
    data.next_swap = SwapDirection::ToSol;

    Some(SwapRecord {
        timestamp: current_timestamp(),
        direction: "SOL → USDC".to_string(),
        price: price.value,
        amount_in: amount_sol,
        amount_out: received_usdc,
    })
}

fn swap_to_sol(data: &mut AppData, price: &PriceInfo) -> Option<SwapRecord> {
    if data.wallet.usdc <= 0.0 {
        data.next_swap = SwapDirection::ToUsdc;
        return None;
    }

    let amount_usdc = data.wallet.usdc.min(USDC_TO_SOL_SWAP_AMOUNT);
    if amount_usdc <= 0.0 {
        data.next_swap = SwapDirection::ToUsdc;
        return None;
    }

    let received_sol = amount_usdc / price.value;

    data.wallet.usdc -= amount_usdc;
    data.wallet.sol += received_sol;
    data.next_swap = SwapDirection::ToUsdc;

    Some(SwapRecord {
        timestamp: current_timestamp(),
        direction: "USDC → SOL".to_string(),
        price: price.value,
        amount_in: amount_usdc,
        amount_out: received_sol,
    })
}

fn current_timestamp() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string()
}
