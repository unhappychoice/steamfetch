use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::io::{self, Write};
use std::time::Duration;

use super::error::SteamApiError;
use super::models::{
    AchievementStats, AchievementsResponse, GameStat, GlobalAchievementsResponse,
    OwnedGamesResponse, PlayerSummaryResponse, RarestAchievement, SteamStats,
};
use crate::cache::AchievementCache;

const BASE_URL: &str = "https://api.steampowered.com";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

fn print_status(msg: &str) {
    eprint!("\r\x1b[K{}", msg);
    let _ = io::stderr().flush();
}

fn clear_status() {
    eprint!("\r\x1b[K");
    let _ = io::stderr().flush();
}

pub struct SteamClient {
    client: Client,
    api_key: String,
    steam_id: String,
    verbose: bool,
    timeout: Duration,
}

impl SteamClient {
    pub fn new(api_key: String, steam_id: String) -> Self {
        let timeout = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let client = build_http_client(timeout);

        Self {
            client,
            api_key,
            steam_id,
            verbose: false,
            timeout,
        }
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self.client = build_http_client(self.timeout);
        self
    }

    pub async fn fetch_stats(&self) -> Result<SteamStats> {
        print_status("Fetching player info...");
        let player = self.fetch_player().await?;

        print_status("Fetching owned games...");
        let games = self.fetch_owned_games().await?;

        print_status("Fetching account details...");
        let (steam_level, recently_played) = self.fetch_optional_details().await;

        let unplayed = games
            .games
            .iter()
            .filter(|g| g.playtime_forever == 0)
            .count() as u32;
        let total_playtime = games.games.iter().map(|g| g.playtime_forever).sum();
        let top_games = extract_top_games(&games);
        let achievement_stats = self.fetch_achievement_stats(&games).await;

        Ok(SteamStats {
            username: player.personaname,
            game_count: games.game_count,
            unplayed_count: unplayed,
            total_playtime_minutes: total_playtime,
            top_games,
            achievement_stats,
            account_created: player.timecreated,
            steam_level,
            recently_played,
        })
    }

    pub async fn fetch_stats_for_appids(
        &self,
        appids: &[u32],
        username: &str,
    ) -> Result<SteamStats> {
        print_status("Fetching player info...");
        let player = self.fetch_player().await?;

        print_status("Fetching owned games...");
        let games = self.fetch_owned_games_for_appids(appids).await?;

        print_status("Fetching account details...");
        let (steam_level, recently_played) = self.fetch_optional_details().await;

        let unplayed = games
            .games
            .iter()
            .filter(|g| g.playtime_forever == 0)
            .count() as u32;

        let total_playtime = games.games.iter().map(|g| g.playtime_forever).sum();
        let top_games = extract_top_games(&games);

        let games_with_playtime: std::collections::HashMap<u32, _> =
            games.games.iter().map(|g| (g.appid, g)).collect();

        let native_games = super::models::OwnedGamesData {
            game_count: appids.len() as u32,
            games: appids
                .iter()
                .map(|&appid| super::models::Game {
                    appid,
                    name: games_with_playtime.get(&appid).and_then(|g| g.name.clone()),
                    playtime_forever: games_with_playtime
                        .get(&appid)
                        .map_or(0, |g| g.playtime_forever),
                    playtime_2weeks: 0,
                    rtime_last_played: games_with_playtime
                        .get(&appid)
                        .map_or(0, |g| g.rtime_last_played),
                })
                .collect(),
        };

        let achievement_stats = self.fetch_achievement_stats(&native_games).await;

        Ok(SteamStats {
            username: username.to_string(),
            game_count: appids.len() as u32,
            unplayed_count: unplayed,
            total_playtime_minutes: total_playtime,
            top_games,
            achievement_stats,
            account_created: player.timecreated,
            steam_level,
            recently_played,
        })
    }

    async fn fetch_optional_details(&self) -> (Option<u32>, Vec<GameStat>) {
        let steam_level = match self.fetch_steam_level().await {
            Ok(level) => level,
            Err(e) => {
                if self.verbose {
                    eprintln!("[verbose] Failed to fetch steam level: {}", e);
                }
                None
            }
        };

        let recently_played = match self.fetch_recently_played().await {
            Ok(games) => games,
            Err(e) => {
                if self.verbose {
                    eprintln!("[verbose] Failed to fetch recently played: {}", e);
                }
                Vec::new()
            }
        };

        (steam_level, recently_played)
    }

    async fn fetch_player(&self) -> Result<super::models::Player> {
        let url = format!(
            "{}/ISteamUser/GetPlayerSummaries/v2/?key={}&steamids={}",
            BASE_URL, self.api_key, self.steam_id
        );
        if self.verbose {
            eprintln!(
                "[verbose] Fetching player summary for Steam ID: {}",
                self.steam_id
            );
        }

        let body = self.request_with_retry(&url, "player summary").await?;
        detect_api_error(&body, self.verbose)?;

        let parsed: PlayerSummaryResponse =
            serde_json::from_str(&body).context("Failed to parse player summary")?;

        parsed
            .response
            .players
            .into_iter()
            .next()
            .ok_or_else(|| SteamApiError::PlayerNotFound.into())
    }

    async fn fetch_owned_games(&self) -> Result<super::models::OwnedGamesData> {
        self.fetch_owned_games_filtered(None).await
    }

    async fn fetch_owned_games_for_appids(
        &self,
        appids: &[u32],
    ) -> Result<super::models::OwnedGamesData> {
        const CHUNK_SIZE: usize = 100;

        let mut all_games = Vec::new();
        for chunk in appids.chunks(CHUNK_SIZE) {
            let data = self.fetch_owned_games_filtered(Some(chunk)).await?;
            all_games.extend(data.games);
        }

        Ok(super::models::OwnedGamesData {
            game_count: all_games.len() as u32,
            games: all_games,
        })
    }

    async fn fetch_owned_games_filtered(
        &self,
        appids_filter: Option<&[u32]>,
    ) -> Result<super::models::OwnedGamesData> {
        let mut url = format!(
            "{}/IPlayerService/GetOwnedGames/v1/?key={}&steamid={}&include_appinfo=1&include_played_free_games=1",
            BASE_URL, self.api_key, self.steam_id
        );

        if let Some(appids) = appids_filter {
            for (i, appid) in appids.iter().enumerate() {
                url.push_str(&format!("&appids_filter%5B{}%5D={}", i, appid));
            }
        }

        if self.verbose {
            eprintln!("[verbose] Fetching owned games...");
        }

        let body = self.request_with_retry(&url, "owned games").await?;
        detect_private_profile(&body)?;

        let parsed: OwnedGamesResponse =
            serde_json::from_str(&body).context("Failed to parse owned games")?;

        Ok(parsed.response)
    }

    async fn fetch_steam_level(&self) -> Result<Option<u32>> {
        let url = format!(
            "{}/IPlayerService/GetSteamLevel/v1/?key={}&steamid={}",
            BASE_URL, self.api_key, self.steam_id
        );
        if self.verbose {
            eprintln!("[verbose] Fetching steam level...");
        }

        let body = self.request_with_retry(&url, "steam level").await?;

        let parsed: super::models::SteamLevelResponse =
            serde_json::from_str(&body).context("Failed to parse steam level")?;

        Ok(parsed.response.player_level)
    }

    async fn fetch_recently_played(&self) -> Result<Vec<GameStat>> {
        let url = format!(
            "{}/IPlayerService/GetRecentlyPlayedGames/v1/?key={}&steamid={}&count=5",
            BASE_URL, self.api_key, self.steam_id
        );
        if self.verbose {
            eprintln!("[verbose] Fetching recently played...");
        }

        let body = self.request_with_retry(&url, "recently played").await?;

        let parsed: super::models::RecentlyPlayedResponse =
            serde_json::from_str(&body).context("Failed to parse recently played")?;

        Ok(parsed
            .response
            .games
            .into_iter()
            .map(|g| GameStat {
                name: g.name.unwrap_or_else(|| format!("App {}", g.appid)),
                playtime_minutes: g.playtime_2weeks,
            })
            .collect())
    }

    async fn fetch_achievement_stats(
        &self,
        games: &super::models::OwnedGamesData,
    ) -> Option<AchievementStats> {
        let mut cache = AchievementCache::load();
        let all_games: Vec<_> = games.games.iter().collect();
        let total_games = all_games.len();

        clear_status();
        let pb = ProgressBar::new(total_games as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("\r{msg} [{bar:30.cyan/blue}] {pos}/{len}")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message("Achievements (0 cached, 0 fetched)");

        let mut total_achieved = 0u32;
        let mut total_possible = 0u32;
        let mut perfect_games = 0u32;
        let mut rarest_candidates: Vec<RarestAchievement> = Vec::new();
        let mut cached_count = 0u32;
        let mut fetched_count = 0u32;

        for game in &all_games {
            let game_name = game
                .name
                .clone()
                .unwrap_or_else(|| format!("App {}", game.appid));

            if let Some(cached) = cache.get(game.appid, game.rtime_last_played) {
                cached_count += 1;
                total_achieved += cached.achieved;
                total_possible += cached.total;
                if cached.achieved == cached.total && cached.total > 0 {
                    perfect_games += 1;
                }
                if let (Some(name), Some(percent)) = (&cached.rarest_name, cached.rarest_percent) {
                    rarest_candidates.push(RarestAchievement {
                        name: name.clone(),
                        game: game_name,
                        percent,
                    });
                }
                pb.inc(1);
                pb.set_message(format!(
                    "Achievements ({} cached, {} fetched)",
                    cached_count, fetched_count
                ));
                continue;
            }

            fetched_count += 1;
            pb.inc(1);
            pb.set_message(format!(
                "Achievements ({} cached, {} fetched)",
                cached_count, fetched_count
            ));

            if let Some(result) = self
                .fetch_game_achievements(game.appid, game_name.clone())
                .await
            {
                total_achieved += result.achieved;
                total_possible += result.total;
                if result.achieved == result.total && result.total > 0 {
                    perfect_games += 1;
                }

                let rarest_for_cache = result.rarest.as_ref().map(|r| (r.name.as_str(), r.percent));
                cache.set(
                    game.appid,
                    game.rtime_last_played,
                    result.achieved,
                    result.total,
                    rarest_for_cache,
                );

                if let Some(r) = result.rarest {
                    rarest_candidates.push(r);
                }
            }
        }

        pb.finish_and_clear();
        clear_status();
        cache.save();

        let rarest = rarest_candidates.into_iter().min_by(|a, b| {
            a.percent
                .partial_cmp(&b.percent)
                .unwrap()
                .then_with(|| a.game.cmp(&b.game))
                .then_with(|| a.name.cmp(&b.name))
        });

        (total_possible > 0).then_some(AchievementStats {
            total_achieved,
            total_possible,
            perfect_games,
            rarest,
        })
    }

    async fn fetch_game_achievements(
        &self,
        appid: u32,
        game_name: String,
    ) -> Option<GameAchievementResult> {
        let (player_achievements, global_percentages) = tokio::join!(
            self.fetch_player_achievements(appid),
            self.fetch_global_percentages(appid)
        );

        let achievements = player_achievements.ok()?;
        let percentages = global_percentages.ok().unwrap_or_default();

        let achieved = achievements.iter().filter(|a| a.achieved == 1).count() as u32;
        let total = achievements.len() as u32;

        let rarest = achievements
            .iter()
            .filter(|a| a.achieved == 1)
            .filter_map(|a| {
                let percent = percentages
                    .get(&a.apiname)
                    .or_else(|| percentages.get(&a.apiname.to_uppercase()))?;
                Some(RarestAchievement {
                    name: a.name.clone().unwrap_or_else(|| a.apiname.clone()),
                    game: game_name.clone(),
                    percent: *percent,
                })
            })
            .min_by(|a, b| a.percent.partial_cmp(&b.percent).unwrap());

        Some(GameAchievementResult {
            achieved,
            total,
            rarest,
        })
    }

    async fn fetch_player_achievements(
        &self,
        appid: u32,
    ) -> Result<Vec<super::models::Achievement>> {
        let url = format!(
            "{}/ISteamUserStats/GetPlayerAchievements/v1/?key={}&steamid={}&appid={}&l=english",
            BASE_URL, self.api_key, self.steam_id, appid
        );
        Ok(self
            .client
            .get(&url)
            .send()
            .await?
            .json::<AchievementsResponse>()
            .await?
            .playerstats
            .achievements)
    }

    async fn fetch_global_percentages(
        &self,
        appid: u32,
    ) -> Result<std::collections::HashMap<String, f64>> {
        let url = format!(
            "{}/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2/?gameid={}",
            BASE_URL, appid
        );
        let response: GlobalAchievementsResponse =
            self.client.get(&url).send().await?.json().await?;

        Ok(response
            .achievementpercentages
            .achievements
            .into_iter()
            .map(|a| (a.name, a.percent))
            .collect())
    }

    async fn request_with_retry(&self, url: &str, context: &str) -> Result<String> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1));
                if self.verbose {
                    eprintln!(
                        "[verbose] Retry {}/{} for {} (waiting {}ms)",
                        attempt,
                        MAX_RETRIES - 1,
                        context,
                        backoff.as_millis()
                    );
                }
                tokio::time::sleep(backoff).await;
            }

            match self.client.get(url).send().await {
                Ok(response) => {
                    let status = response.status();

                    if self.verbose {
                        eprintln!("[verbose] {} API status: {}", context, status);
                    }

                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        last_error = Some(SteamApiError::RateLimited);
                        continue;
                    }

                    if status == reqwest::StatusCode::FORBIDDEN {
                        return Err(SteamApiError::InvalidApiKey.into());
                    }

                    if status.is_server_error() {
                        let body = response.text().await.unwrap_or_default();
                        last_error = Some(SteamApiError::ApiError {
                            status: status.as_u16(),
                            message: body,
                        });
                        continue;
                    }

                    let body = response
                        .text()
                        .await
                        .with_context(|| format!("Failed to read {} response body", context))?;

                    if self.verbose {
                        eprintln!(
                            "[verbose] {} response body: {}",
                            context,
                            &body[..body.len().min(500)]
                        );
                    }

                    return Ok(body);
                }
                Err(e) => {
                    let api_error = if e.is_timeout() {
                        SteamApiError::Timeout
                    } else {
                        SteamApiError::NetworkError(e.to_string())
                    };

                    if api_error.is_retryable() {
                        if self.verbose {
                            eprintln!("[verbose] {} request failed: {}", context, api_error);
                        }
                        last_error = Some(api_error);
                        continue;
                    }

                    return Err(api_error.into());
                }
            }
        }

        Err(last_error
            .map(anyhow::Error::from)
            .unwrap_or_else(|| anyhow::anyhow!("Failed to fetch {} after retries", context)))
    }
}

fn build_http_client(timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .pool_idle_timeout(Duration::from_secs(1))
        .pool_max_idle_per_host(0)
        .build()
        .unwrap_or_else(|_| Client::new())
}

fn extract_top_games(games: &super::models::OwnedGamesData) -> Vec<GameStat> {
    let mut sorted: Vec<_> = games.games.iter().collect();
    sorted.sort_by(|a, b| b.playtime_forever.cmp(&a.playtime_forever));

    sorted
        .into_iter()
        .take(5)
        .map(|g| GameStat {
            name: g.name.clone().unwrap_or_else(|| format!("App {}", g.appid)),
            playtime_minutes: g.playtime_forever,
        })
        .collect()
}

fn detect_api_error(body: &str, verbose: bool) -> Result<()> {
    if body.contains("\"players\":[]") || body.contains("\"players\": []") {
        return Err(SteamApiError::PlayerNotFound.into());
    }

    if body.contains("Forbidden") || body.contains("Access is denied") {
        if verbose {
            eprintln!("[verbose] API key rejected by Steam");
        }
        return Err(SteamApiError::InvalidApiKey.into());
    }

    Ok(())
}

fn detect_private_profile(body: &str) -> Result<()> {
    // Private profiles return an empty or minimal response for owned games
    let parsed: Result<OwnedGamesResponse, _> = serde_json::from_str(body);
    match parsed {
        Ok(resp) if resp.response.games.is_empty() && resp.response.game_count == 0 => {
            // Could be a private profile or truly no games.
            // Check if the response body looks like a minimal/empty response
            if !body.contains("\"games\"") {
                return Err(SteamApiError::PrivateProfile.into());
            }
            Ok(())
        }
        Err(_) => {
            // Parse failure on owned games often indicates private profile
            if body.contains("\"game_count\":0") || !body.contains("\"games\"") {
                return Err(SteamApiError::PrivateProfile.into());
            }
            Err(anyhow::anyhow!("Failed to parse owned games response"))
        }
        _ => Ok(()),
    }
}

struct GameAchievementResult {
    achieved: u32,
    total: u32,
    rarest: Option<RarestAchievement>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_api_error_empty_players() {
        let body = r#"{"response":{"players":[]}}"#;
        let result = detect_api_error(body, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_api_error_with_player() {
        let body = r#"{"response":{"players":[{"personaname":"test"}]}}"#;
        let result = detect_api_error(body, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_detect_private_profile_no_games_key() {
        let body = r#"{"response":{"game_count":0}}"#;
        let result = detect_private_profile(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_private_profile_with_games() {
        let body = r#"{"response":{"game_count":1,"games":[{"appid":220,"name":"Half-Life 2","playtime_forever":100}]}}"#;
        let result = detect_private_profile(body);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_top_games_sorts_by_playtime() {
        let games = super::super::models::OwnedGamesData {
            game_count: 3,
            games: vec![
                super::super::models::Game {
                    appid: 1,
                    name: Some("A".to_string()),
                    playtime_forever: 10,
                    playtime_2weeks: 0,
                    rtime_last_played: 0,
                },
                super::super::models::Game {
                    appid: 2,
                    name: Some("B".to_string()),
                    playtime_forever: 100,
                    playtime_2weeks: 0,
                    rtime_last_played: 0,
                },
                super::super::models::Game {
                    appid: 3,
                    name: Some("C".to_string()),
                    playtime_forever: 50,
                    playtime_2weeks: 0,
                    rtime_last_played: 0,
                },
            ],
        };
        let top = extract_top_games(&games);
        assert_eq!(top[0].name, "B");
        assert_eq!(top[1].name, "C");
        assert_eq!(top[2].name, "A");
    }
}
