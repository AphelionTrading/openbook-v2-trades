use crate::logs::{FillLog, Trade};
use crate::name::parse_name;
use crate::utils::{get_owner_account_for_ooa, price_lots_to_ui, to_native, to_ui_decimals};
use anchor_lang::__private::base64;
use anchor_lang::{AnchorDeserialize, AnchorSerialize, Discriminator};
use futures::StreamExt;
use log::{debug, error, info, warn, LevelFilter};
use openbookv2_generated::state::Market;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::hash::Hash;
use solana_program::pubkey::Pubkey;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::Signature;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::subscribe_update::UpdateOneof;
use yellowstone_grpc_proto::geyser::CommitmentLevel;
use yellowstone_grpc_proto::prelude::{SubscribeRequest, SubscribeRequestFilterTransactions};
use dotenv::dotenv;
use env_logger::fmt::Formatter;
use std::io::Write;
use chrono;

pub mod constants;
mod config;
mod logs;
mod market;
mod name;
mod utils;

use config::{Config, Commitment};

// Custom logger format that doesn't include the module path
fn custom_format(
    buf: &mut Formatter,
    record: &log::Record,
) -> std::io::Result<()> {
    let level = record.level();
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    
    writeln!(
        buf,
        "{} [{}] {}",
        timestamp,
        level,
        record.args()
    )
}

// CFSMrBssNG8Ud1edW59jNLnq2cwrQ9uY5cM3wXmqRJj3 DBSZ24hqXS5o8djunrTzBsJUb1P8ZvBs1nng5rmZKsJt 5h4DTiBqZctQWq7xc3H2t8qRdGcFNQNk1DstVNnbJvXs
#[tokio::main]
async fn main() {
    // Initialize logger with custom format based on RUST_LOG
    let log_level = match std::env::var("RUST_LOG") {
        Ok(level) => {
            match level.to_lowercase().as_str() {
                "trace" => LevelFilter::Trace,
                "debug" => LevelFilter::Debug,
                "info" => LevelFilter::Info,
                "warn" => LevelFilter::Warn,
                "error" => LevelFilter::Error,
                _ => LevelFilter::Info,
            }
        },
        Err(_) => LevelFilter::Info,
    };
    
    env_logger::builder()
        .format(custom_format)
        .filter_level(log_level)
        .init();
    
    // Load configuration from CLI and environment
    let config = Config::new();
    
    // Print configuration in a nicely formatted table
    info!("╔════════════════════════════════════════════════════════════════════════════╗");
    info!("║                           CONFIGURATION                                    ║");
    info!("╠════════════════════════════════════════════════════════════════════════════╣");
    info!("║ RPC URL:      {:<60} ║", config.rpc_url);
    info!("║ GRPC URL:     {:<60} ║", config.grpc);
    info!("║ Host:         {:<60} ║", config.host);
    info!("║ Port:         {:<60} ║", config.port);
    info!("║ Commitment:   {:<60} ║", format!("{:?}", config.commitment));
    info!("║ Connect Mode: {:<60} ║", if config.connect { "Connect" } else { "Bind" });
    info!("║ X-Token:      {:<60} ║", config.x_token);
    info!("║ Check:        {:<60} ║", config.check);
    info!("╠════════════════════════════════════════════════════════════════════════════╣");
    info!("║ Markets:                                                                   ║");
    for (i, market_key) in config.market_keys.iter().enumerate() {
        info!("║  {:<2}: {:<69} ║", i+1, market_key.to_string());
    }
    info!("╚════════════════════════════════════════════════════════════════════════════╝");

    // Add this before the dotenv() call
    match std::env::current_dir() {
        Ok(path) => info!("Current working directory: {:?}", path),
        Err(e) => info!("Could not determine current directory: {}", e),
    }

    let processed_commitment = CommitmentConfig::processed();
    let client = RpcClient::new_with_commitment(config.rpc_url.clone(), processed_commitment);
    let client_for_slot = RpcClient::new_with_commitment(config.rpc_url.clone(), processed_commitment);
    
    let accounts = client.get_multiple_accounts(&config.market_keys).await.unwrap();
    let mut market_names = BTreeMap::new();
    let mut markets = BTreeMap::new();
    for (idx, option) in accounts.iter().enumerate() {
        if let Some(account) = option {
            let data = account.data.clone();
            let market = Market::deserialize(&mut &data[8..]).unwrap();
            let market_name = parse_name(&market.name);
            market_names.insert(config.market_keys[idx], market_name.clone());
            markets.insert(config.market_keys[idx], market);
            info!("Subscribing for fills for market: {:<30} Pubkey: {:<10}", market_name.as_str(), &config.market_keys[idx].to_string()[..5]);
        } else {
            warn!("Market account not found for pubkey: {}", config.market_keys[idx]);
        }
    }

    let mut grpc_client = GeyserGrpcClient::build_from_shared(config.grpc)
        .unwrap()
        .x_token(Some(config.x_token.clone()))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let pong = grpc_client.ping(0).await.unwrap();
    info!("{:?}", pong);

    let mut transactions = HashMap::new();
    for key in markets.keys() {
        let tx_filter = SubscribeRequestFilterTransactions {
            vote: None,
            failed: Some(false),
            signature: None,
            account_include: vec![],
            account_exclude: vec![],
            account_required: vec![key.to_string()],
        };
        transactions.insert(key.to_string(), tx_filter);
    }
    let commitment = match config.commitment {
        Commitment::Processed => CommitmentLevel::Processed,
        Commitment::Confirmed => CommitmentLevel::Confirmed,
        Commitment::Finalized => CommitmentLevel::Finalized,
    };
    let request = SubscribeRequest {
        accounts: Default::default(),
        slots: Default::default(),
        transactions,
        blocks: Default::default(),
        blocks_meta: Default::default(),
        entry: Default::default(),
        commitment: Some(i32::from(commitment)),
        accounts_data_slice: vec![],
        ping: None,
        transactions_status: Default::default(),
    };

    let (tx_sender, mut tx_receiver) = unbounded_channel::<(FillLog, String)>();
    let discriminator = FillLog::discriminator();
    let request = request.clone();
    let check = config.check;
    spawn(async move {
        let mut counter = 0;
        let mut check = check;
        'outer: loop {
            // Add error handling for the GRPC client connection
            let subscribe_result = grpc_client
                .subscribe_with_request(Some(request.clone()))
                .await;
                
            let (_subscribe_tx, mut stream) = match subscribe_result {
                Ok(result) => result,
                Err(err) => {
                    error!("Failed to subscribe to GRPC: {:?}. Retrying in 5 seconds...", err);
                    sleep(Duration::from_secs(5)).await;
                    continue 'outer; // Retry the outer loop
                }
            };
            
            loop {
                let message = stream.next().await;
                match message {
                    Some(Ok(msg)) => {
                        debug!("new message: {msg:?}");
                        #[allow(clippy::single_match)]
                        match msg.update_oneof {
                            Some(UpdateOneof::Transaction(txn)) => {
                                let tx = txn.transaction.unwrap();
                                let logs = tx.meta.unwrap().log_messages;
                                for log in logs.iter() {
                                    if log.contains("Program data: ") {
                                        let data = log.replace("Program data: ", "");
                                        let data = base64::decode(data).unwrap();
                                        if discriminator == data.as_slice()[..8] {
                                            if counter >= check {
                                                let time =
                                                    client_for_slot.get_block_time(txn.slot).await;
                                                match time {
                                                    Ok(t) => {
                                                        let system_t = SystemTime::now()
                                                            .duration_since(UNIX_EPOCH)
                                                            .unwrap()
                                                            .as_secs();
                                                        info!(
                                                            "checking slot: {} lagging: {} s",
                                                            txn.slot,
                                                            system_t - t.unsigned_abs()
                                                        )
                                                    }
                                                    Err(err) => {
                                                        warn!(
                                                            "during checking slot got: {:?}",
                                                            err
                                                        );
                                                    }
                                                }
                                                check = 0;
                                            }
                                            let signature =
                                                Signature::new(&tx.signature).to_string();
                                            let fill_log =
                                                FillLog::deserialize(&mut &data[8..]).unwrap();
                                            tx_sender.send((fill_log, signature)).unwrap();
                                            counter += 1;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Err(e)) => {
                        error!("Stream error: {:?}. Reconnecting...", e);
                        sleep(Duration::from_secs(1)).await;
                        break; // Exit inner loop to reconnect
                    }
                    None => {
                        warn!("Stream returned None. Restarting connection...");
                        sleep(Duration::from_secs(1)).await;
                        break;
                    }
                }
            }
        }
    });

    let ctx = zmq::Context::new();
    let zero_url = format!("tcp://{}:{}", config.host, config.port);
    let socket = ctx.socket(zmq::PUB).unwrap();
    if config.connect {
        socket.connect(&zero_url).unwrap()
    } else {
        socket.bind(&zero_url).unwrap();
    }

    let mut ooa2owner = BTreeMap::new();
    while let Some((mut fill_log, tx_hash)) = tx_receiver.recv().await {
        if let Some(market) = markets.get(&fill_log.market) {
            let market_name: &String = market_names.get(&fill_log.market).unwrap();
            let result = get_owner_account_for_ooa(&client, &ooa2owner, &fill_log.maker).await;
            if result.is_some() {
                let maker_owner = result.unwrap();
                if ooa2owner.contains_key(&fill_log.maker) {
                    ooa2owner.insert(fill_log.maker, maker_owner);
                }
                fill_log.maker = maker_owner;
            }
            let result = get_owner_account_for_ooa(&client, &ooa2owner, &fill_log.taker).await;
            if result.is_some() {
                let maker_owner = result.unwrap();
                if ooa2owner.contains_key(&fill_log.taker) {
                    ooa2owner.insert(fill_log.taker, maker_owner);
                }
                fill_log.taker = maker_owner;
            }
            let trade = Trade::new(
                &fill_log,
                market,
                market_name.clone().replace('\0', ""),
                tx_hash.clone(),
            );
            let t = serde_json::to_string(&trade).unwrap();
            let r = socket.send(&t, 0);
            match r {
                Ok(_) => {}
                Err(err) => {
                    error!("sending to socket returned error: {}", err);
                }
            }
            info!("{:?}, signature: {}", t, tx_hash);
        } else {
            warn!("tx: {} contains log, which can't be parsed, because does not contain specified market", tx_hash);
        }
    }
}
