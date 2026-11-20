//! Résolution AppID Steam par nom (Heroic GOG / Faugus / Lutris) via l’API Store + cache local.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::adapters::shared::urlencoding_lite;
use crate::domain::{steamish_app_id, GameEntry};
use crate::ports::SteamAppIdLookup;

#[derive(Debug, Default, Serialize, Deserialize)]
struct DiskCache {
    /// nom normalisé → appid (0 = introuvable, pour ne pas re-spam l’API)
    entries: HashMap<String, u32>,
}

pub struct SteamAppIdResolver {
    cache_path: PathBuf,
    memory: Mutex<HashMap<String, (u32, Instant)>>,
    timeout_secs: u64,
}

impl SteamAppIdResolver {
    pub fn new(timeout_secs: u64) -> Self {
        let cache_path = directories::ProjectDirs::from("fr", "glf", "gamefocus")
            .map(|d| d.cache_dir().join("steam_appids.json"))
            .unwrap_or_else(|| PathBuf::from("/tmp/gamefocus-steam-appids.json"));
        if let Some(parent) = cache_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        Self {
            cache_path,
            memory: Mutex::new(HashMap::new()),
            timeout_secs: timeout_secs.max(2),
        }
    }

    /// AppID Steam pour ProtonDB / jaquettes — tous launchers.
    pub fn resolve(&self, game: &GameEntry) -> Option<u32> {
        if let Some(id) = steamish_app_id(game) {
            return Some(id);
        }
        if is_launcher_shell(&game.name) {
            return None;
        }
        self.resolve_by_name(&game.name)
    }

    pub fn resolve_by_name(&self, name: &str) -> Option<u32> {
        let key = normalize_name(name);
        if key.len() < 2 {
            return None;
        }

        if let Ok(mem) = self.memory.lock() {
            if let Some((id, at)) = mem.get(&key) {
                if at.elapsed() < Duration::from_secs(3600) {
                    return if *id == 0 { None } else { Some(*id) };
                }
            }
        }

        if let Some(id) = self.load_disk().entries.get(&key).copied() {
            self.remember(&key, id);
            return if id == 0 { None } else { Some(id) };
        }

        let id = search_steam_store(name, self.timeout_secs).unwrap_or(0);
        self.remember(&key, id);
        self.save_disk_entry(&key, id);
        if id == 0 {
            None
        } else {
            Some(id)
        }
    }

    fn remember(&self, key: &str, id: u32) {
        if let Ok(mut mem) = self.memory.lock() {
            mem.insert(key.to_string(), (id, Instant::now()));
        }
    }

    fn load_disk(&self) -> DiskCache {
        fs::read_to_string(&self.cache_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save_disk_entry(&self, key: &str, id: u32) {
        let mut disk = self.load_disk();
        disk.entries.insert(key.to_string(), id);
        if let Ok(raw) = serde_json::to_string_pretty(&disk) {
            let _ = fs::write(&self.cache_path, raw);
        }
    }
}

impl SteamAppIdLookup for SteamAppIdResolver {
    fn resolve(&self, game: &GameEntry) -> Option<u32> {
        SteamAppIdResolver::resolve(self, game)
    }
}

fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Launchers / stores : pas de page ProtonDB utile.
fn is_launcher_shell(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "battle.net",
        "battlenet",
        "ea app",
        "ea desktop",
        "ubisoft connect",
        "uplay",
        "epic games launcher",
        "gog galaxy",
        "origin",
        "rockstar games launcher",
        "xbox app",
    ]
    .iter()
    .any(|s| n == *s || n.contains(s))
}

fn search_steam_store(name: &str, timeout_secs: u64) -> Option<u32> {
    let term = name.trim();
    if term.is_empty() {
        return None;
    }
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=FR",
        urlencoding_lite(term)
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("gamefocus/0.1 (GLF)")
        .build();
    let resp = agent.get(&url).call().ok()?;
    if resp.status() != 200 {
        return None;
    }
    let raw = resp.into_string().ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let items = v.get("items")?.as_array()?;
    pick_best_appid(term, items)
}

fn pick_best_appid(query: &str, items: &[Value]) -> Option<u32> {
    let q = normalize_name(query);
    let mut best: Option<(i32, u32)> = None;
    for item in items {
        let id = item.get("id").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        if id == 0 {
            continue;
        }
        let typ = item
            .get("type")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_lowercase();
        // Ignorer DLC / music / hardware
        if matches!(
            typ.as_str(),
            "dlc" | "music" | "hardware" | "video" | "demo"
        ) {
            continue;
        }
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let n = normalize_name(&name);
        let mut score = 0i32;
        if n == q {
            score += 1000;
        } else if n.starts_with(&q) || q.starts_with(&n) {
            score += 500;
        } else if n.contains(&q) || q.contains(&n) {
            score += 200;
        } else {
            // token overlap
            let qt: Vec<&str> = q.split_whitespace().collect();
            let nt: Vec<&str> = n.split_whitespace().collect();
            let common = qt.iter().filter(|t| nt.contains(t)).count() as i32;
            if common == 0 {
                continue;
            }
            score += common * 40;
        }
        if typ == "app" || typ.is_empty() {
            score += 20;
        }
        if best.map(|(s, _)| score > s).unwrap_or(true) {
            best = Some((score, id));
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_exact_name() {
        let items = vec![
            serde_json::json!({"id": 1, "name": "Other", "type": "app"}),
            serde_json::json!({"id": 1091500, "name": "Cyberpunk 2077", "type": "app"}),
        ];
        assert_eq!(pick_best_appid("Cyberpunk 2077", &items), Some(1091500));
    }

    #[test]
    fn skips_launcher_shells() {
        assert!(is_launcher_shell("Battle.net"));
        assert!(is_launcher_shell("EA App"));
        assert!(!is_launcher_shell("Soulframe"));
    }

    #[test]
    fn normalize_strips_punct() {
        assert_eq!(normalize_name("Hello: World!"), "hello world");
    }
}
