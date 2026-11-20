//! Applique un Proton choisi à Steam / Heroic / Faugus pour un jeu (ou tous).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::adapters::proton_tools::ProtonTool;
use crate::adapters::shared::{faugus_games_json, find_heroic_game_config, find_matching_brace};
use crate::domain::{GameEntry, GameSource};

/// Force le Proton sélectionné sur le launcher du jeu (avant Lancer).
pub fn apply_proton_for_game(game: &GameEntry, tool: &ProtonTool) -> Result<String, String> {
    match game.source {
        GameSource::Steam => apply_steam(game, tool),
        GameSource::Heroic => apply_heroic(game, tool),
        GameSource::Faugus => apply_faugus(game, tool),
        GameSource::Lutris => Err(
            "Lutris : change le runner dans Lutris (pas d’écriture auto pour l’instant).".into(),
        ),
        GameSource::Manual => Err("Jeu manuel.".into()),
    }
}

pub fn apply_proton_to_all(games: &[GameEntry], tool: &ProtonTool) -> (usize, usize, String) {
    let mut ok = 0usize;
    let mut fail = 0usize;
    for g in games {
        match apply_proton_for_game(g, tool) {
            Ok(_) => ok += 1,
            Err(_) => fail += 1,
        }
    }
    (
        ok,
        fail,
        format!(
            "Proton « {} » → {} jeux OK, {} ignorés/échecs.",
            tool.name, ok, fail
        ),
    )
}

fn apply_steam(game: &GameEntry, tool: &ProtonTool) -> Result<String, String> {
    let app_id = game
        .app_id
        .ok_or_else(|| "Steam : AppID manquant".to_string())?;
    let path = find_steam_config().ok_or_else(|| "config.vdf Steam introuvable".to_string())?;
    backup(&path);
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let updated = upsert_compat_mapping(&raw, app_id, &tool.name)?;
    fs::write(&path, updated).map_err(|e| e.to_string())?;
    Ok(format!(
        "Steam : {} → Proton « {} » (relance Steam si déjà ouvert).",
        app_id, tool.name
    ))
}

fn apply_heroic(game: &GameEntry, tool: &ProtonTool) -> Result<String, String> {
    let app = game
        .launcher_id
        .clone()
        .or_else(|| game.app_id.map(|i| i.to_string()))
        .ok_or_else(|| "Heroic : id manquant".to_string())?;
    let path = find_heroic_game_config(&app)
        .ok_or_else(|| format!("GamesConfig Heroic introuvable ({app})"))?;
    backup(&path);
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| "GamesConfig invalide".to_string())?;
    let entry = obj
        .values_mut()
        .find(|x| x.is_object())
        .ok_or_else(|| "entrée Heroic vide".to_string())?;
    entry["wineVersion"] = json!({
        "bin": tool.proton_bin.to_string_lossy(),
        "name": tool.name,
        "type": "proton"
    });
    fs::write(
        &path,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!("Heroic : Proton « {} ».", tool.name))
}

fn apply_faugus(game: &GameEntry, tool: &ProtonTool) -> Result<String, String> {
    let gid = game
        .launcher_id
        .as_deref()
        .ok_or_else(|| "Faugus : gameid manquant".to_string())?;
    let path = faugus_games_json().ok_or_else(|| "games.json Faugus introuvable".to_string())?;
    backup(&path);
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let arr = v
        .as_array_mut()
        .ok_or_else(|| "games.json : tableau attendu".to_string())?;
    let mut found = false;
    for item in arr.iter_mut() {
        if item.get("gameid").and_then(|x| x.as_str()) != Some(gid) {
            continue;
        }
        item["runner"] = json!(tool.name);
        found = true;
        break;
    }
    if !found {
        return Err(format!("Faugus : « {gid} » introuvable"));
    }
    fs::write(
        &path,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!("Faugus : runner « {} ».", tool.name))
}

fn find_steam_config() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    for p in [
        home.join(".local/share/Steam/config/config.vdf"),
        home.join(".steam/steam/config/config.vdf"),
    ] {
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn backup(path: &Path) {
    let bak = path.with_extension("gamefocus.bak");
    let _ = fs::copy(path, bak);
}

/// Insert / update CompatToolMapping entry for app_id.
pub fn upsert_compat_mapping(raw: &str, app_id: u32, tool_name: &str) -> Result<String, String> {
    let marker = "CompatToolMapping\"";
    let Some(map_key) = raw.find(marker) else {
        return Err("CompatToolMapping absent de config.vdf".into());
    };
    let after = &raw[map_key + marker.len()..];
    let brace_rel = after
        .find('{')
        .ok_or_else(|| "CompatToolMapping mal formé".to_string())?;
    let block_start = map_key + marker.len() + brace_rel;
    let block_end = find_matching_brace(raw, block_start)
        .ok_or_else(|| "CompatToolMapping non fermé".to_string())?;
    let block = &raw[block_start..=block_end];

    let entry = format!(
        "\t\t\t\t\t\"{app_id}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"name\"\t\t\"{tool_name}\"\n\t\t\t\t\t\t\"config\"\t\t\"\"\n\t\t\t\t\t\t\"priority\"\t\t\"250\"\n\t\t\t\t\t}}\n"
    );

    let needle = format!("\"{app_id}\"");
    let new_block = if let Some(pos) = block.find(&needle) {
        // replace existing block for this app
        let after_id = &block[pos + needle.len()..];
        let open_rel = after_id
            .find('{')
            .ok_or_else(|| "bloc appid Compat mal formé".to_string())?;
        let open = pos + needle.len() + open_rel;
        let close = find_matching_brace(block, open)
            .ok_or_else(|| "bloc appid Compat non fermé".to_string())?;
        // include leading whitespace before "appid"
        let mut line_start = pos;
        while line_start > 0 {
            let c = block.as_bytes()[line_start - 1];
            if c == b'\n' {
                break;
            }
            if c == b'\t' || c == b' ' {
                line_start -= 1;
                continue;
            }
            break;
        }
        format!("{}{}{}", &block[..line_start], entry, &block[close + 1..])
    } else {
        let insert_at = block.len() - 1;
        format!("{}{}{}", &block[..insert_at], entry, &block[insert_at..])
    };

    Ok(format!(
        "{}{}{}",
        &raw[..block_start],
        new_block,
        &raw[block_end + 1..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_adds_and_updates() {
        let raw = r#"
"InstallConfigStore"
{
	"Software"
	{
		"Valve"
		{
			"Steam"
			{
				"CompatToolMapping"
				{
					"570"
					{
						"name"		"proton_experimental"
						"config"		""
						"priority"		"250"
					}
				}
			}
		}
	}
}
"#;
        let v = upsert_compat_mapping(raw, 1091500, "Proton-GE-GLF").unwrap();
        assert!(v.contains("1091500"));
        assert!(v.contains("Proton-GE-GLF"));
        let v2 = upsert_compat_mapping(&v, 1091500, "Proton-CachyOS Latest").unwrap();
        assert!(v2.contains("Proton-CachyOS Latest"));
        assert_eq!(v2.matches("1091500").count(), 1);
    }
}
