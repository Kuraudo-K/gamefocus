//! Scan Lutris via `pga.db` (SQLite) — lecture légère via Python 3 (toujours présent).

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

use crate::domain::{GameEntry, GameSource};
use crate::ports::GameLibrary;

pub struct LutrisLibrary {
    override_db: Option<PathBuf>,
}

impl LutrisLibrary {
    pub fn new() -> Self {
        Self { override_db: None }
    }

    pub fn with_db(path: PathBuf) -> Self {
        Self {
            override_db: Some(path),
        }
    }

    pub fn default_db_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for rel in [
                ".local/share/lutris/pga.db",
                ".var/app/net.lutris.Lutris/data/lutris/pga.db",
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

impl Default for LutrisLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLibrary for LutrisLibrary {
    fn scan(&self) -> Result<Vec<GameEntry>, String> {
        let dbs = if let Some(ref db) = self.override_db {
            vec![db.clone()]
        } else {
            Self::default_db_paths()
        };
        let mut games = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for db in dbs {
            for g in scan_pga_db(&db)? {
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

pub fn parse_lutris_json_rows(raw: &str) -> Result<Vec<GameEntry>, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for item in arr {
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let install_path = item
            .get("directory")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let launcher_id = item
            .get("slug")
            .and_then(|x| x.as_str())
            .map(str::to_string);
        out.push(GameEntry {
            app_id: None,
            name,
            source: GameSource::Lutris,
            install_path,
            launch_options: None,
            cover_path: None,
            launcher_id,
        });
    }
    Ok(out)
}

fn scan_pga_db(db: &PathBuf) -> Result<Vec<GameEntry>, String> {
    // Évite rusqlite (deps C) : Python stdlib suffit sur GLF-OS.
    let script = r#"
import json, sqlite3, sys
con = sqlite3.connect(sys.argv[1])
rows = con.execute(
    "SELECT name, directory, slug FROM games WHERE installed = 1 AND name IS NOT NULL AND name != ''"
).fetchall()
print(json.dumps([{"name": n, "directory": d or "", "slug": s or ""} for n, d, s in rows]))
"#;
    let output = Command::new("python3")
        .args(["-c", script, db.to_str().unwrap_or("")])
        .output()
        .map_err(|e| format!("python3/lutris: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        // DB vide / schema old → pas d'erreur fatale
        if err.contains("no such table") {
            return Ok(vec![]);
        }
        return Err(format!("lutris pga.db: {err}"));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    parse_lutris_json_rows(raw.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rows() {
        let raw = r#"[{"name":"Celeste","directory":"/games/celeste"}]"#;
        let games = parse_lutris_json_rows(raw).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].source, GameSource::Lutris);
        assert_eq!(games[0].name, "Celeste");
    }
}
