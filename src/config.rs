use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub api_key: String,
    pub steam_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("STEAM_API_KEY")
            .context("STEAM_API_KEY not set. Get one at: https://steamcommunity.com/dev/apikey")?;
        let steam_id =
            env::var("STEAM_ID").context("STEAM_ID not set. Find yours at: https://steamid.io")?;

        Ok(Self { api_key, steam_id })
    }
}
