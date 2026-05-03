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

    /// Load only API key (for Native SDK mode where steam_id is auto-detected)
    pub fn load_api_key_only(config_path: Option<PathBuf>) -> Result<String> {
        let config_file = load_config_file(config_path)?;

        env::var("STEAM_API_KEY")
            .ok()
            .or(config_file.api.steam_api_key)
            .context(API_KEY_HELP)
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
        Some(p) => {
            create_default_config(&p)?;
            Ok(ConfigFile::default())
        }
        _ => Ok(ConfigFile::default()),
    }
}

fn create_default_config(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }

    fs::write(path, DEFAULT_CONFIG)
        .with_context(|| format!("Failed to write config file: {}", path.display()))?;

    eprintln!("Created config file: {}", path.display());
    Ok(())
}

const DEFAULT_CONFIG: &str = r#"# steamfetch configuration file
# https://github.com/unhappychoice/steamfetch

[api]
# Get your API key at: https://steamcommunity.com/dev/apikey
# steam_api_key = "YOUR_API_KEY"

# Find your Steam ID at: https://steamid.io
# Note: If Steam is running, STEAM_ID is auto-detected
# steam_id = "YOUR_STEAM_ID"

[display]
# Number of top played games to show
# show_top_games = 5

# Show recently played games (last 2 weeks)
# show_recently_played = true

# Show achievement statistics
# show_achievements = true

# Show rarest achievement
# show_rarest = true
"#;

fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("steamfetch").join("config.toml"))
}

/// Returns the default config file path for display purposes
pub fn config_path() -> Option<PathBuf> {
    default_config_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        env::temp_dir().join(format!(
            "steamfetch-test-{}-{}-{}-{}",
            label,
            std::process::id(),
            nanos,
            n
        ))
    }

    #[test]
    fn test_display_config_default_values() {
        let d = DisplayConfig::default();
        assert_eq!(d.show_top_games, 5);
        assert!(d.show_recently_played);
        assert!(d.show_achievements);
        assert!(d.show_rarest);
    }

    #[test]
    fn test_default_helpers() {
        assert_eq!(default_top_games(), 5);
        assert!(default_true());
    }

    #[test]
    fn test_config_file_parses_empty_to_defaults() {
        let parsed: ConfigFile = toml::from_str("").expect("empty toml should parse");
        assert!(parsed.api.steam_api_key.is_none());
        assert!(parsed.api.steam_id.is_none());
        assert_eq!(parsed.display.show_top_games, 5);
        assert!(parsed.display.show_recently_played);
    }

    #[test]
    fn test_config_file_parses_api_section() {
        let toml_str = r#"
[api]
steam_api_key = "abc123"
steam_id = "76561197960265728"
"#;
        let parsed: ConfigFile = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.api.steam_api_key.as_deref(), Some("abc123"));
        assert_eq!(parsed.api.steam_id.as_deref(), Some("76561197960265728"));
        assert_eq!(parsed.display.show_top_games, 5);
    }

    #[test]
    fn test_config_file_parses_display_overrides() {
        let toml_str = r#"
[display]
show_top_games = 10
show_recently_played = false
show_achievements = false
show_rarest = false
"#;
        let parsed: ConfigFile = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.display.show_top_games, 10);
        assert!(!parsed.display.show_recently_played);
        assert!(!parsed.display.show_achievements);
        assert!(!parsed.display.show_rarest);
    }

    #[test]
    fn test_config_file_partial_display_keeps_other_defaults() {
        let toml_str = r#"
[display]
show_top_games = 3
"#;
        let parsed: ConfigFile = toml::from_str(toml_str).expect("should parse");
        assert_eq!(parsed.display.show_top_games, 3);
        assert!(parsed.display.show_recently_played);
        assert!(parsed.display.show_achievements);
        assert!(parsed.display.show_rarest);
    }

    #[test]
    fn test_load_config_file_reads_existing_toml() {
        let path = unique_temp_path("read");
        fs::write(
            &path,
            r#"
[api]
steam_api_key = "from-file"

[display]
show_top_games = 7
"#,
        )
        .unwrap();

        let cfg = load_config_file(Some(path.clone())).expect("load should succeed");
        assert_eq!(cfg.api.steam_api_key.as_deref(), Some("from-file"));
        assert_eq!(cfg.display.show_top_games, 7);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_load_config_file_creates_default_when_missing() {
        let dir = unique_temp_path("missing-dir");
        let path = dir.join("config.toml");
        assert!(!path.exists());

        let cfg = load_config_file(Some(path.clone())).expect("load should succeed");
        assert!(path.exists(), "default config file should be written");
        assert!(cfg.api.steam_api_key.is_none());
        assert_eq!(cfg.display.show_top_games, 5);

        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains("[api]"));
        assert!(written.contains("[display]"));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_config_file_invalid_toml_errors() {
        let path = unique_temp_path("invalid");
        fs::write(&path, "this is = not [valid toml").unwrap();

        let err = load_config_file(Some(path.clone())).expect_err("invalid toml should error");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Failed to parse config file"),
            "expected parse-failure context, got: {}",
            msg
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_create_default_config_writes_file_and_directory() {
        let dir = unique_temp_path("create-dir");
        let path = dir.join("nested").join("config.toml");
        assert!(!dir.exists());

        create_default_config(&path).expect("should create default config");
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("steamfetch configuration file"));
        assert!(content.contains("show_top_games"));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(path.parent().unwrap());
        let _ = fs::remove_dir(&dir);
    }
}
