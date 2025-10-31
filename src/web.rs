use crate::model::AppState;
use crate::simulation::publish_time_to_string;
use axum::{extract::State, response::Html};

pub async fn index(State(state): State<AppState>) -> Html<String> {
    let data = state.lock().await;
    let latest_price_value = data.latest_price.as_ref().map(|price| price.value);
    let latest_price_display = latest_price_value
        .map(|price| format!("{:.4}", price))
        .unwrap_or_else(|| "waiting…".to_string());

    let publish_time_display = data
        .latest_price
        .as_ref()
        .map(|price| publish_time_to_string(price.publish_time))
        .unwrap_or_else(|| "unknown".to_string());

    let strategy_rows = data
        .strategies
        .iter()
        .map(|strategy| {
            let sol = format!("{:.4}", strategy.wallet.sol);
            let usdc = format!("{:.2}", strategy.wallet.usdc);
            let total_usd = latest_price_value
                .map(|price| strategy.wallet.sol * price + strategy.wallet.usdc)
                .map(|total| format!("{:.2}", total))
                .unwrap_or_else(|| "--".to_string());

            format!(
                "<tr>\
                    <td>{}</td>\
                    <td>{}</td>\
                    <td>{}</td>\
                    <td>{}</td>\
                </tr>",
                strategy.id.label(),
                sol,
                usdc,
                total_usd
            )
        })
        .collect::<String>();

    let history_rows = data
        .history
        .iter()
        .rev()
        .map(|record| {
            let gas_display = record
                .gas_lamports
                .map(|value| value.to_string())
                .unwrap_or_else(|| "--".to_string());
            let price_impact_display = record
                .price_impact_pct
                .map(|value| format!("{:.4}", value * 100.0))
                .unwrap_or_else(|| "--".to_string());

            format!(
                "<tr>\
                    <td>{}</td>\
                    <td>{}</td>\
                    <td>{}</td>\
                    <td>{:.4}</td>\
                    <td>{:.4} {}</td>\
                    <td>{:.4} {}</td>\
                    <td>{}</td>\
                    <td>{}</td>\
                </tr>",
                record.timestamp,
                record.strategy.label(),
                record.direction,
                record.price,
                record.amount_in,
                record.input_token.symbol(),
                record.amount_out,
                record.output_token.symbol(),
                gas_display,
                price_impact_display
            )
        })
        .collect::<String>();

    Html(build_page(
        latest_price_display,
        publish_time_display,
        strategy_rows,
        history_rows,
    ))
}

fn build_page(
    latest_price: String,
    publish_time: String,
    strategy_rows: String,
    history_rows: String,
) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>SOL / USDC Simulation</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            margin: 2rem;
            color: #1f2933;
            background: #f5f7fa;
        }}
        h1 {{
            margin-bottom: 0.5rem;
        }}
        .card {{
            background: white;
            border-radius: 0.75rem;
            padding: 1.5rem;
            box-shadow: 0 10px 25px rgba(15, 23, 42, 0.08);
            margin-bottom: 2rem;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        th, td {{
            padding: 0.75rem;
            border-bottom: 1px solid #d9e2ec;
            text-align: left;
        }}
        th {{
            background: #dceefb;
            color: #102a43;
        }}
        tr:nth-child(even) {{
            background: #f0f4f8;
        }}
        .metric {{
            font-size: 1.5rem;
            font-weight: bold;
        }}
        .metric-label {{
            font-size: 0.85rem;
            color: #627d98;
        }}
        .grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
            gap: 1rem;
            margin-top: 1rem;
        }}
        .chip {{
            display: inline-block;
            padding: 0.5rem 1rem;
            border-radius: 999px;
            background: #edf2ff;
            color: #334e68;
            font-size: 0.85rem;
        }}
    </style>
</head>
<body>
    <div class="card">
        <h1>SOL / USDC Live Simulation</h1>
        <div class="chip">Last publish time: {publish_time}</div>
        <div class="metric">${latest_price}</div>
        <div class="metric-label">Latest SOL price (USD)</div>
    </div>
    <div class="card">
        <h2>Strategy Wallets</h2>
        <table>
            <thead>
                <tr>
                    <th>Strategy</th>
                    <th>SOL</th>
                    <th>USDC</th>
                    <th>Total USD</th>
                </tr>
            </thead>
            <tbody>
                {strategy_rows}
            </tbody>
        </table>
    </div>
    <div class="card">
        <h2>Swap History</h2>
        <table>
            <thead>
                <tr>
                    <th>Timestamp</th>
                    <th>Strategy</th>
                    <th>Direction</th>
                    <th>Price (USD)</th>
                    <th>Amount In</th>
                    <th>Amount Out</th>
                    <th>Gas (lamports)</th>
                    <th>Price Impact (%)</th>
                </tr>
            </thead>
            <tbody>
                {history_rows}
            </tbody>
        </table>
    </div>
</body>
</html>
"#,
        publish_time = publish_time,
        latest_price = latest_price,
        strategy_rows = strategy_rows,
        history_rows = history_rows
    )
}
