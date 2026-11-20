//! Scan Faugus Launcher (`~/.local/share/faugus-launcher/games.json`).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::domain::{GameEntry, GameSource};
use crate::ports::GameLibrary;

pub struct FaugusLibrary {
    override_paths: Option<Vec<PathBuf>>,
}

impl FaugusLibrary {
    pub fn new() -> Self {
        Self {
            override_paths: None,
        }
    }

    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            override_paths: Some(paths),
        }
    }

    pub fn default_games_json_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for rel in [
                ".local/share/faugus-launcher/games.json",
                ".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher/games.json",
                ".var/app/io.github.faugus.faugus-launcher/data/faugus-launcher/games.json",
            ] {
                let p = home.join(rel);
                if p.exists() {
                    paths.push(p);
                }
            }
        }
        paths
    }
}

impl Default for FaugusLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLibrary for FaugusLibrary {
    fn scan(&self) -> Result<Vec<GameEntry>, String> {
        let paths = if let Some(ref o) = self.override_paths {
            o.clone()
        } else {
            Self::default_games_json_paths()
        };
        let mut games = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for path in paths {
            let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            for g in parse_faugus_games(&raw)? {
                let key = g.name.to_lowercase();
                if seen.insert(key) {
                    games.push(g);
                }
            }
        }
        games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(games)
    }
}

pub fn parse_faugus_games(raw: &str) -> Result<Vec<GameEntry>, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let arr = match v {
        Value::Array(a) => a,
        Value::Object(map) => map
            .get("games")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default(),
        _ => vec![],
    };
    let mut out = Vec::new();
    for item in arr {
        if item
            .get("hidden")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        // Entrées « catégorie » (pas un jeu)
        if item
            .get("category")
            .and_then(|x| x.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let name = item
            .get("title")
            .or_else(|| item.get("name"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let install_path = item
            .get("path")
            .or_else(|| item.get("prefix"))
            .and_then(|x| x.as_str())
            .map(PathBuf::from);
        let launch_options = item
            .get("launch_arguments")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        // `cover` est souvent "" → ne pas bloquer le fallback `icon`.
        let cover_path = ["cover", "icon"]
            .iter()
            .find_map(|k| {
                item.get(*k)
                    .and_then(|x| x.as_str())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .filter(|p| p.exists())
            })
            .or_else(|| {
                // Faugus range aussi covers/icons/{gameid}.png
                let gid = item.get("gameid").and_then(|x| x.as_str())?;
                let home = std::env::var_os("HOME").map(PathBuf::from)?;
                for rel in [
                    format!(".local/share/faugus-launcher/covers/{gid}.png"),
                    format!(".local/share/faugus-launcher/icons/{gid}.png"),
                    format!(".local/share/faugus-launcher/icons/{gid}.ico"),
                ] {
                    let p = home.join(rel);
                    if p.exists() {
                        return Some(p);
                    }
                }
                None
            });
        let launcher_id = item
            .get("gameid")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        out.push(GameEntry {
            app_id: None,
            name,
            source: GameSource::Faugus,
            install_path,
            launch_options,
            cover_path,
            launcher_id,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample() {
        let raw = r#"[
          {"title": "Soulframe", "path": "/games/sf.exe", "hidden": false, "category": false},
          {"title": "Hidden", "path": "/x", "hidden": true, "category": false},
          {"title": "Cat", "path": "", "hidden": false, "category": true}
        ]"#;
        let games = parse_faugus_games(raw).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Soulframe");
        assert_eq!(games[0].source, GameSource::Faugus);
    }

    #[test]
    fn empty_cover_falls_back_to_icon() {
        let dir = tempfile::tempdir().unwrap();
        let icon = dir.path().join("game.png");
        fs::write(&icon, b"x").unwrap();
        let raw = format!(
            r#"[{{"title":"X","path":"/x","cover":"","icon":"{}","hidden":false,"category":false}}]"#,
            icon.display()
        );
        let games = parse_faugus_games(&raw).unwrap();
        assert_eq!(games[0].cover_path.as_deref(), Some(icon.as_path()));
    }
}
