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

    #[test]
    fn test_config_path_shape_when_available() {
        // `dirs::config_dir()` may return None on exotic platforms; only
        // assert when Some (mirrors the cache_path test pattern).
        if let Some(path) = config_path() {
            assert!(path.ends_with("steamfetch/config.toml"));
        }
    }

    mod env_tests {
        use super::super::*;
        use std::env;
        use std::fs;
        use std::sync::Mutex;
        use std::time::{SystemTime, UNIX_EPOCH};

        // STEAM_API_KEY and STEAM_ID are process-wide; serialize mutations
        // across this submodule so parallel test threads don't race.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        fn unique_path(label: &str) -> std::path::PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            env::temp_dir().join(format!(
                "steamfetch-cfg-env-test-{}-{}-{}",
                label,
                std::process::id(),
                nanos
            ))
        }

        struct EnvScope {
            key: &'static str,
            prev: Option<String>,
        }

        impl EnvScope {
            fn save(key: &'static str) -> Self {
                let prev = env::var(key).ok();
                env::remove_var(key);
                Self { key, prev }
            }

            fn set(key: &'static str, value: &str) -> Self {
                let prev = env::var(key).ok();
                env::set_var(key, value);
                Self { key, prev }
            }
        }

        impl Drop for EnvScope {
            fn drop(&mut self) {
                match &self.prev {
                    Some(v) => env::set_var(self.key, v),
                    None => env::remove_var(self.key),
                }
            }
        }

        #[test]
        fn test_load_prefers_env_vars_over_config_file() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::set("STEAM_API_KEY", "env-key");
            let _sid = EnvScope::set("STEAM_ID", "env-sid");

            let path = unique_path("env-wins");
            fs::write(
                &path,
                r#"
[api]
steam_api_key = "file-key"
steam_id = "file-sid"
"#,
            )
            .unwrap();

            let cfg = Config::load(Some(path.clone())).expect("load should succeed");
            assert_eq!(cfg.api_key, "env-key");
            assert_eq!(cfg.steam_id, "env-sid");

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_falls_back_to_config_file_when_env_unset() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::save("STEAM_API_KEY");
            let _sid = EnvScope::save("STEAM_ID");

            let path = unique_path("file-wins");
            fs::write(
                &path,
                r#"
[api]
steam_api_key = "file-key"
steam_id = "file-sid"
"#,
            )
            .unwrap();

            let cfg = Config::load(Some(path.clone())).expect("load should succeed");
            assert_eq!(cfg.api_key, "file-key");
            assert_eq!(cfg.steam_id, "file-sid");

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_errors_with_help_when_api_key_missing() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::save("STEAM_API_KEY");
            let _sid = EnvScope::set("STEAM_ID", "env-sid");

            let path = unique_path("no-api-key");
            fs::write(&path, "").unwrap();

            let err = Config::load(Some(path.clone()))
                .err()
                .expect("missing api key should error");
            let msg = format!("{:#}", err);
            assert!(
                msg.contains("STEAM_API_KEY not set"),
                "expected api-key help, got: {}",
                msg
            );

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_errors_with_help_when_steam_id_missing() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::set("STEAM_API_KEY", "env-key");
            let _sid = EnvScope::save("STEAM_ID");

            let path = unique_path("no-sid");
            fs::write(&path, "").unwrap();

            let err = Config::load(Some(path.clone()))
                .err()
                .expect("missing steam id should error");
            let msg = format!("{:#}", err);
            assert!(
                msg.contains("STEAM_ID not set"),
                "expected steam-id help, got: {}",
                msg
            );

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_api_key_only_prefers_env() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::set("STEAM_API_KEY", "env-key");

            let path = unique_path("api-env");
            fs::write(
                &path,
                r#"
[api]
steam_api_key = "file-key"
"#,
            )
            .unwrap();

            let key = Config::load_api_key_only(Some(path.clone())).expect("should succeed");
            assert_eq!(key, "env-key");

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_api_key_only_falls_back_to_file() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::save("STEAM_API_KEY");

            let path = unique_path("api-file");
            fs::write(
                &path,
                r#"
[api]
steam_api_key = "file-key"
"#,
            )
            .unwrap();

            let key = Config::load_api_key_only(Some(path.clone())).expect("should succeed");
            assert_eq!(key, "file-key");

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_api_key_only_errors_when_missing() {
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::save("STEAM_API_KEY");

            let path = unique_path("api-missing");
            fs::write(&path, "").unwrap();

            let err = Config::load_api_key_only(Some(path.clone()))
                .expect_err("missing api key should error");
            let msg = format!("{:#}", err);
            assert!(
                msg.contains("STEAM_API_KEY not set"),
                "expected api-key help, got: {}",
                msg
            );

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_propagates_load_config_file_error() {
            // Invalid TOML in the config file makes `load_config_file` return
            // Err, which `Config::load` propagates via `?` (line 94).
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::set("STEAM_API_KEY", "env-key");
            let _sid = EnvScope::set("STEAM_ID", "env-sid");

            let path = unique_path("load-bad-toml");
            fs::write(&path, "this is = not [valid toml").unwrap();

            let err = Config::load(Some(path.clone()))
                .err()
                .expect("invalid toml should error");
            let msg = format!("{:#}", err);
            assert!(
                msg.contains("Failed to parse config file"),
                "expected parse-failure context, got: {}",
                msg
            );

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_load_api_key_only_propagates_load_config_file_error() {
            // Same `?` propagation in `load_api_key_only` (line 116).
            let _guard = ENV_LOCK.lock().unwrap();
            let _api = EnvScope::set("STEAM_API_KEY", "env-key");

            let path = unique_path("api-bad-toml");
            fs::write(&path, "this is = not [valid toml").unwrap();

            let err = Config::load_api_key_only(Some(path.clone()))
                .expect_err("invalid toml should error");
            let msg = format!("{:#}", err);
            assert!(
                msg.contains("Failed to parse config file"),
                "expected parse-failure context, got: {}",
                msg
            );

            let _ = fs::remove_file(&path);
        }

        #[test]
        fn test_envscope_drop_restores_previous_value_when_present() {
            // The other tests in this module always start with the env var
            // unset, so EnvScope::Drop's `None` arm is the only one ever hit.
            // Pre-seed the var so prev is Some, exercising the `Some(v)` arm
            // that restores the prior value on Drop.
            let _guard = ENV_LOCK.lock().unwrap();
            let outer_prev = env::var("STEAM_API_KEY").ok();

            let sentinel = "preexisting-sentinel-value";
            env::set_var("STEAM_API_KEY", sentinel);

            {
                let _scope = EnvScope::set("STEAM_API_KEY", "scoped-value");
                assert_eq!(env::var("STEAM_API_KEY").unwrap(), "scoped-value");
            }

            // Drop ran the `Some(v) => env::set_var(self.key, v)` branch.
            assert_eq!(env::var("STEAM_API_KEY").unwrap(), sentinel);

            match outer_prev {
                Some(v) => env::set_var("STEAM_API_KEY", v),
                None => env::remove_var("STEAM_API_KEY"),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_create_default_config_errors_when_parent_is_file() {
        // A regular file used as a directory component makes `create_dir_all`
        // fail; this exercises the `with_context` closure (lines 146–147).
        let blocker = unique_temp_path("blocker");
        fs::write(&blocker, "not a directory").unwrap();

        let path = blocker.join("nested").join("config.toml");
        let err = create_default_config(&path)
            .expect_err("create_dir_all should fail when parent is a file");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Failed to create config directory"),
            "expected create-dir context, got: {}",
            msg
        );

        let _ = fs::remove_file(&blocker);
    }

    #[cfg(unix)]
    #[test]
    fn test_create_default_config_errors_when_target_is_directory() {
        // The parent already exists, so `create_dir_all` succeeds, but the
        // target path itself resolves to an existing directory — `fs::write`
        // then fails with `Is a directory`, exercising the write-context
        // closure on lines 149-150.
        let dir = unique_temp_path("write-fail-target");
        fs::create_dir_all(&dir).unwrap();

        let err = create_default_config(&dir).expect_err("writing to a directory path should fail");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Failed to write config file"),
            "expected write-context, got: {}",
            msg
        );

        let _ = fs::remove_dir(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_load_config_file_read_failure_returns_error() {
        // Pass a directory as the path: `p.exists()` is true (directories
        // exist), but `fs::read_to_string` fails, hitting the read-context
        // closure on line 131.
        let dir = unique_temp_path("read-fail-dir");
        fs::create_dir_all(&dir).unwrap();

        let err = load_config_file(Some(dir.clone()))
            .expect_err("reading a directory as a file should error");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Failed to read config file"),
            "expected read-context, got: {}",
            msg
        );

        let _ = fs::remove_dir(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_load_config_file_propagates_create_default_failure() {
        // The supplied path doesn't exist (so we enter the `Some(p) =>` arm
        // that calls `create_default_config(&p)?`), but its parent directory
        // is a regular file — `create_dir_all` fails inside
        // `create_default_config`, and the `?` on line 136 propagates that
        // error out of `load_config_file`. The other `Some(p) =>` test only
        // covers the success path of the same `?`.
        let blocker = unique_temp_path("create-fail-blocker");
        fs::write(&blocker, "not a directory").unwrap();
        let path = blocker.join("nested").join("config.toml");
        assert!(!path.exists());

        let err = load_config_file(Some(path))
            .expect_err("create_default_config failure should propagate");
        let msg = format!("{:#}", err);
        assert!(
            msg.contains("Failed to create config directory"),
            "expected create-dir context to surface via load_config_file, got: {}",
            msg
        );

        let _ = fs::remove_file(&blocker);
    }
}
