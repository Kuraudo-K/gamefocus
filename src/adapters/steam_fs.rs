//! Scan filesystem Steam (libraryfolders.vdf + appmanifest_*.acf).
//! Parser VDF ligne-à-ligne (pas de dépendance keyvalues-parser).

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{GameEntry, GameSource};
use crate::ports::GameLibrary;

pub struct SteamLibrary {
    extra_roots: Vec<PathBuf>,
    /// Si défini, remplace la découverte HOME (tests / fixtures).
    override_roots: Option<Vec<PathBuf>>,
}

impl SteamLibrary {
    pub fn new(extra_roots: Vec<PathBuf>) -> Self {
        Self {
            extra_roots,
            override_roots: None,
        }
    }

    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            extra_roots: vec![],
            override_roots: Some(roots),
        }
    }

    pub fn default_steam_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            for rel in [
                ".local/share/Steam",
                ".steam/steam",
                ".steam/root",
                ".var/app/com.valvesoftware.Steam/.local/share/Steam",
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

impl GameLibrary for SteamLibrary {
    fn scan(&self) -> Result<Vec<GameEntry>, String> {
        let mut roots = if let Some(ref o) = self.override_roots {
            o.clone()
        } else {
            Self::default_steam_roots()
        };
        for r in &self.extra_roots {
            if !roots.iter().any(|x| x == r) {
                roots.push(r.clone());
            }
        }

        let mut games = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for root in roots {
            let steamapps = root.join("steamapps");
            if !steamapps.is_dir() {
                continue;
            }

            for lib in discover_library_paths(&steamapps)? {
                let apps = lib.join("steamapps");
                let dir = if apps.is_dir() { apps } else { lib };
                for entry in scan_appmanifests(&dir)? {
                    let key = (entry.app_id.unwrap_or(0), entry.name.to_lowercase());
                    if seen.insert(key) {
                        games.push(entry);
                    }
                }
            }
        }

        games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(games)
    }
}

fn discover_library_paths(steamapps: &Path) -> Result<Vec<PathBuf>, String> {
    let mut libs = vec![steamapps.parent().unwrap_or(steamapps).to_path_buf()];

    let vdf_path = steamapps.join("libraryfolders.vdf");
    if !vdf_path.exists() {
        return Ok(libs);
    }

    let raw = fs::read_to_string(&vdf_path)
        .map_err(|e| format!("lecture {}: {e}", vdf_path.display()))?;
    for path in parse_library_folders_vdf(&raw) {
        let p = PathBuf::from(&path);
        if p.exists() && !libs.iter().any(|x| x == &p) {
            libs.push(p);
        }
    }
    Ok(libs)
}

/// Extrait les chemins `path` de libraryfolders.vdf (format Valve).
pub fn parse_library_folders_vdf(raw: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        // Ligne dédiée `"path" "…"` ou compacte `"0" { "path" "…" }`
        if let Some(idx) = t.find("\"path\"") {
            let rest = t[idx + "\"path\"".len()..].trim();
            if let Some(p) = extract_quoted(rest) {
                paths.push(p.replace("\\\\", "\\"));
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn extract_quoted(s: &str) -> Option<String> {
    let s = s.trim();
    if !s.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = s.chars().skip(1);
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else if c == '"' {
            break;
        } else {
            out.push(c);
        }
    }
    Some(out)
}

pub fn scan_appmanifests(steamapps: &Path) -> Result<Vec<GameEntry>, String> {
    let mut games = Vec::new();
    if !steamapps.is_dir() {
        return Ok(games);
    }
    for entry in fs::read_dir(steamapps).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
            continue;
        }
        let raw = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
        if let Some(g) = parse_appmanifest(&raw, steamapps) {
            games.push(g);
        }
    }
    Ok(games)
}

pub fn parse_appmanifest(raw: &str, steamapps: &Path) -> Option<GameEntry> {
    let app_id = find_acf_field(raw, "appid")?.parse::<u32>().ok();
    let name = find_acf_field(raw, "name")?;
    let lower = name.to_lowercase();
    if lower.contains("steamworks")
        || lower.contains("proton")
        || lower.starts_with("steam linux")
        || lower.contains("redistributable")
    {
        return None;
    }
    let installdir = find_acf_field(raw, "installdir");
    let install_path = installdir.map(|d| steamapps.join("common").join(d));
    Some(GameEntry {
        app_id,
        name,
        source: GameSource::Steam,
        install_path,
        launch_options: None,
        cover_path: None,
        launcher_id: None,
    })
}

fn find_acf_field(raw: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    for line in raw.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix(&needle) {
            return extract_quoted(rest.trim());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_library_folders() {
        let raw = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/user/.local/share/Steam"
		"label"		""
	}
	"1"
	{
		"path"		"/mnt/games/SteamLibrary"
	}
}
"#;
        let paths = parse_library_folders_vdf(raw);
        assert!(paths.iter().any(|p| p.contains("SteamLibrary")));
        assert!(paths.iter().any(|p| p.contains(".local/share/Steam")));
    }

    #[test]
    fn parse_manifest_cyberpunk() {
        let raw = r#"
"AppState"
{
	"appid"		"1091500"
	"Universe"		"1"
	"name"		"Cyberpunk 2077"
	"StateFlags"		"4"
	"installdir"		"Cyberpunk 2077"
}
"#;
        let dir = tempdir().unwrap();
        let g = parse_appmanifest(raw, dir.path()).unwrap();
        assert_eq!(g.app_id, Some(1091500));
        assert_eq!(g.name, "Cyberpunk 2077");
    }

    #[test]
    fn skips_proton() {
        let raw = r#"
"AppState"
{
	"appid"		"1887720"
	"name"		"Proton 8.0"
	"installdir"		"Proton 8.0"
}
"#;
        let dir = tempdir().unwrap();
        assert!(parse_appmanifest(raw, dir.path()).is_none());
    }

    #[test]
    fn scan_fixture_dir() {
        let dir = tempdir().unwrap();
        let steamapps = dir.path().join("steamapps");
        fs::create_dir_all(steamapps.join("common")).unwrap();
        let mut f = fs::File::create(steamapps.join("appmanifest_570.acf")).unwrap();
        writeln!(
            f,
            r#""AppState"
{{
	"appid"		"570"
	"name"		"Dota 2"
	"installdir"		"dota 2 beta"
}}"#
        )
        .unwrap();
        let games = scan_appmanifests(&steamapps).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Dota 2");
    }

    #[test]
    fn skips_redistributable_and_steamworks() {
        let dir = tempdir().unwrap();
        for (id, name) in [
            ("228980", "Steamworks Common Redistributables"),
            ("90", "Steam Linux Runtime - Soldier"),
        ] {
            let raw = format!(
                r#""AppState"
{{
	"appid"		"{id}"
	"name"		"{name}"
	"installdir"		"x"
}}"#
            );
            assert!(
                parse_appmanifest(&raw, dir.path()).is_none(),
                "should skip {name}"
            );
        }
    }

    #[test]
    fn vdf_compact_inline_path() {
        let raw = r#""0" { "path" "/mnt/games/SteamLibrary" }"#;
        let paths = parse_library_folders_vdf(raw);
        assert_eq!(paths, vec!["/mnt/games/SteamLibrary".to_string()]);
    }
}
