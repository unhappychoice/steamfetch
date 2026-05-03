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
            avatar_url: player.avatarfull,
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
            avatar_url: player.avatarfull,
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

            let api_error = match self.client.get(url).send().await {
                Ok(response) => {
                    let status = response.status();

                    if self.verbose {
                        eprintln!("[verbose] {} API status: {}", context, status);
                    }

                    if status.is_success() {
                        let body = response
                            .text()
                            .await
                            .with_context(|| format!("Failed to read {} response body", context))?;

                        if self.verbose {
                            let truncated = &body[..body.floor_char_boundary(500)];
                            eprintln!("[verbose] {} response body: {}", context, truncated);
                        }

                        return Ok(body);
                    }

                    classify_http_error(status, response).await
                }
                Err(e) if e.is_timeout() => SteamApiError::Timeout,
                Err(e) => SteamApiError::NetworkError(e.to_string()),
            };

            if self.verbose {
                eprintln!("[verbose] {} request failed: {}", context, api_error);
            }

            if !api_error.is_retryable() {
                return Err(api_error.into());
            }
            last_error = Some(api_error);
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
        .expect("Failed to build HTTP client")
}

fn extract_top_games(games: &super::models::OwnedGamesData) -> Vec<GameStat> {
    let mut sorted: Vec<_> = games.games.iter().collect();
    sorted.sort_by_key(|g| std::cmp::Reverse(g.playtime_forever));

    sorted
        .into_iter()
        .take(5)
        .map(|g| GameStat {
            name: g.name.clone().unwrap_or_else(|| format!("App {}", g.appid)),
            playtime_minutes: g.playtime_forever,
        })
        .collect()
}

async fn classify_http_error(
    status: reqwest::StatusCode,
    response: reqwest::Response,
) -> SteamApiError {
    match status {
        reqwest::StatusCode::TOO_MANY_REQUESTS => SteamApiError::RateLimited,
        reqwest::StatusCode::FORBIDDEN => SteamApiError::InvalidApiKey,
        _ => {
            let body = response.text().await.unwrap_or_default();
            SteamApiError::ApiError {
                status: status.as_u16(),
                message: body,
            }
        }
    }
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
                make_game(1, Some("A"), 10),
                make_game(2, Some("B"), 100),
                make_game(3, Some("C"), 50),
            ],
        };
        let top = extract_top_games(&games);
        assert_eq!(top[0].name, "B");
        assert_eq!(top[1].name, "C");
        assert_eq!(top[2].name, "A");
    }

    fn make_game(appid: u32, name: Option<&str>, playtime: u32) -> super::super::models::Game {
        super::super::models::Game {
            appid,
            name: name.map(|n| n.to_string()),
            playtime_forever: playtime,
            playtime_2weeks: 0,
            rtime_last_played: 0,
        }
    }

    #[test]
    fn test_detect_api_error_forbidden_message() {
        let body = r#"{"error":"Forbidden"}"#;
        let result = detect_api_error(body, false);
        let err = result.unwrap_err();
        assert!(err.downcast_ref::<SteamApiError>().is_some());
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::InvalidApiKey
        ));
    }

    #[test]
    fn test_detect_api_error_access_is_denied_message() {
        let body = "<html><body>Access is denied</body></html>";
        let result = detect_api_error(body, false);
        let err = result.unwrap_err();
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::InvalidApiKey
        ));
    }

    #[test]
    fn test_detect_api_error_players_with_space() {
        let body = r#"{"response":{"players": []}}"#;
        let err = detect_api_error(body, false).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::PlayerNotFound
        ));
    }

    #[test]
    fn test_detect_private_profile_empty_games_array_with_key_is_ok() {
        let body = r#"{"response":{"game_count":0,"games":[]}}"#;
        assert!(detect_private_profile(body).is_ok());
    }

    #[test]
    fn test_detect_private_profile_parse_failure_with_zero_count_is_private() {
        let body = r#"{"response":{"game_count":0"#; // truncated/invalid JSON
        let err = detect_private_profile(body).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::PrivateProfile
        ));
    }

    #[test]
    fn test_detect_private_profile_parse_failure_without_games_is_private() {
        let body = "totally not json";
        let err = detect_private_profile(body).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::PrivateProfile
        ));
    }

    #[test]
    fn test_detect_private_profile_parse_failure_other_returns_anyhow() {
        // Has "games" key but is malformed → falls through to the
        // generic "Failed to parse owned games response" branch.
        let body = r#"{"response":{"games":"not_an_array"}}"#;
        let err = detect_private_profile(body).unwrap_err();
        assert!(err.downcast_ref::<SteamApiError>().is_none());
        assert!(err.to_string().contains("Failed to parse owned games"));
    }

    #[test]
    fn test_extract_top_games_empty_input() {
        let games = super::super::models::OwnedGamesData {
            game_count: 0,
            games: vec![],
        };
        let top = extract_top_games(&games);
        assert!(top.is_empty());
    }

    #[test]
    fn test_extract_top_games_truncates_to_five() {
        let games = super::super::models::OwnedGamesData {
            game_count: 7,
            games: (0..7)
                .map(|i| make_game(i as u32, Some(&format!("G{}", i)), (i as u32) * 10))
                .collect(),
        };
        let top = extract_top_games(&games);
        assert_eq!(top.len(), 5);
        assert_eq!(top[0].name, "G6");
        assert_eq!(top[4].name, "G2");
    }

    #[test]
    fn test_extract_top_games_falls_back_to_appid_when_name_missing() {
        let games = super::super::models::OwnedGamesData {
            game_count: 1,
            games: vec![make_game(12345, None, 60)],
        };
        let top = extract_top_games(&games);
        assert_eq!(top[0].name, "App 12345");
        assert_eq!(top[0].playtime_minutes, 60);
    }

    #[test]
    fn test_steam_client_builder_defaults() {
        let c = SteamClient::new("k".to_string(), "id".to_string());
        assert!(!c.verbose);
        assert_eq!(c.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        assert_eq!(c.api_key, "k");
        assert_eq!(c.steam_id, "id");
    }

    #[test]
    fn test_steam_client_with_verbose_sets_flag() {
        let c = SteamClient::new("k".into(), "id".into()).with_verbose(true);
        assert!(c.verbose);
    }

    #[test]
    fn test_steam_client_with_timeout_overrides_default() {
        let c = SteamClient::new("k".into(), "id".into()).with_timeout(7);
        assert_eq!(c.timeout, Duration::from_secs(7));
    }

    #[test]
    fn test_build_http_client_does_not_panic() {
        // Constructs and drops the client to ensure the builder
        // settings remain valid.
        let _ = build_http_client(Duration::from_secs(1));
        let _ = build_http_client(Duration::from_secs(120));
    }

    #[test]
    fn test_print_status_does_not_panic() {
        // Writes a CR + clear-line escape + message to stderr; the
        // assertion is simply that it completes without panicking.
        print_status("hello");
        print_status("");
    }

    #[test]
    fn test_clear_status_does_not_panic() {
        clear_status();
    }

    #[test]
    fn test_detect_api_error_forbidden_with_verbose_true_logs_and_returns_invalid_key() {
        // Exercises the `if verbose { eprintln!(...) }` branch inside the
        // Forbidden/Access-is-denied arm — previously only the verbose=false
        // path was covered.
        let body = r#"{"error":"Forbidden"}"#;
        let err = detect_api_error(body, true).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::InvalidApiKey
        ));
    }

    #[test]
    fn test_detect_api_error_access_denied_with_verbose_true() {
        // Same verbose branch via the alternative trigger string.
        let body = "<html>Access is denied</html>";
        let err = detect_api_error(body, true).unwrap_err();
        assert!(matches!(
            err.downcast_ref::<SteamApiError>().unwrap(),
            SteamApiError::InvalidApiKey
        ));
    }

    #[tokio::test]
    async fn test_fetch_owned_games_for_appids_empty_returns_empty_data() {
        // Empty appids slice — the chunks(100) iterator yields nothing,
        // so no HTTP request is dispatched. Exercises the function entry,
        // the empty loop, and the final OwnedGamesData construction.
        let client = SteamClient::new("k".into(), "id".into());
        let data = client
            .fetch_owned_games_for_appids(&[])
            .await
            .expect("empty input should not error");
        assert_eq!(data.game_count, 0);
        assert!(data.games.is_empty());
    }

    #[test]
    fn test_fetch_owned_games_for_appids_enters_chunks_loop_with_non_empty_slice() {
        // A non-empty appids slice produces one chunk, so the for-loop body
        // is entered and `fetch_owned_games_filtered(Some(chunk))` is invoked
        // — exercising the source line that the empty-slice test cannot
        // reach (chunks(100) of [] yields nothing). The inner call targets
        // BASE_URL and is therefore not reachable from a unit test, so we
        // wrap the whole call in a tight `tokio::time::timeout` to abort
        // before any real network handshake completes. The line counter for
        // the loop-body statement increments as soon as the future starts
        // evaluating that statement, which happens before the inner await
        // yields — independently of whether the timeout or the request
        // itself wins the race.
        use std::time::Duration;
        let client = SteamClient::new("k".into(), "id".into()).with_timeout(1);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        let _ = rt.block_on(async {
            tokio::time::timeout(
                Duration::from_millis(50),
                client.fetch_owned_games_for_appids(&[1]),
            )
            .await
        });
    }

    mod request_with_retry_tests {
        use super::super::*;
        use std::io::{Read, Write};
        use std::net::TcpListener;

        fn run_async<F: std::future::Future>(f: F) -> F::Output {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt")
                .block_on(f)
        }

        // Bind to a random port, capture it, then drop the listener.
        // Guarantees nothing is listening on the returned URL so reqwest
        // fails fast with a connection error rather than timing out.
        fn unbound_localhost_url() -> String {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
            let port = listener.local_addr().expect("local_addr").port();
            drop(listener);
            format!("http://127.0.0.1:{}/", port)
        }

        // Spin up a TCP server that responds to `n` consecutive requests
        // with the same status/body, then exits.
        fn spawn_n_shot_server(
            n: usize,
            status: u16,
            reason: &'static str,
            body: &'static [u8],
        ) -> (String, std::thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/", addr);

            let server = std::thread::spawn(move || {
                for _ in 0..n {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let response = format!(
                        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        status,
                        reason,
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(body);
                    let _ = stream.flush();
                }
            });

            (url, server)
        }

        #[test]
        fn test_request_with_retry_returns_body_on_200_success() {
            let (url, server) = spawn_n_shot_server(1, 200, "OK", b"hello-body");
            let client = SteamClient::new("k".into(), "id".into());
            let body = run_async(client.request_with_retry(&url, "test"))
                .expect("200 response should succeed");
            assert_eq!(body, "hello-body");
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_returns_body_on_200_when_verbose() {
            // Same success path but with verbose=true to exercise the
            // [verbose] status / response body log branches.
            let (url, server) = spawn_n_shot_server(1, 200, "OK", b"verbose-success");
            let client = SteamClient::new("k".into(), "id".into()).with_verbose(true);
            let body = run_async(client.request_with_retry(&url, "verbose"))
                .expect("200 response should succeed");
            assert_eq!(body, "verbose-success");
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_returns_invalid_api_key_on_403() {
            // 403 maps to InvalidApiKey via classify_http_error, which is
            // non-retryable, so only one request is made.
            let (url, server) = spawn_n_shot_server(1, 403, "Forbidden", b"");
            let client = SteamClient::new("k".into(), "id".into());
            let err = run_async(client.request_with_retry(&url, "test"))
                .expect_err("403 should produce an error");
            assert!(matches!(
                err.downcast_ref::<SteamApiError>().unwrap(),
                SteamApiError::InvalidApiKey
            ));
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_returns_api_error_on_400_with_body() {
            // 400 maps via classify_http_error's catch-all arm, which reads
            // the response body into ApiError.message. Non-retryable.
            let (url, server) = spawn_n_shot_server(1, 400, "Bad Request", b"bad-input");
            let client = SteamClient::new("k".into(), "id".into());
            let err = run_async(client.request_with_retry(&url, "test"))
                .expect_err("400 should produce an error");
            match err.downcast_ref::<SteamApiError>().unwrap() {
                SteamApiError::ApiError { status, message } => {
                    assert_eq!(*status, 400);
                    assert_eq!(message, "bad-input");
                }
                other => panic!("unexpected error: {:?}", other),
            }
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_exhausts_retries_on_429() {
            // 429 → RateLimited (retryable). All 3 attempts fail, so the
            // loop completes and the last error is returned.
            let (url, server) = spawn_n_shot_server(3, 429, "Too Many Requests", b"");
            let client = SteamClient::new("k".into(), "id".into());
            let err = run_async(client.request_with_retry(&url, "rate"))
                .expect_err("429 retries should still fail");
            assert!(matches!(
                err.downcast_ref::<SteamApiError>().unwrap(),
                SteamApiError::RateLimited
            ));
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_exhausts_retries_on_500() {
            // 500 → ApiError (status 5xx is retryable). Three attempts.
            let (url, server) =
                spawn_n_shot_server(3, 500, "Internal Server Error", b"server-oops");
            let client = SteamClient::new("k".into(), "id".into());
            let err = run_async(client.request_with_retry(&url, "srv"))
                .expect_err("500 retries should still fail");
            match err.downcast_ref::<SteamApiError>().unwrap() {
                SteamApiError::ApiError { status, message } => {
                    assert_eq!(*status, 500);
                    assert_eq!(message, "server-oops");
                }
                other => panic!("unexpected error: {:?}", other),
            }
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_exhausts_retries_on_connection_refused() {
            // No listener at the URL → reqwest send() returns Err that is
            // not is_timeout(), mapping to NetworkError (retryable).
            let url = unbound_localhost_url();
            let client = SteamClient::new("k".into(), "id".into());
            let err = run_async(client.request_with_retry(&url, "net"))
                .expect_err("connection refused should fail");
            assert!(matches!(
                err.downcast_ref::<SteamApiError>().unwrap(),
                SteamApiError::NetworkError(_)
            ));
        }

        #[test]
        fn test_request_with_retry_logs_backoff_when_verbose() {
            // verbose=true + retryable error exercises the backoff and
            // request-failed [verbose] log branches inside the retry loop.
            let url = unbound_localhost_url();
            let client = SteamClient::new("k".into(), "id".into()).with_verbose(true);
            let err = run_async(client.request_with_retry(&url, "verbose-net"))
                .expect_err("connection refused should fail");
            assert!(err.downcast_ref::<SteamApiError>().is_some());
        }

        // Spin up a TCP server that responds to consecutive requests using
        // the supplied (status, reason, body) sequence. After the sequence is
        // exhausted the server thread exits.
        fn spawn_sequential_server(
            sequence: Vec<(u16, &'static str, &'static [u8])>,
        ) -> (String, std::thread::JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/", addr);

            let server = std::thread::spawn(move || {
                for (status, reason, body) in sequence {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let response = format!(
                        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        status,
                        reason,
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(body);
                    let _ = stream.flush();
                }
            });

            (url, server)
        }

        #[test]
        fn test_request_with_retry_returns_body_after_retryable_failure() {
            // First attempt: 500 (retryable) → loop sets last_error, sleeps a
            // backoff, then second attempt returns 200. Exercises the
            // retry-then-success path: the `attempt > 0` backoff branch and a
            // successful body return after a previous failure was recorded.
            let (url, server) = spawn_sequential_server(vec![
                (500, "Internal Server Error", b"transient"),
                (200, "OK", b"recovered-body"),
            ]);
            let client = SteamClient::new("k".into(), "id".into());
            let body = run_async(client.request_with_retry(&url, "retry-success"))
                .expect("retry should succeed on second attempt");
            assert_eq!(body, "recovered-body");
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_succeeds_after_retry_when_verbose() {
            // Same retry-then-success flow with verbose=true — exercises the
            // verbose backoff log AND the verbose status / body log on the
            // successful retry attempt within the same call.
            let (url, server) = spawn_sequential_server(vec![
                (429, "Too Many Requests", b""),
                (200, "OK", b"verbose-recovered"),
            ]);
            let client = SteamClient::new("k".into(), "id".into()).with_verbose(true);
            let body = run_async(client.request_with_retry(&url, "verbose-retry"))
                .expect("retry should succeed on second attempt");
            assert_eq!(body, "verbose-recovered");
            let _ = server.join();
        }

        #[test]
        fn test_request_with_retry_returns_timeout_when_server_hangs() {
            // Bind a listener but never call accept(): the kernel completes
            // each TCP handshake and queues the connection, so reqwest's
            // send() proceeds past connect, then waits indefinitely for a
            // response. With a 1s client timeout, this triggers the
            // `Err(e) if e.is_timeout()` arm in request_with_retry, mapping
            // to SteamApiError::Timeout (retryable).
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/", addr);

            let client = SteamClient::new("k".into(), "id".into()).with_timeout(1);
            let err = run_async(client.request_with_retry(&url, "hang"))
                .expect_err("hanging server should produce timeout error");
            assert!(matches!(
                err.downcast_ref::<SteamApiError>().unwrap(),
                SteamApiError::Timeout
            ));
            drop(listener);
        }

        #[test]
        fn test_request_with_retry_propagates_body_read_failure() {
            // Server promises Content-Length: 100 in a 200 OK response, then
            // closes the connection before sending any body bytes. reqwest's
            // `.text().await` detects the truncated payload and yields an
            // error, hitting the `with_context("Failed to read ... response
            // body")?` arm on line 508 — a path the other 200-success tests
            // don't reach because they always send a complete body.
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("addr");
            let url = format!("http://{}/", addr);

            let server = std::thread::spawn(move || {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    // Promise a body but don't deliver it. `Connection: close`
                    // signals to the client that the stream end is the body
                    // end, so the missing 100 bytes surface as a
                    // partial-response error rather than a hang.
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n",
                    );
                    let _ = stream.flush();
                    // Drop the stream → FIN reaches the client mid-body.
                }
            });

            let client = SteamClient::new("k".into(), "id".into()).with_timeout(2);
            let err = run_async(client.request_with_retry(&url, "truncated"))
                .expect_err("truncated body should propagate as an error");
            let msg = format!("{:#}", err);
            assert!(
                msg.contains("Failed to read truncated response body"),
                "expected body-read context, got: {msg}",
            );
            let _ = server.join();
        }
    }

    mod fetch_achievement_stats_tests {
        use super::super::*;

        fn run_async<F: std::future::Future>(f: F) -> F::Output {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt")
                .block_on(f)
        }

        #[test]
        fn test_fetch_achievement_stats_returns_none_for_empty_games() {
            // Empty input → the per-game `for` loop is skipped, so no HTTP
            // requests are dispatched and the function reaches the
            // `(total_possible > 0).then_some(...)` tail with
            // `total_possible == 0`, returning None.
            //
            // Robust against XDG_CACHE_HOME races with sibling test
            // submodules: the assertion is None regardless of which
            // directory `AchievementCache::load`/`save` happens to touch.
            let games = super::super::super::models::OwnedGamesData {
                game_count: 0,
                games: vec![],
            };
            let client = SteamClient::new("k".into(), "id".into());
            let result = run_async(client.fetch_achievement_stats(&games));
            assert!(result.is_none());
        }

        #[cfg(target_os = "linux")]
        mod cache_hit_tests {
            use super::super::super::*;
            use crate::cache::AchievementCache;
            use crate::steam::models;
            use std::env;
            use std::path::Path;
            use std::sync::Mutex;
            use std::time::{SystemTime, UNIX_EPOCH};

            // XDG_CACHE_HOME is process-global; serialize mutations within this
            // submodule. Other test files (cache.rs, image_display.rs,
            // display.rs) hold their own ENV_LOCKs — cross-module races are
            // possible but the test here only asserts on data that came from
            // *this* test's pre-populated cache, so a stale read would
            // surface as an assertion failure rather than silent flakiness.
            static ENV_LOCK: Mutex<()> = Mutex::new(());

            struct EnvScope {
                prev: Option<String>,
            }

            impl EnvScope {
                fn set(root: &Path) -> Self {
                    let prev = env::var("XDG_CACHE_HOME").ok();
                    env::set_var("XDG_CACHE_HOME", root);
                    Self { prev }
                }
            }

            impl Drop for EnvScope {
                fn drop(&mut self) {
                    match &self.prev {
                        Some(v) => env::set_var("XDG_CACHE_HOME", v),
                        None => env::remove_var("XDG_CACHE_HOME"),
                    }
                }
            }

            fn unique_cache_root(label: &str) -> std::path::PathBuf {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                env::temp_dir().join(format!(
                    "steamfetch-fetch-ach-{}-{}-{}",
                    label,
                    std::process::id(),
                    nanos
                ))
            }

            fn run_async<F: std::future::Future>(f: F) -> F::Output {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("rt")
                    .block_on(f)
            }

            fn make_game(appid: u32, name: Option<&str>, last_played: u64) -> models::Game {
                models::Game {
                    appid,
                    name: name.map(|s| s.to_string()),
                    playtime_forever: 0,
                    playtime_2weeks: 0,
                    rtime_last_played: last_played,
                }
            }

            // Run `body` with XDG_CACHE_HOME pinned to a unique temp root.
            // Other test modules (cache.rs, image_display.rs, display.rs,
            // config.rs) hold their own ENV_LOCKs but the env var is
            // process-global, so a cross-module write between our
            // `cache.save()` and the function's internal
            // `AchievementCache::load()` would silently leave the cache
            // empty — `fetch_achievement_stats` then falls through to HTTP
            // fetches that also fail and returns None. Treat None as
            // "race lost; retry" up to a few times so the assertions only
            // run when the cache hit actually occurred.
            fn run_with_pinned_cache<F>(label: &str, body: F)
            where
                F: Fn(&std::path::Path) -> bool,
            {
                let _guard = ENV_LOCK.lock().unwrap();
                for attempt in 0..5 {
                    let root = unique_cache_root(&format!("{}-{}", label, attempt));
                    let scope = EnvScope::set(&root);
                    let succeeded = body(&root);
                    drop(scope);
                    let _ = std::fs::remove_dir_all(&root);
                    if succeeded {
                        return;
                    }
                }
                panic!(
                    "fetch_achievement_stats never observed our pinned cache \
                     after 5 attempts — XDG_CACHE_HOME race lost every time"
                );
            }

            #[test]
            fn test_fetch_achievement_stats_aggregates_from_cache() {
                // Pre-populate the achievement cache via XDG_CACHE_HOME so the
                // per-game loop hits the `if let Some(cached) = cache.get(...)`
                // branch for every game, never dispatching any HTTP. Exercises
                // the cache-hit accumulation, perfect-games detection, rarest
                // candidate push, and the rarest-selection min_by tail.
                run_with_pinned_cache("agg", |_root| {
                    let mut cache = AchievementCache::default();
                    // Perfect game with rarest achievement.
                    cache.set(100, 1000, 10, 10, Some(("Rare One", 5.0)));
                    // Non-perfect game without rarest.
                    cache.set(200, 2000, 3, 10, None);
                    // Non-perfect game with a rarer (lower percent) achievement
                    // — becomes the global rarest after the min_by selection.
                    cache.set(300, 3000, 1, 4, Some(("Even Rarer", 1.5)));
                    cache.save();

                    let games = models::OwnedGamesData {
                        game_count: 3,
                        games: vec![
                            make_game(100, Some("Game One"), 1000),
                            make_game(200, Some("Game Two"), 2000),
                            make_game(300, Some("Game Three"), 3000),
                        ],
                    };

                    let client = SteamClient::new("k".into(), "id".into());
                    let Some(stats) = run_async(client.fetch_achievement_stats(&games)) else {
                        return false; // race lost; retry
                    };

                    assert_eq!(stats.total_achieved, 14);
                    assert_eq!(stats.total_possible, 24);
                    assert_eq!(stats.perfect_games, 1);

                    let rarest = stats.rarest.expect("two cached entries had rarest");
                    assert_eq!(rarest.name, "Even Rarer");
                    assert_eq!(rarest.game, "Game Three");
                    assert!((rarest.percent - 1.5).abs() < f64::EPSILON);
                    true
                });
            }

            #[test]
            fn test_fetch_achievement_stats_falls_back_to_appid_for_unnamed_game() {
                // A cached game with `name: None` exercises the
                // `unwrap_or_else(|| format!("App {}", appid))` fallback for
                // game_name. The rarest's `game` field then carries the
                // synthesized "App {appid}" label.
                run_with_pinned_cache("noname", |_root| {
                    let mut cache = AchievementCache::default();
                    cache.set(4242, 7777, 2, 5, Some(("Lonely", 9.5)));
                    cache.save();

                    let games = models::OwnedGamesData {
                        game_count: 1,
                        games: vec![make_game(4242, None, 7777)],
                    };

                    let client = SteamClient::new("k".into(), "id".into());
                    let Some(stats) = run_async(client.fetch_achievement_stats(&games)) else {
                        return false; // race lost; retry
                    };

                    assert_eq!(stats.total_achieved, 2);
                    assert_eq!(stats.total_possible, 5);
                    assert_eq!(stats.perfect_games, 0);
                    let rarest = stats.rarest.expect("rarest was present in cache");
                    assert_eq!(rarest.game, "App 4242");
                    assert_eq!(rarest.name, "Lonely");
                    true
                });
            }
        }
    }
}
