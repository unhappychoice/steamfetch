use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAchievement {
    pub last_played: u64,
    pub achieved: u32,
    pub total: u32,
    pub rarest_name: Option<String>,
    pub rarest_percent: Option<f64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AchievementCache {
    games: HashMap<u32, CachedAchievement>,
}

impl AchievementCache {
    pub fn load() -> Self {
        cache_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = cache_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(path, serde_json::to_string(self).unwrap_or_default());
        }
    }

    pub fn get(&self, appid: u32, last_played: u64) -> Option<&CachedAchievement> {
        self.games
            .get(&appid)
            .filter(|c| c.last_played == last_played)
    }

    pub fn set(
        &mut self,
        appid: u32,
        last_played: u64,
        achieved: u32,
        total: u32,
        rarest: Option<(&str, f64)>,
    ) {
        self.games.insert(
            appid,
            CachedAchievement {
                last_played,
                achieved,
                total,
                rarest_name: rarest.map(|(n, _)| n.to_string()),
                rarest_percent: rarest.map(|(_, p)| p),
            },
        );
    }
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("steamfetch").join("achievements.json"))
}
