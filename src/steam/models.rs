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
}

// Recently Played Games API
#[derive(Debug, Deserialize)]
pub struct RecentlyPlayedResponse {
    pub response: RecentlyPlayedData,
}

#[derive(Debug, Deserialize)]
pub struct RecentlyPlayedData {
    pub total_count: Option<u32>,
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
    pub loccountrycode: Option<String>,
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
    pub country: Option<String>,
    pub steam_level: Option<u32>,
    pub recently_played: Vec<GameStat>,
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

    pub fn playtime_days(&self) -> u32 {
        self.total_playtime_minutes / 60 / 24
    }
}

impl GameStat {
    pub fn playtime_hours(&self) -> u32 {
        self.playtime_minutes / 60
    }
}
