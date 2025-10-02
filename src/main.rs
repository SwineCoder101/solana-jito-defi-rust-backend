use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use jito_sdk_rust::JitoJsonRpcSDK;
use serde_json::json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::{fs::File, io::BufReader, str::FromStr, env};
use tokio::time::{sleep, Duration};


const NUMBER_TRANSACTIONS: usize = 5;
const MIN_JITO_TIP_LAMPORTS: u64 = 1_000;  // 1_000 lamports ≈ 0.000001 SOL
const POLL_INTERVAL_SECS: u64 = 5;
const POLL_TIMEOUT_SECS: u64 = 60;

fn load_keypair(path: &str) -> Result<Keypair> {
    let reader = BufReader::new(File::open(path)?);
    let bytes: Vec<u8> = serde_json::from_reader(reader)?;
    Ok(Keypair::from_bytes(&bytes)?)
}

async fn poll_bundle_status(
    sdk: &JitoJsonRpcSDK,
    bundle_id: &str,
) -> Result<()> {
    let start = tokio::time::Instant::now();
    loop {
        let resp = sdk
            .get_in_flight_bundle_statuses(vec![bundle_id.to_string()])
            .await?;

        let status = resp["result"]["value"][0]["status"]
            .as_str()
            .unwrap_or("Unknown");
        match status {
            "Landed" => return Ok(()),
            "Failed" => return Err(anyhow!("bundle failed on‑chain")),
            _ => {
                if start.elapsed() > Duration::from_secs(POLL_TIMEOUT_SECS) {
                    return Err(anyhow!("bundle not confirmed in time"));
                }
                sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // STEP 1 — local key & RPC handles
    let payer = load_keypair("secret.json")?;
    println!("Using wallet: {}", payer.pubkey());
    dotenv::dotenv().ok();
    let jito_endpoint = env::var("JITO_ENDPOINT").expect("JITO_ENDPOINT must be set in .env file");
    let solana_rpc = env::var("SOLANA_RPC").expect("SOLANA_RPC must be set in .env file");

    let solana_rpc = RpcClient::new(solana_rpc.as_str());
    let jito_sdk = JitoJsonRpcSDK::new(jito_endpoint.as_str(), None);

    // STEP 2 — choose a Jito tip account
    let random_tip_account = jito_sdk.get_random_tip_account().await?;
    let jito_tip_account = Pubkey::from_str(&random_tip_account)?;
    println!("Selected tip account: {random_tip_account}");

    // STEP 3 — recent block‑hash
    let blockhash = solana_rpc.get_latest_blockhash()?;
    println!("Latest blockhash: {blockhash}");
    
    // STEP 4 — build & sign five transactions
    let memo_program = Pubkey::from_str("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr")?;
    let mut encoded: Vec<String> = Vec::with_capacity(NUMBER_TRANSACTIONS);

    for i in 0..NUMBER_TRANSACTIONS {
        let memo_ix = Instruction::new_with_bytes(
            memo_program,
            format!("lil jit demo transaction # {}", i + 1).as_bytes(),
            vec![AccountMeta::new(payer.pubkey(), true)],
        );

        let mut ixs = vec![memo_ix];

        // For the last transaction, add a tip instruction
        if i == NUMBER_TRANSACTIONS - 1 {
            ixs.push(system_instruction::transfer(
                &payer.pubkey(),
                &jito_tip_account,
                MIN_JITO_TIP_LAMPORTS,
            ));
        }

        let mut tx = Transaction::new_with_payer(&ixs, Some(&payer.pubkey()));
        tx.sign(&[&payer], blockhash);
        let bytes = bincode::serialize(&tx)?;
        encoded.push(general_purpose::STANDARD.encode(bytes));
    }

    println!("Signed and encoded all {NUMBER_TRANSACTIONS} transactions…");

    // STEP 5 — send the bundle
    let params = json!([
        encoded.clone(),
        { "encoding": "base64" }
    ]);
    let resp = jito_sdk.send_bundle(Some(params), None).await?;
    let bundle_id = resp["result"]
        .as_str()
        .ok_or_else(|| anyhow!("no bundle id in response"))?;

    println!("Bundle submitted: {bundle_id}");

    // STEP 6 — confirm inclusion
    poll_bundle_status(&jito_sdk, bundle_id).await?;
    println!("Bundle landed! View it at https://explorer.jito.wtf/bundle/{bundle_id}");

    Ok(())
}
