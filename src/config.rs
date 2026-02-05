use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub api_key: String,
    pub steam_id: String,
}

const API_KEY_HELP: &str = r#"STEAM_API_KEY not set.

To get your API key:
  1. Visit https://steamcommunity.com/dev/apikey
  2. Log in with your Steam account
  3. Enter a domain name (anything works, e.g., "localhost")
  4. Copy the key and set it:

     export STEAM_API_KEY="your-api-key-here"
"#;

const STEAM_ID_HELP: &str = r#"STEAM_ID not set.

To find your Steam ID:
  1. Visit https://steamid.io
  2. Enter your Steam profile URL or username
  3. Copy the "steamID64" value and set it:

     export STEAM_ID="your-steam-id-here"

Note: If Steam is running, STEAM_ID is auto-detected and not required.
"#;

impl Config {
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("STEAM_API_KEY").context(API_KEY_HELP)?;
        let steam_id = env::var("STEAM_ID").context(STEAM_ID_HELP)?;

        Ok(Self { api_key, steam_id })
    }
}
