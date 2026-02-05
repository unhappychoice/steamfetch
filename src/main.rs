mod config;
mod display;
mod steam;

use anyhow::Result;
use clap::Parser;

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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let stats = if cli.demo {
        demo_stats()
    } else {
        fetch_stats().await?
    };

    display::render(&stats);
    Ok(())
}

async fn fetch_stats() -> Result<steam::SteamStats> {
    // Try Steamworks SDK first, fallback to Web API
    match NativeSteamClient::try_new() {
        Some(native) => fetch_native_stats(native).await,
        None => fetch_web_stats().await,
    }
}

async fn fetch_web_stats() -> Result<steam::SteamStats> {
    let config = Config::from_env()?;
    let client = SteamClient::new(config.api_key, config.steam_id);
    client.fetch_stats().await
}

async fn fetch_native_stats(native: NativeSteamClient) -> Result<steam::SteamStats> {
    let username = native.username();
    let steam_id = native.steam_id().to_string();

    let all_appids = steam::native::fetch_all_game_appids().await?;
    let owned_appids = native.get_owned_appids(&all_appids);

    let api_key = std::env::var("STEAM_API_KEY").map_err(|_| {
        anyhow::anyhow!(
            "STEAM_API_KEY not set.\n\n\
            To get your API key:\n  \
            1. Visit https://steamcommunity.com/dev/apikey\n  \
            2. Log in and create a key\n  \
            3. Set it: export STEAM_API_KEY=\"your-key\""
        )
    })?;
    let client = SteamClient::new(api_key, steam_id);
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
        country: Some("JP".to_string()),
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
