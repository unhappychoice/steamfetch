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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_cache_is_empty() {
        let cache = AchievementCache::default();
        assert!(cache.get(123, 0).is_none());
    }

    #[test]
    fn test_set_then_get_returns_entry_when_last_played_matches() {
        let mut cache = AchievementCache::default();
        cache.set(42, 1000, 5, 10, Some(("Rare", 1.5)));
        let entry = cache.get(42, 1000).expect("entry should exist");
        assert_eq!(entry.last_played, 1000);
        assert_eq!(entry.achieved, 5);
        assert_eq!(entry.total, 10);
        assert_eq!(entry.rarest_name.as_deref(), Some("Rare"));
        assert_eq!(entry.rarest_percent, Some(1.5));
    }

    #[test]
    fn test_get_returns_none_when_last_played_mismatches() {
        let mut cache = AchievementCache::default();
        cache.set(42, 1000, 5, 10, None);
        assert!(cache.get(42, 999).is_none());
    }

    #[test]
    fn test_get_returns_none_for_unknown_appid() {
        let mut cache = AchievementCache::default();
        cache.set(42, 1000, 5, 10, None);
        assert!(cache.get(43, 1000).is_none());
    }

    #[test]
    fn test_set_without_rarest_clears_rarest_fields() {
        let mut cache = AchievementCache::default();
        cache.set(7, 500, 1, 2, None);
        let entry = cache.get(7, 500).unwrap();
        assert!(entry.rarest_name.is_none());
        assert!(entry.rarest_percent.is_none());
    }

    #[test]
    fn test_set_overwrites_existing_entry() {
        let mut cache = AchievementCache::default();
        cache.set(1, 100, 1, 5, Some(("Old", 50.0)));
        cache.set(1, 200, 3, 5, Some(("New", 10.0)));
        assert!(cache.get(1, 100).is_none());
        let entry = cache.get(1, 200).unwrap();
        assert_eq!(entry.achieved, 3);
        assert_eq!(entry.rarest_name.as_deref(), Some("New"));
        assert_eq!(entry.rarest_percent, Some(10.0));
    }

    #[test]
    fn test_serde_roundtrip_preserves_entries() {
        let mut cache = AchievementCache::default();
        cache.set(11, 1234, 8, 12, Some(("Legend", 0.25)));
        cache.set(22, 5678, 0, 50, None);
        let json = serde_json::to_string(&cache).unwrap();
        let restored: AchievementCache = serde_json::from_str(&json).unwrap();
        let a = restored.get(11, 1234).unwrap();
        assert_eq!(a.achieved, 8);
        assert_eq!(a.total, 12);
        assert_eq!(a.rarest_name.as_deref(), Some("Legend"));
        assert_eq!(a.rarest_percent, Some(0.25));
        let b = restored.get(22, 5678).unwrap();
        assert_eq!(b.total, 50);
        assert!(b.rarest_name.is_none());
    }
}
