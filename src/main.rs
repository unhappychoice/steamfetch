mod config;
mod display;
mod steam;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use config::Config;
use steam::{NativeSteamClient, SteamClient};

#[derive(Parser)]
#[command(name = "steamfetch")]
#[command(about = "neofetch for Steam - Display your Steam stats in terminal")]
#[command(version)]
struct Cli {
    /// Show demo output with sample data
    #[arg(long)]
    demo: bool,

    /// Show verbose output for debugging
    #[arg(long, short)]
    verbose: bool,

    /// Path to config file
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Show config file path and exit
    #[arg(long)]
    config_path: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.config_path {
        match config::config_path() {
            Some(path) => println!("{}", path.display()),
            None => eprintln!("Could not determine config directory"),
        }
        return Ok(());
    }

    let stats = if cli.demo {
        demo_stats()
    } else {
        fetch_stats(cli.verbose, cli.config).await?
    };

    display::render(&stats);
    Ok(())
}

async fn fetch_stats(verbose: bool, config_path: Option<PathBuf>) -> Result<steam::SteamStats> {
    // Try Steamworks SDK first, fallback to Web API
    match NativeSteamClient::try_new(verbose) {
        Some(native) => fetch_native_stats(native, verbose, config_path).await,
        None => fetch_web_stats(verbose, config_path).await,
    }
}

async fn fetch_web_stats(verbose: bool, config_path: Option<PathBuf>) -> Result<steam::SteamStats> {
    let config = Config::load(config_path)?;
    let client = SteamClient::new(config.api_key, config.steam_id).with_verbose(verbose);
    client.fetch_stats().await
}

async fn fetch_native_stats(
    native: NativeSteamClient,
    verbose: bool,
    config_path: Option<PathBuf>,
) -> Result<steam::SteamStats> {
    let username = native.username();
    let steam_id = native.steam_id().to_string();

    if verbose {
        eprintln!("[verbose] Native SDK username: {}", username);
        eprintln!("[verbose] Native SDK steam_id: {}", steam_id);
    }

    let all_appids = steam::native::fetch_all_game_appids().await?;
    let owned_appids = native.get_owned_appids(&all_appids);

    if verbose {
        eprintln!(
            "[verbose] Found {} owned games via Native SDK",
            owned_appids.len()
        );
    }

    // Load API key only (steam_id already obtained from Native SDK)
    let api_key = Config::load_api_key_only(config_path)?;
    let client = SteamClient::new(api_key, steam_id).with_verbose(verbose);
    client
        .fetch_stats_for_appids(&owned_appids, &username)
        .await
}

fn demo_stats() -> steam::SteamStats {
    use steam::SteamStats;

    SteamStats {
        username: "unhappychoice".to_string(),
        game_count: 486,
        unplayed_count: 123,
        total_playtime_minutes: 170820,
        top_games: vec![
            steam::GameStat {
                name: "Borderlands 3".to_string(),
                playtime_minutes: 28680,
            },
            steam::GameStat {
                name: "Coin Push RPG".to_string(),
                playtime_minutes: 22620,
            },
            steam::GameStat {
                name: "DRG Survivor".to_string(),
                playtime_minutes: 15120,
            },
        ],
        achievement_stats: Some(steam::AchievementStats {
            total_achieved: 3241,
            total_possible: 5892,
            perfect_games: 24,
            rarest: Some(steam::RarestAchievement {
                name: "Impossible Task".to_string(),
                game: "Dark Souls III".to_string(),
                percent: 0.1,
            }),
        }),
        account_created: Some(1234567890),
        steam_level: Some(42),
        recently_played: vec![
            steam::GameStat {
                name: "Elden Ring".to_string(),
                playtime_minutes: 1200,
            },
            steam::GameStat {
                name: "Hades II".to_string(),
                playtime_minutes: 480,
            },
        ],
    }
}
