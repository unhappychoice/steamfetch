use anyhow::{Context, Result};
use futures::future::join_all;
use reqwest::Client;

use super::models::{
    AchievementStats, AchievementsResponse, GameStat, GlobalAchievementsResponse,
    OwnedGamesResponse, PlayerSummaryResponse, RarestAchievement, SteamStats,
};

const BASE_URL: &str = "https://api.steampowered.com";

pub struct SteamClient {
    client: Client,
    api_key: String,
    steam_id: String,
    verbose: bool,
}

impl SteamClient {
    pub fn new(api_key: String, steam_id: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            steam_id,
            verbose: false,
        }
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub async fn fetch_stats(&self) -> Result<SteamStats> {
        // Sequential requests to avoid Steam API rate limiting
        let player = self.fetch_player().await?;
        let games = self.fetch_owned_games().await?;
        let steam_level = self.fetch_steam_level().await?;
        let recently_played = self.fetch_recently_played().await?;

        let unplayed = games
            .games
            .iter()
            .filter(|g| g.playtime_forever == 0)
            .count() as u32;
        let total_playtime = games.games.iter().map(|g| g.playtime_forever).sum();
        let top_games = self.extract_top_games(&games);
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

    /// Fetch stats for a specific list of AppIDs (used with Steamworks SDK)
    pub async fn fetch_stats_for_appids(
        &self,
        appids: &[u32],
        username: &str,
    ) -> Result<SteamStats> {
        // Sequential requests to avoid Steam API rate limiting
        let player = self.fetch_player().await?;
        let games = self.fetch_owned_games_for_appids(appids).await?;
        let steam_level = self.fetch_steam_level().await?;
        let recently_played = self.fetch_recently_played().await?;

        // Count unplayed from API response (only games with playtime data)
        let unplayed = games
            .games
            .iter()
            .filter(|g| g.playtime_forever == 0)
            .count() as u32;

        let total_playtime = games.games.iter().map(|g| g.playtime_forever).sum();
        let top_games = self.extract_top_games(&games);

        // Create OwnedGamesData for achievement scanning with native appid count
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
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch player summary")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if self.verbose {
            eprintln!("[verbose] API response status: {}", status);
            eprintln!(
                "[verbose] API response body: {}",
                &body[..body.len().min(500)]
            );
        }

        let parsed: PlayerSummaryResponse =
            serde_json::from_str(&body).context("Failed to parse player summary")?;

        parsed
            .response
            .players
            .into_iter()
            .next()
            .context("Player not found")
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
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch owned games")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if self.verbose {
            eprintln!("[verbose] Owned games API status: {}", status);
            eprintln!(
                "[verbose] Owned games API body: {}",
                &body[..body.len().min(500)]
            );
        }

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
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch steam level")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if self.verbose {
            eprintln!("[verbose] Steam level API status: {}", status);
        }

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
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch recently played")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        if self.verbose {
            eprintln!("[verbose] Recently played API status: {}", status);
        }

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
        let all_games = self.get_all_games(games);
        let results = join_all(
            all_games
                .iter()
                .map(|(appid, name)| self.fetch_game_achievements(*appid, name.clone())),
        )
        .await;

        let mut total_achieved = 0u32;
        let mut total_possible = 0u32;
        let mut perfect_games = 0u32;
        let mut rarest: Option<RarestAchievement> = None;

        for result in results.into_iter().flatten() {
            total_achieved += result.achieved;
            total_possible += result.total;

            if result.achieved == result.total && result.total > 0 {
                perfect_games += 1;
            }

            if let Some(r) = result.rarest {
                match &rarest {
                    None => rarest = Some(r),
                    Some(current) if r.percent < current.percent => rarest = Some(r),
                    _ => {}
                }
            }
        }

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

        // Find rarest achievement - try matching by apiname first, then by lowercase
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

    fn extract_top_games(&self, games: &super::models::OwnedGamesData) -> Vec<GameStat> {
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

    fn get_all_games(&self, games: &super::models::OwnedGamesData) -> Vec<(u32, String)> {
        games
            .games
            .iter()
            .map(|g| {
                (
                    g.appid,
                    g.name.clone().unwrap_or_else(|| format!("App {}", g.appid)),
                )
            })
            .collect()
    }
}

struct GameAchievementResult {
    achieved: u32,
    total: u32,
    rarest: Option<RarestAchievement>,
}
