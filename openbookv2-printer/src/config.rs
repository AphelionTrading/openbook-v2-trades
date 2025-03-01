use clap::Parser;
use log::info;
use solana_program::pubkey::Pubkey;
use std::str::FromStr;

#[derive(Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub rpc_url: Option<String>,
    #[arg(short, long, value_delimiter = ' ', num_args = 0..50)]
    pub market: Vec<String>,
    #[arg(short, long)]
    pub port: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(short, long)]
    pub grpc: Option<String>,
    #[clap(value_enum)]
    pub commitment: Option<Commitment>,
    #[arg(long, action)]
    pub connect: bool,
    #[arg(short, long)]
    pub x_token: Option<String>,
    #[arg(long)]
    pub check: Option<u64>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Commitment {
    Processed,
    Confirmed,
    Finalized,
}

pub struct Config {
    pub rpc_url: String,
    pub market_keys: Vec<Pubkey>,
    pub port: String,
    pub host: String,
    pub grpc: String,
    pub commitment: Commitment,
    pub connect: bool,
    pub x_token: String,
    pub check: u64,
}

impl Config {
    pub fn new() -> Self {
        // Load environment variables from .env files
        Self::load_env_files();
        
        // Start with default values
        let mut config = Config {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            market_keys: vec![],
            port: "8585".to_string(),
            host: "127.0.0.1".to_string(),
            grpc: "http://127.0.0.1:10000".to_string(),
            commitment: Commitment::Finalized,
            connect: false,
            x_token: "x-token".to_string(),
            check: 1000,
        };
        
        // Default market string
        let mut market_str = "ACP9pwHhehxpsQcAzEi5bb93oUgushtJ1A1dtZaeSKWY".to_string();
        
        // Override with environment variables if they exist
        if let Ok(rpc_url) = std::env::var("RPC_URL") {
            config.rpc_url = rpc_url;
        }
        
        if let Ok(port) = std::env::var("PORT") {
            config.port = port;
        }
        
        if let Ok(host) = std::env::var("HOST") {
            config.host = host;
        }
        
        if let Ok(grpc) = std::env::var("GRPC_URL") {
            config.grpc = grpc;
        }
        
        if let Ok(x_token) = std::env::var("X_TOKEN") {
            config.x_token = x_token;
        }
        
        if let Ok(env_market) = std::env::var("MARKET") {
            market_str = env_market;
        }
        
        // Parse CLI arguments
        let cli = Cli::parse();
        
        // Override with CLI arguments if they exist
        if let Some(rpc_url) = cli.rpc_url {
            config.rpc_url = rpc_url;
        }
        
        if let Some(port) = cli.port {
            config.port = port;
        }
        
        if let Some(host) = cli.host {
            config.host = host;
        }
        
        if let Some(grpc) = cli.grpc {
            config.grpc = grpc;
        }
        
        if let Some(commitment) = cli.commitment {
            config.commitment = commitment;
        }
        
        if let Some(x_token) = cli.x_token {
            config.x_token = x_token;
        }
        
        if let Some(check) = cli.check {
            config.check = check;
        }
        
        config.connect = cli.connect;
        
        let markets = if !cli.market.is_empty() {
            cli.market
        } else {
            // Parse the market string (from default or environment)
            market_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        
        // Convert market strings to Pubkeys
        config.market_keys = markets
            .iter()
            .map(|market_key| Pubkey::from_str(market_key).unwrap())
            .collect();
        
        config
    }
    
    fn load_env_files() {
        // Try loading from multiple possible locations
        let env_paths = vec![
            ".env",
            "../.env",
            "../../.env",
            // Add more potential paths if needed
        ];

        for path in env_paths {
            info!("Trying to load .env from: {}", path);
            match dotenv::from_path(path) {
                Ok(_) => {
                    info!("Successfully loaded .env from: {}", path);
                    break;
                },
                Err(e) => info!("Could not load .env from {}: {}", path, e),
            }
        }
    }
} 