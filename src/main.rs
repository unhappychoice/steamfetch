mod cache;
mod config;
mod display;
mod image_display;
mod steam;
#[cfg(test)]
mod test_support;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use config::Config;
use steam::{NativeSteamClient, SteamClient};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ImageProtocol {
    Auto,
    Kitty,
    Iterm,
    Sixel,
}

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

    /// Request timeout in seconds (default: 30)
    #[arg(long, value_name = "SECONDS", default_value = "30", value_parser = clap::value_parser!(u64).range(1..))]
    timeout: u64,

    /// Show profile avatar as image instead of ASCII logo
    #[arg(long)]
    image: bool,

    /// Image display protocol (auto, kitty, iterm, sixel)
    #[arg(long, value_enum, default_value = "auto")]
    image_protocol: ImageProtocol,
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

    let image_config = display::ImageConfig {
        enabled: cli.image,
        protocol: cli.image_protocol,
    };

    let stats = if cli.demo {
        demo_stats()
    } else {
        fetch_stats(&cli).await?
    };

    display::render(&stats, &image_config).await;
    Ok(())
}

async fn fetch_stats(cli: &Cli) -> Result<steam::SteamStats> {
    match NativeSteamClient::try_new(cli.verbose) {
        Some(native) => fetch_native_stats(native, cli).await,
        None => fetch_web_stats(cli).await,
    }
}

async fn fetch_web_stats(cli: &Cli) -> Result<steam::SteamStats> {
    let config = Config::load(cli.config.clone())?;
    let client = SteamClient::new(config.api_key, config.steam_id)
        .with_verbose(cli.verbose)
        .with_timeout(cli.timeout);
    client.fetch_stats().await
}

async fn fetch_native_stats(native: NativeSteamClient, cli: &Cli) -> Result<steam::SteamStats> {
    let username = native.username();
    let steam_id = native.steam_id().to_string();

    if cli.verbose {
        eprintln!("[verbose] Native SDK username: {}", username);
        eprintln!("[verbose] Native SDK steam_id: {}", steam_id);
    }

    let all_appids = steam::native::fetch_all_game_appids().await?;
    let owned_appids = native.get_owned_appids(&all_appids);

    if cli.verbose {
        eprintln!(
            "[verbose] Found {} owned games via Native SDK",
            owned_appids.len()
        );
    }

    let api_key = Config::load_api_key_only(cli.config.clone())?;
    let client = SteamClient::new(api_key, steam_id)
        .with_verbose(cli.verbose)
        .with_timeout(cli.timeout);
    client
        .fetch_stats_for_appids(&owned_appids, &username)
        .await
}

pub(crate) fn demo_stats() -> steam::SteamStats {
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
        avatar_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_demo_stats_returns_expected_username() {
        let stats = demo_stats();
        assert_eq!(stats.username, "unhappychoice");
    }

    #[test]
    fn test_demo_stats_top_games_in_descending_playtime() {
        let stats = demo_stats();
        assert!(stats.top_games.len() >= 2);
        for window in stats.top_games.windows(2) {
            assert!(window[0].playtime_minutes >= window[1].playtime_minutes);
        }
    }

    #[test]
    fn test_demo_stats_unplayed_does_not_exceed_total() {
        let stats = demo_stats();
        assert!(stats.unplayed_count <= stats.game_count);
    }

    #[test]
    fn test_demo_stats_achievements_consistent() {
        let stats = demo_stats();
        let ach = stats.achievement_stats.expect("demo has achievements");
        assert!(ach.total_achieved <= ach.total_possible);
        assert!(ach.perfect_games <= stats.game_count);
        let rarest = ach.rarest.expect("demo has rarest achievement");
        assert!(rarest.percent >= 0.0 && rarest.percent <= 100.0);
        assert!(!rarest.name.is_empty());
        assert!(!rarest.game.is_empty());
    }

    #[test]
    fn test_demo_stats_recently_played_has_entries() {
        let stats = demo_stats();
        assert!(!stats.recently_played.is_empty());
        for game in &stats.recently_played {
            assert!(!game.name.is_empty());
        }
    }

    #[test]
    fn test_demo_stats_avatar_url_is_none() {
        assert!(demo_stats().avatar_url.is_none());
    }

    #[test]
    fn test_demo_stats_profile_details_are_present() {
        let stats = demo_stats();
        let achievements = stats.achievement_stats.as_ref().expect("demo achievements");
        let rarest = achievements.rarest.as_ref().expect("demo rarest");

        assert_eq!(stats.account_created, Some(1234567890));
        assert_eq!(stats.steam_level, Some(42));
        assert_eq!(rarest.name, "Impossible Task");
        assert_eq!(rarest.game, "Dark Souls III");
        assert_eq!(stats.recently_played[0].playtime_minutes, 1200);
    }

    #[test]
    fn test_cli_parses_minimum_args() {
        let cli = Cli::try_parse_from(["steamfetch"]).expect("default args should parse");
        assert!(!cli.demo);
        assert!(!cli.verbose);
        assert!(!cli.config_path);
        assert!(!cli.image);
        assert_eq!(cli.timeout, 30);
        assert!(cli.config.is_none());
        assert!(matches!(cli.image_protocol, ImageProtocol::Auto));
    }

    #[test]
    fn test_cli_parses_demo_flag() {
        let cli = Cli::try_parse_from(["steamfetch", "--demo"]).expect("--demo should parse");
        assert!(cli.demo);
    }

    #[test]
    fn test_cli_parses_verbose_short_flag() {
        let cli = Cli::try_parse_from(["steamfetch", "-v"]).expect("-v should parse");
        assert!(cli.verbose);
    }

    #[test]
    fn test_cli_parses_verbose_long_flag_with_other_options() {
        let cli = Cli::try_parse_from([
            "steamfetch",
            "--verbose",
            "--timeout",
            "5",
            "--image",
            "--image-protocol",
            "sixel",
            "--config",
            "/tmp/steamfetch.toml",
        ])
        .expect("long verbose and related options should parse");

        assert!(cli.verbose);
        assert_eq!(cli.timeout, 5);
        assert!(cli.image);
        assert!(matches!(cli.image_protocol, ImageProtocol::Sixel));
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/steamfetch.toml"))
        );
    }

    #[test]
    fn test_cli_parses_demo_with_image_options() {
        let cli = Cli::try_parse_from([
            "steamfetch",
            "--demo",
            "--image",
            "--image-protocol",
            "iterm",
            "--timeout",
            "9",
            "--config",
            "/tmp/demo.toml",
        ])
        .expect("demo with image options should parse");

        assert!(cli.demo);
        assert!(cli.image);
        assert!(matches!(cli.image_protocol, ImageProtocol::Iterm));
        assert_eq!(cli.timeout, 9);
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/demo.toml"))
        );
    }

    #[test]
    fn test_cli_parses_config_path_flag() {
        let cli = Cli::try_parse_from(["steamfetch", "--config-path"])
            .expect("--config-path should parse");
        assert!(cli.config_path);
    }

    #[test]
    fn test_cli_parses_custom_timeout() {
        let cli =
            Cli::try_parse_from(["steamfetch", "--timeout", "5"]).expect("timeout should parse");
        assert_eq!(cli.timeout, 5);
    }

    #[test]
    fn test_cli_rejects_zero_timeout() {
        // Range is 1.. — zero must be rejected by clap's value_parser.
        assert!(Cli::try_parse_from(["steamfetch", "--timeout", "0"]).is_err());
    }

    #[test]
    fn test_cli_parses_image_with_protocol() {
        let cli = Cli::try_parse_from(["steamfetch", "--image", "--image-protocol", "kitty"])
            .expect("image flags should parse");
        assert!(cli.image);
        assert!(matches!(cli.image_protocol, ImageProtocol::Kitty));
    }

    #[test]
    fn test_cli_parses_each_image_protocol_variant() {
        for (arg, expected) in [
            ("auto", "Auto"),
            ("kitty", "Kitty"),
            ("iterm", "Iterm"),
            ("sixel", "Sixel"),
        ] {
            let cli = Cli::try_parse_from(["steamfetch", "--image-protocol", arg])
                .unwrap_or_else(|_| panic!("--image-protocol {} should parse", arg));
            let got = format!("{:?}", cli.image_protocol);
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn test_cli_rejects_unknown_image_protocol() {
        assert!(Cli::try_parse_from(["steamfetch", "--image-protocol", "bogus"]).is_err());
    }

    #[test]
    fn test_cli_rejects_missing_option_values() {
        for args in [
            ["steamfetch", "--timeout"],
            ["steamfetch", "--config"],
            ["steamfetch", "--image-protocol"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn test_cli_parses_config_path_value() {
        let cli = Cli::try_parse_from(["steamfetch", "--config", "/tmp/cfg.toml"])
            .expect("--config should parse");
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/cfg.toml"))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_fetch_stats_routes_to_web_stats_when_native_unavailable() {
        // On this Linux test environment the user's real $HOME may have a
        // Steam install at ~/.steam/sdk64/steamclient.so, so calling
        // `fetch_stats` without scoping $HOME could load real steamclient.so
        // and reach Steam SDK code. Point $HOME at an empty temp directory:
        // `NativeSteamClient::try_new` then matches the
        // `None => fetch_web_stats(cli).await` arm of `fetch_stats`. Pair it
        // with a malformed config path so `fetch_web_stats` propagates a
        // `Config::load` error before any HTTP request — proving the routing
        // hit the web-stats path.
        use std::env;
        use std::path::PathBuf;
        use std::time::{SystemTime, UNIX_EPOCH};

        let _guard = crate::test_support::lock_env();

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home: PathBuf = env::temp_dir().join(format!(
            "steamfetch-fetch-stats-home-{}-{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&home).unwrap();

        let cfg_path = env::temp_dir().join(format!(
            "steamfetch-fetch-stats-cfg-{}-{}.toml",
            std::process::id(),
            nanos
        ));
        std::fs::write(&cfg_path, "this is = not [valid toml").unwrap();

        let prev_home = env::var("HOME").ok();
        env::set_var("HOME", &home);

        let cli = Cli {
            demo: false,
            verbose: false,
            config: Some(cfg_path.clone()),
            config_path: false,
            timeout: 30,
            image: false,
            image_protocol: ImageProtocol::Auto,
        };

        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(fetch_stats(&cli))
            .expect_err("malformed config should propagate from fetch_web_stats");
        let msg = format!("{:#}", err);

        match prev_home {
            Some(v) => env::set_var("HOME", v),
            None => env::remove_var("HOME"),
        }
        let _ = std::fs::remove_file(&cfg_path);
        let _ = std::fs::remove_dir_all(&home);

        assert!(
            msg.contains("Failed to parse config file"),
            "expected web-stats config-parse failure, got: {msg}",
        );
    }

    #[tokio::test]
    async fn test_fetch_web_stats_propagates_config_load_error() {
        // Invalid TOML in the supplied config path makes `Config::load` return
        // Err, which `fetch_web_stats` propagates via `?` before constructing
        // the SteamClient or making any HTTP request. Exercises the function
        // entry, the `Config::load(...)?` line, and the early-return path.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "steamfetch-fetch-web-stats-test-{}-{}.toml",
            std::process::id(),
            nanos
        ));
        std::fs::write(&path, "this is = not [valid toml").unwrap();

        let cli = Cli {
            demo: false,
            verbose: false,
            config: Some(path.clone()),
            config_path: false,
            timeout: 30,
            image: false,
            image_protocol: ImageProtocol::Auto,
        };

        let err = fetch_web_stats(&cli)
            .await
            .expect_err("invalid TOML should make Config::load propagate an error");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Failed to parse config file"),
            "expected parse-failure context, got: {msg}",
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_fetch_web_stats_propagates_client_fetch_error_after_valid_config() {
        use std::env;

        const PROXY_VARS: &[&str] = &[
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "NO_PROXY",
            "no_proxy",
        ];

        struct EnvScope {
            saved: Vec<(&'static str, Option<String>)>,
        }

        impl EnvScope {
            fn set_unreachable_proxy(url: &str) -> Self {
                let saved = PROXY_VARS
                    .iter()
                    .map(|&key| {
                        let prev = env::var(key).ok();
                        env::remove_var(key);
                        (key, prev)
                    })
                    .collect();

                env::set_var("HTTPS_PROXY", url);
                env::set_var("https_proxy", url);
                env::set_var("ALL_PROXY", url);
                env::set_var("all_proxy", url);
                env::set_var("NO_PROXY", "127.0.0.1,localhost");
                env::set_var("no_proxy", "127.0.0.1,localhost");

                Self { saved }
            }
        }

        impl Drop for EnvScope {
            fn drop(&mut self) {
                for (key, value) in &self.saved {
                    match value {
                        Some(v) => env::set_var(key, v),
                        None => env::remove_var(key),
                    }
                }
            }
        }

        let _guard = crate::test_support::lock_env();
        let outer_all_proxy = env::var("ALL_PROXY").ok();
        env::set_var("ALL_PROXY", "http://proxy-sentinel.invalid:9");

        let err = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
            let addr = listener.local_addr().expect("local addr");
            drop(listener);
            let _scope = EnvScope::set_unreachable_proxy(&format!("http://{}", addr));

            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = env::temp_dir().join(format!(
                "steamfetch-fetch-web-stats-valid-{}-{}.toml",
                std::process::id(),
                nanos
            ));
            std::fs::write(
                &path,
                r#"
[api]
steam_api_key = "test-key"
steam_id = "76561197960265728"
"#,
            )
            .unwrap();

            let cli = Cli {
                demo: false,
                verbose: false,
                config: Some(path.clone()),
                config_path: false,
                timeout: 1,
                image: false,
                image_protocol: ImageProtocol::Auto,
            };

            let err = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt")
                .block_on(fetch_web_stats(&cli))
                .expect_err("unreachable proxy should make fetch_stats fail");

            let _ = std::fs::remove_file(&path);
            err
        };

        assert_eq!(
            env::var("ALL_PROXY").unwrap(),
            "http://proxy-sentinel.invalid:9"
        );
        match outer_all_proxy {
            Some(v) => env::set_var("ALL_PROXY", v),
            None => env::remove_var("ALL_PROXY"),
        }

        assert!(err.downcast_ref::<steam::error::SteamApiError>().is_some());
    }

    #[test]
    fn test_fetch_web_stats_uses_env_credentials_before_client_error() {
        use std::env;

        const PROXY_VARS: &[&str] = &[
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "NO_PROXY",
            "no_proxy",
        ];

        struct ProxyEnvScope {
            saved: Vec<(&'static str, Option<String>)>,
        }

        impl ProxyEnvScope {
            fn set_unreachable_proxy(url: &str) -> Self {
                let saved = PROXY_VARS
                    .iter()
                    .map(|&key| {
                        let prev = env::var(key).ok();
                        env::remove_var(key);
                        (key, prev)
                    })
                    .collect();

                env::set_var("HTTPS_PROXY", url);
                env::set_var("https_proxy", url);
                env::set_var("ALL_PROXY", url);
                env::set_var("all_proxy", url);
                env::set_var("NO_PROXY", "127.0.0.1,localhost");
                env::set_var("no_proxy", "127.0.0.1,localhost");

                Self { saved }
            }
        }

        impl Drop for ProxyEnvScope {
            fn drop(&mut self) {
                for (key, value) in &self.saved {
                    match value {
                        Some(v) => env::set_var(key, v),
                        None => env::remove_var(key),
                    }
                }
            }
        }

        fn restore_env(key: &str, value: Option<String>) {
            match value {
                Some(v) => env::set_var(key, v),
                None => env::remove_var(key),
            }
        }

        let _guard = crate::test_support::lock_env();
        let original_api_key = env::var("STEAM_API_KEY").ok();
        let original_steam_id = env::var("STEAM_ID").ok();
        env::set_var("STEAM_API_KEY", "env-test-key");
        env::set_var("STEAM_ID", "76561197960265728");

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let proxy_url = format!("http://{}", listener.local_addr().expect("local addr"));
        drop(listener);
        let _proxy = ProxyEnvScope::set_unreachable_proxy(&proxy_url);

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = env::temp_dir().join(format!(
            "steamfetch-fetch-web-stats-env-{}-{}.toml",
            std::process::id(),
            nanos
        ));
        std::fs::write(&path, "").unwrap();

        let cli = Cli {
            demo: false,
            verbose: true,
            config: Some(path.clone()),
            config_path: false,
            timeout: 1,
            image: false,
            image_protocol: ImageProtocol::Auto,
        };

        let err = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(fetch_web_stats(&cli))
            .expect_err("unreachable proxy should make env-backed fetch fail");

        let _ = std::fs::remove_file(&path);
        restore_env("STEAM_API_KEY", original_api_key);
        restore_env("STEAM_ID", original_steam_id);

        assert!(err.downcast_ref::<steam::error::SteamApiError>().is_some());
    }
}
