use serde::Deserialize;

// Owned Games API
#[derive(Debug, Deserialize)]
pub struct OwnedGamesResponse {
    pub response: OwnedGamesData,
}

#[derive(Debug, Deserialize)]
pub struct OwnedGamesData {
    pub game_count: u32,
    #[serde(default)]
    pub games: Vec<Game>,
}

#[derive(Debug, Deserialize)]
pub struct Game {
    pub appid: u32,
    pub name: Option<String>,
    pub playtime_forever: u32,
    #[serde(default)]
    pub playtime_2weeks: u32,
    #[serde(default)]
    pub rtime_last_played: u64,
}

// Recently Played Games API
#[derive(Debug, Deserialize)]
pub struct RecentlyPlayedResponse {
    pub response: RecentlyPlayedData,
}

#[derive(Debug, Deserialize)]
pub struct RecentlyPlayedData {
    #[serde(default)]
    pub games: Vec<Game>,
}

// Steam Level API
#[derive(Debug, Deserialize)]
pub struct SteamLevelResponse {
    pub response: SteamLevelData,
}

#[derive(Debug, Deserialize)]
pub struct SteamLevelData {
    pub player_level: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct PlayerSummaryResponse {
    pub response: PlayerSummaryData,
}

#[derive(Debug, Deserialize)]
pub struct PlayerSummaryData {
    pub players: Vec<Player>,
}

#[derive(Debug, Deserialize)]
pub struct Player {
    pub personaname: String,
    pub timecreated: Option<u64>,
    pub avatarfull: Option<String>,
}

// Achievements API
#[derive(Debug, Deserialize)]
pub struct AchievementsResponse {
    pub playerstats: AchievementsData,
}

#[derive(Debug, Deserialize)]
pub struct AchievementsData {
    #[serde(default)]
    pub achievements: Vec<Achievement>,
}

#[derive(Debug, Deserialize)]
pub struct Achievement {
    pub apiname: String,
    pub achieved: u8,
    pub name: Option<String>,
}

// Global Achievement Percentages API
#[derive(Debug, Deserialize)]
pub struct GlobalAchievementsResponse {
    pub achievementpercentages: GlobalAchievementsData,
}

#[derive(Debug, Deserialize)]
pub struct GlobalAchievementsData {
    #[serde(default)]
    pub achievements: Vec<GlobalAchievement>,
}

#[derive(Debug, Deserialize)]
pub struct GlobalAchievement {
    pub name: String,
    #[serde(deserialize_with = "deserialize_percent")]
    pub percent: f64,
}

fn deserialize_percent<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct PercentVisitor;

    impl<'de> Visitor<'de> for PercentVisitor {
        type Value = f64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a float or string representing a float")
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<f64, E> {
            v.parse().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(PercentVisitor)
}

// Aggregated Stats
#[derive(Debug)]
pub struct SteamStats {
    pub username: String,
    pub game_count: u32,
    pub unplayed_count: u32,
    pub total_playtime_minutes: u32,
    pub top_games: Vec<GameStat>,
    pub achievement_stats: Option<AchievementStats>,
    pub account_created: Option<u64>,
    pub steam_level: Option<u32>,
    pub recently_played: Vec<GameStat>,
    pub avatar_url: Option<String>,
}

#[derive(Debug)]
pub struct AchievementStats {
    pub total_achieved: u32,
    pub total_possible: u32,
    pub perfect_games: u32,
    pub rarest: Option<RarestAchievement>,
}

#[derive(Debug)]
pub struct RarestAchievement {
    pub name: String,
    pub game: String,
    pub percent: f64,
}

#[derive(Debug)]
pub struct GameStat {
    pub name: String,
    pub playtime_minutes: u32,
}

impl SteamStats {
    pub fn playtime_hours(&self) -> u32 {
        self.total_playtime_minutes / 60
    }
}

impl GameStat {
    pub fn playtime_hours(&self) -> u32 {
        self.playtime_minutes / 60
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_steam_stats(total_playtime_minutes: u32) -> SteamStats {
        SteamStats {
            username: "tester".to_string(),
            game_count: 0,
            unplayed_count: 0,
            total_playtime_minutes,
            top_games: Vec::new(),
            achievement_stats: None,
            account_created: None,
            steam_level: None,
            recently_played: Vec::new(),
            avatar_url: None,
        }
    }

    #[test]
    fn test_steam_stats_playtime_hours_exact() {
        let stats = make_steam_stats(180);
        assert_eq!(stats.playtime_hours(), 3);
    }

    #[test]
    fn test_steam_stats_playtime_hours_floors() {
        let stats = make_steam_stats(125);
        assert_eq!(stats.playtime_hours(), 2);
    }

    #[test]
    fn test_steam_stats_playtime_hours_zero() {
        let stats = make_steam_stats(0);
        assert_eq!(stats.playtime_hours(), 0);
    }

    #[test]
    fn test_steam_stats_playtime_hours_under_one_hour() {
        let stats = make_steam_stats(45);
        assert_eq!(stats.playtime_hours(), 0);
    }

    #[test]
    fn test_game_stat_playtime_hours_exact() {
        let game = GameStat {
            name: "Game A".to_string(),
            playtime_minutes: 600,
        };
        assert_eq!(game.playtime_hours(), 10);
    }

    #[test]
    fn test_game_stat_playtime_hours_floors() {
        let game = GameStat {
            name: "Game B".to_string(),
            playtime_minutes: 119,
        };
        assert_eq!(game.playtime_hours(), 1);
    }

    #[test]
    fn test_deserialize_owned_games_response_with_games() {
        let json = r#"{
            "response": {
                "game_count": 2,
                "games": [
                    {"appid": 10, "name": "Counter-Strike", "playtime_forever": 120},
                    {"appid": 20, "playtime_forever": 60, "playtime_2weeks": 30, "rtime_last_played": 12345}
                ]
            }
        }"#;
        let parsed: OwnedGamesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.response.game_count, 2);
        assert_eq!(parsed.response.games.len(), 2);
        assert_eq!(parsed.response.games[0].appid, 10);
        assert_eq!(
            parsed.response.games[0].name.as_deref(),
            Some("Counter-Strike")
        );
        assert_eq!(parsed.response.games[0].playtime_forever, 120);
        // Defaults applied to missing fields
        assert_eq!(parsed.response.games[0].playtime_2weeks, 0);
        assert_eq!(parsed.response.games[0].rtime_last_played, 0);
        // Second game has no name
        assert!(parsed.response.games[1].name.is_none());
        assert_eq!(parsed.response.games[1].playtime_2weeks, 30);
        assert_eq!(parsed.response.games[1].rtime_last_played, 12345);
    }

    #[test]
    fn test_deserialize_owned_games_response_without_games_key() {
        let json = r#"{"response": {"game_count": 0}}"#;
        let parsed: OwnedGamesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.response.game_count, 0);
        assert!(parsed.response.games.is_empty());
    }

    #[test]
    fn test_deserialize_recently_played_default_games() {
        let json = r#"{"response": {}}"#;
        let parsed: RecentlyPlayedResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.response.games.is_empty());
    }

    #[test]
    fn test_deserialize_steam_level_response_with_level() {
        let json = r#"{"response": {"player_level": 42}}"#;
        let parsed: SteamLevelResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.response.player_level, Some(42));
    }

    #[test]
    fn test_deserialize_steam_level_response_null_level() {
        let json = r#"{"response": {"player_level": null}}"#;
        let parsed: SteamLevelResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.response.player_level.is_none());
    }

    #[test]
    fn test_deserialize_player_summary_response() {
        let json = r#"{
            "response": {
                "players": [
                    {"personaname": "alice", "timecreated": 1577836800, "avatarfull": "http://example.com/a.jpg"}
                ]
            }
        }"#;
        let parsed: PlayerSummaryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.response.players.len(), 1);
        assert_eq!(parsed.response.players[0].personaname, "alice");
        assert_eq!(parsed.response.players[0].timecreated, Some(1577836800));
        assert_eq!(
            parsed.response.players[0].avatarfull.as_deref(),
            Some("http://example.com/a.jpg")
        );
    }

    #[test]
    fn test_deserialize_player_summary_minimal_player() {
        let json = r#"{"response": {"players": [{"personaname": "bob"}]}}"#;
        let parsed: PlayerSummaryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.response.players[0].personaname, "bob");
        assert!(parsed.response.players[0].timecreated.is_none());
        assert!(parsed.response.players[0].avatarfull.is_none());
    }

    #[test]
    fn test_deserialize_achievements_response() {
        let json = r#"{
            "playerstats": {
                "achievements": [
                    {"apiname": "ACH_1", "achieved": 1, "name": "First"},
                    {"apiname": "ACH_2", "achieved": 0}
                ]
            }
        }"#;
        let parsed: AchievementsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.playerstats.achievements.len(), 2);
        assert_eq!(parsed.playerstats.achievements[0].apiname, "ACH_1");
        assert_eq!(parsed.playerstats.achievements[0].achieved, 1);
        assert_eq!(
            parsed.playerstats.achievements[0].name.as_deref(),
            Some("First")
        );
        assert!(parsed.playerstats.achievements[1].name.is_none());
    }

    #[test]
    fn test_deserialize_achievements_response_default_empty() {
        let json = r#"{"playerstats": {}}"#;
        let parsed: AchievementsResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.playerstats.achievements.is_empty());
    }

    #[test]
    fn test_deserialize_global_achievements_percent_as_float() {
        let json = r#"{
            "achievementpercentages": {
                "achievements": [
                    {"name": "ACH_1", "percent": 12.5},
                    {"name": "ACH_2", "percent": 0.0}
                ]
            }
        }"#;
        let parsed: GlobalAchievementsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.achievementpercentages.achievements.len(), 2);
        assert!(
            (parsed.achievementpercentages.achievements[0].percent - 12.5).abs() < f64::EPSILON
        );
        assert!(parsed.achievementpercentages.achievements[1].percent.abs() < f64::EPSILON);
    }

    #[test]
    fn test_deserialize_global_achievements_percent_as_string() {
        let json = r#"{
            "achievementpercentages": {
                "achievements": [
                    {"name": "ACH_S", "percent": "7.25"}
                ]
            }
        }"#;
        let parsed: GlobalAchievementsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.achievementpercentages.achievements.len(), 1);
        assert!(
            (parsed.achievementpercentages.achievements[0].percent - 7.25).abs() < f64::EPSILON
        );
    }

    #[test]
    fn test_deserialize_global_achievements_percent_invalid_string_errors() {
        let json = r#"{
            "achievementpercentages": {
                "achievements": [{"name": "ACH_BAD", "percent": "not-a-number"}]
            }
        }"#;
        let result: Result<GlobalAchievementsResponse, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_global_achievements_default_empty() {
        let json = r#"{"achievementpercentages": {}}"#;
        let parsed: GlobalAchievementsResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.achievementpercentages.achievements.is_empty());
    }

    #[test]
    fn test_deserialize_global_achievements_percent_unsupported_type_errors() {
        // `percent` as a bool triggers serde's default `visit_bool`, which
        // calls `Visitor::expecting()` to format the type-mismatch error.
        let json = r#"{
            "achievementpercentages": {
                "achievements": [{"name": "ACH_BOOL", "percent": true}]
            }
        }"#;
        let err = serde_json::from_str::<GlobalAchievementsResponse>(json)
            .expect_err("bool percent should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("a float or string representing a float"),
            "expected `expecting` message in error, got: {msg}",
        );
    }

    #[test]
    fn test_deserialize_global_achievements_percent_null_errors() {
        // `null` similarly cannot be coerced; the default `visit_unit`
        // rejects with the visitor's `expecting` text.
        let json = r#"{
            "achievementpercentages": {
                "achievements": [{"name": "ACH_NULL", "percent": null}]
            }
        }"#;
        let err = serde_json::from_str::<GlobalAchievementsResponse>(json)
            .expect_err("null percent should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("a float or string representing a float"),
            "expected `expecting` message in error, got: {msg}",
        );
    }

    #[test]
    fn test_deserialize_global_achievements_percent_from_slice_float() {
        // `from_slice` drives `deserialize_percent` through serde_json's
        // `SliceRead`, a different monomorphization than `from_str`'s
        // `StrRead` — exercising a previously-uncovered instantiation of
        // the visitor's `visit_f64` arm.
        let bytes = br#"{
            "achievementpercentages": {
                "achievements": [{"name": "ACH_BYTES_F", "percent": 33.5}]
            }
        }"#;
        let parsed: GlobalAchievementsResponse =
            serde_json::from_slice(bytes).expect("from_slice should parse float percent");
        assert_eq!(parsed.achievementpercentages.achievements.len(), 1);
        assert!(
            (parsed.achievementpercentages.achievements[0].percent - 33.5).abs() < f64::EPSILON
        );
    }

    #[test]
    fn test_deserialize_global_achievements_percent_from_slice_string() {
        // Same `SliceRead` driver, but routes through `visit_str`.
        let bytes = br#"{
            "achievementpercentages": {
                "achievements": [{"name": "ACH_BYTES_S", "percent": "9.5"}]
            }
        }"#;
        let parsed: GlobalAchievementsResponse =
            serde_json::from_slice(bytes).expect("from_slice should parse string percent");
        assert!((parsed.achievementpercentages.achievements[0].percent - 9.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_deserialize_global_achievements_percent_from_slice_invalid_string_errors() {
        // `SliceRead` instantiation of the invalid-string path; the visitor's
        // `visit_str` parses the input and surfaces the parse failure.
        let bytes = br#"{
            "achievementpercentages": {
                "achievements": [{"name": "ACH_BYTES_BAD", "percent": "not-a-number"}]
            }
        }"#;
        let result: Result<GlobalAchievementsResponse, _> = serde_json::from_slice(bytes);
        assert!(result.is_err());
    }
}
