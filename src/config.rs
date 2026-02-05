use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use std::{env, fs};

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub display: DisplayConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiConfig {
    pub steam_api_key: Option<String>,
    pub steam_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DisplayConfig {
    #[serde(default = "default_top_games")]
    pub show_top_games: usize,
    #[serde(default = "default_true")]
    pub show_recently_played: bool,
    #[serde(default = "default_true")]
    pub show_achievements: bool,
    #[serde(default = "default_true")]
    pub show_rarest: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_top_games: 5,
            show_recently_played: true,
            show_achievements: true,
            show_rarest: true,
        }
    }
}

fn default_top_games() -> usize {
    5
}

fn default_true() -> bool {
    true
}

pub struct Config {
    pub api_key: String,
    pub steam_id: String,
    #[allow(dead_code)]
    pub display: DisplayConfig,
}

const API_KEY_HELP: &str = r#"STEAM_API_KEY not set.

To get your API key:
  1. Visit https://steamcommunity.com/dev/apikey
  2. Log in with your Steam account
  3. Enter a domain name (anything works, e.g., "localhost")
  4. Copy the key and set it:

     export STEAM_API_KEY="your-api-key-here"

Or add to config file (~/.config/steamfetch/config.toml):

     [api]
     steam_api_key = "your-api-key-here"
"#;

const STEAM_ID_HELP: &str = r#"STEAM_ID not set.

To find your Steam ID:
  1. Visit https://steamid.io
  2. Enter your Steam profile URL or username
  3. Copy the "steamID64" value and set it:

     export STEAM_ID="your-steam-id-here"

Or add to config file (~/.config/steamfetch/config.toml):

     [api]
     steam_id = "your-steam-id-here"

Note: If Steam is running, STEAM_ID is auto-detected and not required.
"#;

impl Config {
    pub fn load(config_path: Option<PathBuf>) -> Result<Self> {
        let config_file = load_config_file(config_path)?;

        // Environment variables take precedence over config file
        let api_key = env::var("STEAM_API_KEY")
            .ok()
            .or(config_file.api.steam_api_key)
            .context(API_KEY_HELP)?;

        let steam_id = env::var("STEAM_ID")
            .ok()
            .or(config_file.api.steam_id)
            .context(STEAM_ID_HELP)?;

        Ok(Self {
            api_key,
            steam_id,
            display: config_file.display,
        })
    }
}

fn load_config_file(custom_path: Option<PathBuf>) -> Result<ConfigFile> {
    let path = custom_path.or_else(default_config_path);

    match path {
        Some(p) if p.exists() => {
            let content = fs::read_to_string(&p)
                .with_context(|| format!("Failed to read config file: {}", p.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", p.display()))
        }
        _ => Ok(ConfigFile::default()),
    }
}

fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("steamfetch").join("config.toml"))
}

/// Returns the default config file path for display purposes
pub fn config_path() -> Option<PathBuf> {
    default_config_path()
}
