use crate::config::{MAX_HISTORY_ENTRIES, SOL_DECIMALS, SOL_MINT, USDC_DECIMALS, USDC_MINT};
use crate::jupiter::JupiterClient;
use crate::model::{
    AppState, PriceInfo, StrategyData, StrategyId, StrategyState, SwapDirection, SwapRecord, Token,
};
use anyhow::{Error, Result, anyhow};
use chrono::{DateTime, Utc};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

static JUPITER_WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

const TREND_THRESHOLD: f64 = 0.002;
const TREND_SELL_SOL_AMOUNT: f64 = 0.15;
const TREND_BUY_USDC_AMOUNT: f64 = 25.0;

const RANGE_THRESHOLD: f64 = 0.003;
const RANGE_SELL_SOL_AMOUNT: f64 = 0.1;
const RANGE_BUY_USDC_AMOUNT: f64 = 20.0;

const MIN_SOL_AMOUNT: f64 = 1e-6;
const MIN_USDC_AMOUNT: f64 = 0.01;

pub async fn apply_price_update(
    state: &AppState,
    price_info: PriceInfo,
    jupiter: Option<Arc<JupiterClient>>,
) {
    if price_info.value <= 0.0 {
        return;
    }

    let mut guard = state.lock().await;
    guard.latest_price = Some(price_info.clone());

    let mut pending = Vec::new();
    for (index, strategy) in guard.strategies.iter_mut().enumerate() {
        if let Some(action) = determine_action(index, strategy, &price_info) {
            pending.push(action);
        }
    }
    drop(guard);

    for action in pending {
        let execution = execute_action(jupiter.clone(), &price_info, &action.action).await;

        let mut guard = state.lock().await;
        if let Some(strategy) = guard.strategies.get_mut(action.strategy_index) {
            apply_wallet_updates(strategy, &action, &execution);

            let record = SwapRecord {
                timestamp: current_timestamp(),
                direction: action.action.direction_label().to_string(),
                price: price_info.value,
                amount_in: execution.amount_in,
                amount_out: execution.amount_out,
                strategy: action.strategy_id,
                input_token: execution.input_token,
                output_token: execution.output_token,
                gas_lamports: execution.gas_lamports,
                price_impact_pct: execution.price_impact_pct,
            };

            guard.history.push(record);
            if guard.history.len() > MAX_HISTORY_ENTRIES {
                let excess = guard.history.len() - MAX_HISTORY_ENTRIES;
                guard.history.drain(0..excess);
            }
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

#[derive(Debug)]
struct PendingAction {
    strategy_index: usize,
    strategy_id: StrategyId,
    action: SwapAction,
    post_update: Option<StrategyPostUpdate>,
}

#[derive(Debug)]
enum StrategyPostUpdate {
    AlternatingNext(SwapDirection),
}

#[derive(Debug)]
enum SwapAction {
    SolToUsdc { amount_sol: f64 },
    UsdcToSol { amount_usdc: f64 },
}

impl SwapAction {
    fn input_token(&self) -> Token {
        match self {
            SwapAction::SolToUsdc { .. } => Token::Sol,
            SwapAction::UsdcToSol { .. } => Token::Usdc,
        }
    }

    fn output_token(&self) -> Token {
        match self {
            SwapAction::SolToUsdc { .. } => Token::Usdc,
            SwapAction::UsdcToSol { .. } => Token::Sol,
        }
    }

    fn direction_label(&self) -> &'static str {
        match self {
            SwapAction::SolToUsdc { .. } => "SOL → USDC",
            SwapAction::UsdcToSol { .. } => "USDC → SOL",
        }
    }
}

struct SwapExecution {
    amount_in: f64,
    amount_out: f64,
    input_token: Token,
    output_token: Token,
    gas_lamports: Option<u64>,
    price_impact_pct: Option<f64>,
}

fn determine_action(
    index: usize,
    strategy: &mut StrategyData,
    price: &PriceInfo,
) -> Option<PendingAction> {
    let strategy_id = strategy.id;
    match &mut strategy.state {
        StrategyState::Alternating { next_swap } => match next_swap {
            SwapDirection::ToUsdc => {
                if strategy.wallet.sol <= MIN_SOL_AMOUNT {
                    return None;
                }
                Some(PendingAction {
                    strategy_index: index,
                    strategy_id,
                    action: SwapAction::SolToUsdc {
                        amount_sol: strategy.wallet.sol,
                    },
                    post_update: Some(StrategyPostUpdate::AlternatingNext(SwapDirection::ToSol)),
                })
            }
            SwapDirection::ToSol => {
                if strategy.wallet.usdc <= MIN_USDC_AMOUNT {
                    return None;
                }
                Some(PendingAction {
                    strategy_index: index,
                    strategy_id,
                    action: SwapAction::UsdcToSol {
                        amount_usdc: strategy.wallet.usdc,
                    },
                    post_update: Some(StrategyPostUpdate::AlternatingNext(SwapDirection::ToUsdc)),
                })
            }
        },
        StrategyState::TrendFollow { last_price } => {
            let previous = *last_price;
            *last_price = Some(price.value);

            let Some(prev) = previous else {
                return None;
            };

            let change = (price.value - prev) / prev;
            if change >= TREND_THRESHOLD && strategy.wallet.sol > MIN_SOL_AMOUNT {
                let amount = strategy
                    .wallet
                    .sol
                    .min(TREND_SELL_SOL_AMOUNT)
                    .max(MIN_SOL_AMOUNT);
                return Some(PendingAction {
                    strategy_index: index,
                    strategy_id,
                    action: SwapAction::SolToUsdc { amount_sol: amount },
                    post_update: None,
                });
            }

            if change <= -TREND_THRESHOLD && strategy.wallet.usdc > MIN_USDC_AMOUNT {
                let amount = strategy
                    .wallet
                    .usdc
                    .min(TREND_BUY_USDC_AMOUNT)
                    .max(MIN_USDC_AMOUNT);
                return Some(PendingAction {
                    strategy_index: index,
                    strategy_id,
                    action: SwapAction::UsdcToSol {
                        amount_usdc: amount,
                    },
                    post_update: None,
                });
            }

            None
        }
        StrategyState::RangeTrader { last_price } => {
            let previous = *last_price;
            *last_price = Some(price.value);

            let Some(prev) = previous else {
                return None;
            };

            if price.value >= prev * (1.0 + RANGE_THRESHOLD) && strategy.wallet.sol > MIN_SOL_AMOUNT
            {
                let amount = strategy
                    .wallet
                    .sol
                    .min(RANGE_SELL_SOL_AMOUNT)
                    .max(MIN_SOL_AMOUNT);
                return Some(PendingAction {
                    strategy_index: index,
                    strategy_id,
                    action: SwapAction::SolToUsdc { amount_sol: amount },
                    post_update: None,
                });
            }

            if price.value <= prev * (1.0 - RANGE_THRESHOLD)
                && strategy.wallet.usdc > MIN_USDC_AMOUNT
            {
                let amount = strategy
                    .wallet
                    .usdc
                    .min(RANGE_BUY_USDC_AMOUNT)
                    .max(MIN_USDC_AMOUNT);
                return Some(PendingAction {
                    strategy_index: index,
                    strategy_id,
                    action: SwapAction::UsdcToSol {
                        amount_usdc: amount,
                    },
                    post_update: None,
                });
            }

            None
        }
    }
}

async fn execute_action(
    jupiter: Option<Arc<JupiterClient>>,
    price: &PriceInfo,
    action: &SwapAction,
) -> SwapExecution {
    if let Some(client) = jupiter {
        match execute_with_jupiter(client, action).await {
            Ok(execution) => return execution,
            Err(err) => {
                log_jupiter_warning(&err);
            }
        }
    }

    execute_with_price(price, action)
}

async fn execute_with_jupiter(
    client: Arc<JupiterClient>,
    action: &SwapAction,
) -> Result<SwapExecution> {
    let input_token = action.input_token();
    let output_token = action.output_token();
    let (input_mint, input_decimals) = token_details(input_token);
    let (output_mint, output_decimals) = token_details(output_token);

    let amount_in_requested = match action {
        SwapAction::SolToUsdc { amount_sol } => *amount_sol,
        SwapAction::UsdcToSol { amount_usdc } => *amount_usdc,
    };

    let amount_in_base = to_base_units(amount_in_requested, input_decimals)
        .ok_or_else(|| anyhow!("amount too small to convert to base units"))?;

    let quote = client
        .quote_exact_in(input_mint, output_mint, amount_in_base)
        .await?;

    let simulation = match client
        .simulate_swap(&quote, wraps_sol(input_token, output_token))
        .await
    {
        Ok(result) => Some(result),
        Err(err) => {
            eprintln!("Jupiter simulation failed, ignoring gas data: {err:?}");
            None
        }
    };

    let amount_in = from_base_units(quote.in_amount, input_decimals);
    let amount_out = from_base_units(quote.out_amount, output_decimals);

    Ok(SwapExecution {
        amount_in,
        amount_out,
        input_token,
        output_token,
        gas_lamports: simulation.and_then(|s| s.gas_lamports),
        price_impact_pct: quote.price_impact_pct,
    })
}

fn log_jupiter_warning(err: &Error) {
    if !JUPITER_WARNING_EMITTED.swap(true, Ordering::Relaxed) {
        eprintln!("Jupiter quote failed, falling back to local pricing: {err:?}");
    }
}

fn execute_with_price(price: &PriceInfo, action: &SwapAction) -> SwapExecution {
    match action {
        SwapAction::SolToUsdc { amount_sol } => {
            let amount_in = *amount_sol;
            let amount_out = amount_in * price.value;
            SwapExecution {
                amount_in,
                amount_out,
                input_token: Token::Sol,
                output_token: Token::Usdc,
                gas_lamports: None,
                price_impact_pct: None,
            }
        }
        SwapAction::UsdcToSol { amount_usdc } => {
            let amount_in = *amount_usdc;
            let amount_out = if price.value > 0.0 {
                amount_in / price.value
            } else {
                0.0
            };
            SwapExecution {
                amount_in,
                amount_out,
                input_token: Token::Usdc,
                output_token: Token::Sol,
                gas_lamports: None,
                price_impact_pct: None,
            }
        }
    }
}

fn apply_wallet_updates(
    strategy: &mut StrategyData,
    action: &PendingAction,
    execution: &SwapExecution,
) {
    match action.action {
        SwapAction::SolToUsdc { .. } => {
            strategy.wallet.sol = (strategy.wallet.sol - execution.amount_in).max(0.0);
            strategy.wallet.usdc += execution.amount_out;
        }
        SwapAction::UsdcToSol { .. } => {
            strategy.wallet.usdc = (strategy.wallet.usdc - execution.amount_in).max(0.0);
            strategy.wallet.sol += execution.amount_out;
        }
    }

    if let Some(StrategyPostUpdate::AlternatingNext(next)) = action.post_update {
        if let StrategyState::Alternating { next_swap } = &mut strategy.state {
            *next_swap = next;
        }
    }
}

fn wraps_sol(input: Token, output: Token) -> bool {
    matches!(input, Token::Sol) || matches!(output, Token::Sol)
}

fn token_details(token: Token) -> (&'static str, u8) {
    match token {
        Token::Sol => (SOL_MINT, SOL_DECIMALS),
        Token::Usdc => (USDC_MINT, USDC_DECIMALS),
    }
}

fn to_base_units(amount: f64, decimals: u8) -> Option<u64> {
    if amount <= 0.0 {
        return None;
    }
    let factor = 10f64.powi(decimals as i32);
    let value = (amount * factor).round();
    if value < 1.0 {
        None
    } else {
        Some(value as u64)
    }
}

fn from_base_units(amount: u64, decimals: u8) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (amount as f64) / factor
}

fn current_timestamp() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format("%Y-%m-%d %H:%M:%S%.3f UTC").to_string()
}
