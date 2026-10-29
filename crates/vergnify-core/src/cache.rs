// @author Cursor

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::ReleaseLine;

pub const TTL_SECS: u64 = 60 * 60 * 24;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct VersionCache {
    #[serde(default)]
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CacheEntry {
    lines: Vec<ReleaseLine>,
    fetched_at: u64,
}

impl VersionCache {
    pub fn load(path: &Path) -> Self {
        let Ok(bytes) = fs::read(path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, bytes).map_err(|error| error.to_string())?;
        fs::rename(&tmp, path).map_err(|error| error.to_string())
    }

    pub fn fresh(&self, id: &str, now: u64) -> Option<Vec<ReleaseLine>> {
        let entry = self.entries.get(id)?;
        (now.saturating_sub(entry.fetched_at) < TTL_SECS).then(|| entry.lines.clone())
    }

    pub fn any(&self, id: &str) -> Option<Vec<ReleaseLine>> {
        self.entries.get(id).map(|entry| entry.lines.clone())
    }

    pub fn insert(&mut self, id: String, lines: Vec<ReleaseLine>, now: u64) {
        self.entries.insert(
            id,
            CacheEntry {
                lines,
                fetched_at: now,
            },
        );
    }
}

pub fn default_cache_path() -> PathBuf {
    cache_dir().join("latest.json")
}

pub fn cache_dir() -> PathBuf {
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(cache).join("vergnify");
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            return home.join("Library/Caches/vergnify");
        }
        return home.join(".cache/vergnify");
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("vergnify");
    }
    PathBuf::from(".vergnify-cache")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LineSupport;

    fn sample_lines() -> Vec<ReleaseLine> {
        vec![ReleaseLine {
            id: "stable".into(),
            label: "stable".into(),
            latest: "2.47.0".into(),
            support: LineSupport::Stable,
        }]
    }

    #[test]
    fn fresh_entry_expires_after_ttl() {
        let mut cache = VersionCache::default();
        cache.insert("node".into(), sample_lines(), 1_000);
        assert_eq!(
            cache.fresh("node", 1_000 + TTL_SECS - 1),
            Some(sample_lines())
        );
        assert_eq!(cache.fresh("node", 1_000 + TTL_SECS), None);
        assert_eq!(cache.any("node"), Some(sample_lines()));
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("vergnify-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("latest.json");
        let mut cache = VersionCache::default();
        cache.insert("git".into(), sample_lines(), 50);
        cache.save(&path).unwrap();
        let loaded = VersionCache::load(&path);
        assert_eq!(loaded.any("git"), Some(sample_lines()));
        let _ = fs::remove_dir_all(&dir);
    }
}
