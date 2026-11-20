//! Scan Heroic : GOG / Legendary / Nile (chemins réels Heroic 2.x).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::domain::{GameEntry, GameSource};
use crate::ports::GameLibrary;

pub struct HeroicLibrary {
    override_roots: Option<Vec<PathBuf>>,
}

impl HeroicLibrary {
    pub fn new() -> Self {
        Self {
            override_roots: None,
        }
    }

    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            override_roots: Some(roots),
        }
    }

    pub fn default_heroic_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for rel in [
                ".config/heroic",
                ".var/app/com.heroicgameslauncher.hgl/config/heroic",
            ] {
                let p = home.join(rel);
                if p.exists() {
                    roots.push(p);
                }
            }
        }
        roots
    }
}

impl Default for HeroicLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLibrary for HeroicLibrary {
    fn scan(&self) -> Result<Vec<GameEntry>, String> {
        let roots = if let Some(ref o) = self.override_roots {
            o.clone()
        } else {
            Self::default_heroic_roots()
        };

        let mut games = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for root in roots {
            let titles = load_title_index(&root);

            // GOG installés
            let gog = root.join("gog_store/installed.json");
            if gog.exists() {
                let raw = fs::read_to_string(&gog).map_err(|e| e.to_string())?;
                for g in parse_gog_installed(&raw, &titles)? {
                    push_unique(&mut games, &mut seen, g);
                }
            }

            // Legendary (Epic) — chemins Heroic 2.x et anciens
            for rel in [
                "legendaryConfig/legendary/installed.json",
                "legendary/installed.json",
            ] {
                let path = root.join(rel);
                if path.exists() {
                    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    for g in parse_legendary_installed(&raw)? {
                        push_unique(&mut games, &mut seen, g);
                    }
                }
            }

            // Amazon / Nile
            for rel in ["nile_config/nile/installed.json", "nile/installed.json"] {
                let path = root.join(rel);
                if path.exists() {
                    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    for g in parse_nile_installed(&raw, &titles)? {
                        push_unique(&mut games, &mut seen, g);
                    }
                }
            }
        }

        games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(games)
    }
}

fn push_unique(
    games: &mut Vec<GameEntry>,
    seen: &mut std::collections::HashSet<String>,
    g: GameEntry,
) {
    let key = g.name.to_lowercase();
    if seen.insert(key) {
        games.push(g);
    }
}

fn load_title_index(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for rel in [
        "store_cache/gog_library.json",
        "store_cache/legendary_library.json",
        "store_cache/nile_library.json",
        "gog_store/library.json",
    ] {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let games = v
            .get("games")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        for item in games {
            let id = item
                .get("app_name")
                .or_else(|| item.get("appName"))
                .or_else(|| item.get("id"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let title = item
                .get("title")
                .or_else(|| item.get("name"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if !id.is_empty() && !title.is_empty() {
                map.insert(id.to_string(), title.to_string());
            }
        }
    }
    map
}

fn is_redistributable(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("redistributable")
        || n.contains("gog-redist")
        || n == "galaxy common redistributables"
}

/// `{ "installed": [ { "appName", "install_path", ... } ] }`
pub fn parse_gog_installed(
    raw: &str,
    titles: &HashMap<String, String>,
) -> Result<Vec<GameEntry>, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let arr = v
        .get("installed")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("appName")
            .or_else(|| item.get("app_name"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let path = item
            .get("install_path")
            .or_else(|| item.get("installPath"))
            .and_then(|x| x.as_str())
            .map(PathBuf::from);
        let name = titles
            .get(&id)
            .cloned()
            .or_else(|| {
                path.as_ref()
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            })
            .unwrap_or_else(|| id.clone());
        if is_redistributable(&name) || is_redistributable(&id) {
            continue;
        }
        let app_id = id.parse::<u32>().ok();
        out.push(GameEntry {
            app_id,
            name,
            source: GameSource::Heroic,
            install_path: path,
            launch_options: None,
            cover_path: None,
            launcher_id: Some(id),
        });
    }
    Ok(out)
}

pub fn parse_legendary_installed(raw: &str) -> Result<Vec<GameEntry>, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    match v {
        Value::Object(map) => {
            for (_k, item) in map {
                if let Some(g) = entry_from_object(&item) {
                    out.push(g);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                if let Some(g) = entry_from_object(&item) {
                    out.push(g);
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

pub fn parse_nile_installed(
    raw: &str,
    titles: &HashMap<String, String>,
) -> Result<Vec<GameEntry>, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let items = match v {
        Value::Array(arr) => arr,
        Value::Object(map) => map
            .get("installed")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_else(|| map.values().cloned().collect()),
        _ => vec![],
    };
    for item in items {
        let id = item
            .get("id")
            .or_else(|| item.get("appName"))
            .or_else(|| item.get("app_name"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let path = item
            .get("install_path")
            .or_else(|| item.get("path"))
            .and_then(|x| x.as_str())
            .map(PathBuf::from);
        let name = item
            .get("title")
            .or_else(|| item.get("name"))
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .or_else(|| titles.get(&id).cloned())
            .or_else(|| {
                path.as_ref()
                    .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            })
            .unwrap_or_else(|| id.clone());
        if name.is_empty() || is_redistributable(&name) {
            continue;
        }
        let launcher_id = if id.is_empty() { None } else { Some(id) };
        out.push(GameEntry {
            app_id: None,
            name,
            source: GameSource::Heroic,
            install_path: path,
            launch_options: None,
            cover_path: None,
            launcher_id,
        });
    }
    Ok(out)
}

fn entry_from_object(item: &Value) -> Option<GameEntry> {
    let name = item
        .get("title")
        .or_else(|| item.get("app_name"))
        .or_else(|| item.get("appName"))
        .or_else(|| item.get("name"))
        .and_then(|v| v.as_str())?
        .to_string();
    if name.is_empty() || is_redistributable(&name) {
        return None;
    }
    let install_path = item
        .get("install_path")
        .or_else(|| item.get("installPath"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let app_id = item
        .get("app_id")
        .or_else(|| item.get("appid"))
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);
    let launcher_id = item
        .get("app_name")
        .or_else(|| item.get("appName"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some(GameEntry {
        app_id,
        name,
        source: GameSource::Heroic,
        install_path,
        launch_options: None,
        cover_path: None,
        launcher_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gog_installed_with_titles() {
        let raw = r#"{
          "installed": [
            {
              "platform": "windows",
              "appName": "1423049311",
              "install_path": "/media/Jeux/Heroic/Cyberpunk 2077"
            }
          ]
        }"#;
        let mut titles = HashMap::new();
        titles.insert("1423049311".into(), "Cyberpunk 2077".into());
        let games = parse_gog_installed(raw, &titles).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Cyberpunk 2077");
        assert_eq!(games[0].source, GameSource::Heroic);
        assert_eq!(games[0].app_id, Some(1423049311));
    }

    #[test]
    fn parse_gog_falls_back_to_folder_name() {
        let raw = r#"{
          "installed": [
            { "appName": "999", "install_path": "/games/Cool Game" }
          ]
        }"#;
        let games = parse_gog_installed(raw, &HashMap::new()).unwrap();
        assert_eq!(games[0].name, "Cool Game");
    }

    #[test]
    fn parse_legendary_style() {
        let raw = r#"{
          "Cyberpunk2077": {
            "title": "Cyberpunk 2077",
            "install_path": "/games/CP2077",
            "app_id": 1091500
          }
        }"#;
        let games = parse_legendary_installed(raw).unwrap();
        assert!(games.iter().any(|g| g.name == "Cyberpunk 2077"));
    }

    #[test]
    fn parse_empty_legendary() {
        assert!(parse_legendary_installed("{}").unwrap().is_empty());
        assert!(parse_legendary_installed("[]").unwrap().is_empty());
    }
}
