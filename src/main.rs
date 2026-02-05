mod config;
mod display;
mod steam;

use anyhow::Result;
use clap::Parser;

use config::Config;
use steam::SteamClient;

#[derive(Parser)]
#[command(name = "steamfetch")]
#[command(about = "neofetch for Steam - Display your Steam stats in terminal")]
#[command(version)]
struct Cli {}

#[tokio::main]
async fn main() -> Result<()> {
    let _cli = Cli::parse();
    let config = Config::from_env()?;
    let client = SteamClient::new(config.api_key, config.steam_id);
    let stats = client.fetch_stats().await?;

    display::render(&stats);
    Ok(())
}
