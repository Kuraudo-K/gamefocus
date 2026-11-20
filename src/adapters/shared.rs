//! Helpers partagés entre adapters (évite les copies divergentes).

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// Encode une requête URL (forme légère, sans dépendance).
pub fn urlencoding_lite(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".into(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

/// Trouve le `}` apparié à `{` en `open_idx`, en ignorant le contenu entre guillemets.
pub fn find_matching_brace(s: &str, open_idx: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(open_idx) != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut i = open_idx;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    break;
                }
                i += 1;
            }
        } else if bytes[i] == b'{' {
            depth += 1;
        } else if bytes[i] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Chemin `games.json` Faugus (natif + Flatpak).
pub fn faugus_games_json() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    for rel in [
        ".local/share/faugus-launcher/games.json",
        ".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher/games.json",
        ".var/app/io.github.faugus.faugus-launcher/data/faugus-launcher/games.json",
    ] {
        let p = home.join(rel);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Fichier GamesConfig Heroic pour un app id / appName (natif + Flatpak).
/// Essaie `{app}.json`, puis scanne le dossier si l’entrée est ailleurs.
pub fn find_heroic_game_config(app: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dirs = [
        home.join(".config/heroic/GamesConfig"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/GamesConfig"),
    ];
    for dir in &dirs {
        let direct = dir.join(format!("{app}.json"));
        if direct.exists() {
            return Some(direct);
        }
        let Ok(rd) = fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&p) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            if heroic_config_mentions_app(&value, app) {
                return Some(p);
            }
        }
    }
    None
}

fn heroic_config_mentions_app(root: &Value, app: &str) -> bool {
    let Some(object) = root.as_object() else {
        return false;
    };
    if object.get(app).is_some_and(Value::is_object) {
        return true;
    }
    let matches = object.values().filter(|entry| {
        entry.is_object()
            && ["appName", "app_name", "appId", "app_id", "id"]
                .iter()
                .any(|key| entry.get(*key).and_then(Value::as_str) == Some(app))
    });
    match matches.count() {
        1 => true,
        0 if object.len() == 1 => object.values().next().is_some_and(Value::is_object),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_matching_skips_quotes() {
        let s = r#"{ "a": "{ x }", "b": 1 }"#;
        assert_eq!(find_matching_brace(s, 0), Some(s.len() - 1));
    }

    #[test]
    fn urlencoding_spaces_and_unicode() {
        assert_eq!(urlencoding_lite("a b"), "a+b");
        assert!(urlencoding_lite("é").starts_with('%'));
    }
}
