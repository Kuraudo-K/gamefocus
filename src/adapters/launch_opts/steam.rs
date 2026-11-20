//! Options de lancement Steam / VDF.

use std::fs;
use std::path::PathBuf;

use crate::adapters::shared::find_matching_brace;
use crate::domain::GameEntry;

use super::{backup_file, merge_launch_options, remove_managed_launch_options};

// ── apply / clear / read ────────────────────────────────────────────────────

pub(super) fn apply_steam(game: &GameEntry, cmd: &str) -> Result<(String, Vec<String>), String> {
    let app_id = game
        .app_id
        .ok_or_else(|| "Steam : AppID manquant".to_string())?;
    let path = find_localconfig().ok_or_else(|| "localconfig.vdf introuvable".to_string())?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let Some((updated, managed)) = safe_steam_options_update(&raw, app_id, cmd)? else {
        return Ok((
            format!("Options Steam déjà présentes (appid {app_id})."),
            Vec::new(),
        ));
    };
    backup_file(&path)?;
    fs::write(&path, updated).map_err(|e| e.to_string())?;
    Ok((
        format!(
            "Options Steam fusionnées (appid {app_id}). Relance Steam si le jeu était déjà ouvert."
        ),
        managed,
    ))
}

pub(super) fn clear_steam(game: &GameEntry, managed: &[String]) -> Result<String, String> {
    let app_id = game
        .app_id
        .ok_or_else(|| "Steam : AppID manquant".to_string())?;
    let path = find_localconfig().ok_or_else(|| "localconfig.vdf introuvable".to_string())?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let Some(current) = parse_steam_launch_options(&raw, app_id)? else {
        return Ok(format!("Aucune option Steam à retirer (appid {app_id})."));
    };
    let remaining = remove_managed_launch_options(&current, managed)?;
    let value = (remaining != "%command%").then_some(remaining.as_str());
    let updated = set_steam_launch_options(&raw, app_id, value)?;
    backup_file(&path)?;
    fs::write(&path, updated).map_err(|e| e.to_string())?;
    Ok(format!("Options Steam retirées (appid {app_id})."))
}

pub(super) fn read_steam_launch_options_impl(
    game: &GameEntry,
) -> Result<Option<String>, String> {
    let app_id = game
        .app_id
        .ok_or_else(|| "Steam : AppID manquant".to_string())?;
    let path = find_localconfig().ok_or_else(|| "localconfig.vdf introuvable".to_string())?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("lecture {} : {error}", path.display()))?;
    parse_steam_launch_options(&raw, app_id)
}

// ── public Steam API ─────────────────────────────────────────────────────────

/// Insert / update / remove `"LaunchOptions" "…"` inside the appid block.
pub fn set_steam_launch_options(
    raw: &str,
    app_id: u32,
    value: Option<&str>,
) -> Result<String, String> {
    // ------ Bloc apps ------
    // localconfig contient plusieurs `"appid"` (blobs, CDN…).
    // On cible uniquement `"appid" { … LastPlayed/Playtime/LaunchOptions … }`.
    let (block_start, block_end) = find_steam_app_play_block(raw, app_id).ok_or_else(|| {
        format!("AppID {app_id} absent du bloc jeux — lance le jeu une fois via Steam d'abord.")
    })?;
    let block = &raw[block_start..=block_end];

    let new_block = if let Some(cmd) = value {
        let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
        let line = format!("\t\t\t\t\t\t\"LaunchOptions\"\t\t\"{escaped}\"\n");
        if let Some(lo) = find_launch_options_span(block) {
            let (a, b) = lo;
            format!("{}{}{}", &block[..a], line.trim_start(), &block[b..])
        } else {
            let insert_at = block.len() - 1;
            format!("{}{}{}", &block[..insert_at], line, &block[insert_at..])
        }
    } else if let Some((a, b)) = find_launch_options_span(block) {
        format!("{}{}", &block[..a], &block[b..])
    } else {
        return Ok(raw.to_string());
    };

    Ok(format!(
        "{}{}{}",
        &raw[..block_start],
        new_block,
        &raw[block_end + 1..]
    ))
}

// ── private internals ────────────────────────────────────────────────────────

fn parse_steam_launch_options(raw: &str, app_id: u32) -> Result<Option<String>, String> {
    let Some((block_start, block_end)) = find_steam_app_play_block(raw, app_id) else {
        return Ok(None);
    };
    let block = &raw[block_start..=block_end];
    let Some(start) = block.find("\"LaunchOptions\"") else {
        return Ok(None);
    };
    let after_key = &block[start + "\"LaunchOptions\"".len()..];
    let value_start = after_key
        .find('"')
        .map(|index| start + "\"LaunchOptions\"".len() + index + 1)
        .ok_or_else(|| format!("LaunchOptions invalide pour l'AppID {app_id}"))?;
    let mut characters = block[value_start..].chars();
    let mut value = String::new();
    while let Some(character) = characters.next() {
        match character {
            '\\' => {
                let Some(escaped) = characters.next() else {
                    return Err(format!("LaunchOptions non terminée pour l'AppID {app_id}"));
                };
                value.push(escaped);
            }
            '"' => {
                let value = value.trim().to_owned();
                return Ok((!value.is_empty()).then_some(value));
            }
            character => value.push(character),
        }
    }
    Err(format!("LaunchOptions non terminée pour l'AppID {app_id}"))
}

fn safe_steam_options_update(
    raw: &str,
    app_id: u32,
    cmd: &str,
) -> Result<Option<(String, Vec<String>)>, String> {
    let current = parse_steam_launch_options(raw, app_id)?;
    let (merged, managed) = merge_launch_options(current.as_deref(), cmd)?;
    if current.as_deref() == Some(merged.as_str()) {
        return Ok(None);
    }
    let updated = set_steam_launch_options(raw, app_id, Some(&merged))?;
    Ok(Some((updated, managed)))
}

fn find_localconfig() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let steam_roots = [
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];
    for steam_root in &steam_roots {
        let active_account = fs::read_to_string(steam_root.join("config/loginusers.vdf"))
            .ok()
            .and_then(|contents| most_recent_steam_account(&contents));
        if let Some(account) = active_account {
            let path = steam_root
                .join("userdata")
                .join(account)
                .join("config/localconfig.vdf");
            if path.is_file() {
                return Some(path);
            }
        }
    }

    let mut best: Option<(u64, PathBuf)> = None;
    for steam_root in &steam_roots {
        let Ok(rd) = fs::read_dir(steam_root.join("userdata")) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path().join("config/localconfig.vdf");
            if !p.is_file() {
                continue;
            }
            let len = entry
                .path()
                .join("config/localconfig.vdf")
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);
            if best.as_ref().map(|(l, _)| len > *l).unwrap_or(true) {
                best = Some((len, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn most_recent_steam_account(raw: &str) -> Option<String> {
    let mut steam_id = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "}" {
            steam_id = None;
            continue;
        }
        if trimmed.starts_with("\"MostRecent\"") {
            if first_quoted_value(trimmed["\"MostRecent\"".len()..].trim())?
                .parse::<u8>()
                .ok()
                == Some(1)
            {
                return steam_id
                    .and_then(|id: u64| id.checked_sub(76_561_197_960_265_728))
                    .map(|account_id| account_id.to_string());
            }
        } else if trimmed.starts_with('"') {
            let key = first_quoted_value(trimmed)?;
            if key.len() >= 17 && key.bytes().all(|byte| byte.is_ascii_digit()) {
                steam_id = key.parse::<u64>().ok();
            }
        }
    }
    None
}

fn first_quoted_value(input: &str) -> Option<String> {
    let mut chars = input.trim().strip_prefix('"')?.chars();
    let mut value = String::new();
    while let Some(character) = chars.next() {
        match character {
            '\\' => value.push(chars.next()?),
            '"' => return Some(value),
            character => value.push(character),
        }
    }
    None
}

/// Trouve le vrai bloc jeu (pas un blob / CDN).
fn find_steam_app_play_block(raw: &str, app_id: u32) -> Option<(usize, usize)> {
    let needle = format!("\"{app_id}\"");
    let mut search = 0usize;
    let mut fallback: Option<(usize, usize)> = None;
    while let Some(rel) = raw[search..].find(&needle) {
        let app_pos = search + rel;
        let after = &raw[app_pos + needle.len()..];
        let ws = after.len() - after.trim_start().len();
        let rest = after.trim_start();
        if !rest.starts_with('{') {
            search = app_pos + needle.len();
            continue;
        }
        let block_start = app_pos + needle.len() + ws;
        let Some(block_end) = find_matching_brace(raw, block_start) else {
            search = app_pos + needle.len();
            continue;
        };
        let block = &raw[block_start..=block_end];
        let is_play = block.contains("LastPlayed")
            || block.contains("Playtime")
            || block.contains("LaunchOptions")
            || block.contains("autocloud");
        if is_play {
            return Some((block_start, block_end));
        }
        if fallback.is_none() {
            fallback = Some((block_start, block_end));
        }
        search = app_pos + needle.len();
    }
    fallback
}

fn find_launch_options_span(block: &str) -> Option<(usize, usize)> {
    let key = "\"LaunchOptions\"";
    let start = block.find(key)?;
    // include leading whitespace/newline
    let mut line_start = start;
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
    let after = &block[start + key.len()..];
    let q1 = after.find('"')?;
    let rest = &after[q1 + 1..];
    let mut q2 = 0usize;
    let rb = rest.as_bytes();
    while q2 < rb.len() {
        if rb[q2] == b'\\' {
            q2 += 2;
            continue;
        }
        if rb[q2] == b'"' {
            break;
        }
        q2 += 1;
    }
    let end = start + key.len() + q1 + 1 + q2 + 1;
    // swallow trailing newline
    let mut end = end;
    if block.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    } else if block.as_bytes().get(end) == Some(&b'\r') {
        end += 1;
        if block.as_bytes().get(end) == Some(&b'\n') {
            end += 1;
        }
    }
    Some((line_start, end))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_set_and_clear_launch_options() {
        let raw = r#"
"UserLocalConfigStore"
{
	"Software"
	{
		"Valve"
		{
			"Steam"
			{
				"apps"
				{
					"1091500"
					{
						"LastPlayed"		"1"
					}
				}
			}
		}
	}
}
"#;
        let with = set_steam_launch_options(raw, 1091500, Some("mangohud %command%")).unwrap();
        assert!(with.contains("LaunchOptions"));
        assert!(with.contains("mangohud %command%"));
        let cleared = set_steam_launch_options(&with, 1091500, None).unwrap();
        assert!(!cleared.contains("LaunchOptions"));
    }

    #[test]
    fn steam_skips_non_play_blob_occurrence() {
        // Premier "892970" = blob (comme Steam réel) ; le vrai jeu est plus bas.
        let raw = r#"
"x"
{
	"892970"		"38000000deadbeef"
	"apps"
	{
		"892970"
		{
			"LastPlayed"		"1"
			"Playtime"		"10"
			"LaunchOptions"		"FSR4_UPGRADE=1 %command%"
		}
	}
}
"#;
        let with =
            set_steam_launch_options(raw, 892970, Some("mangohud gamemoderun %command%")).unwrap();
        assert!(with.contains("mangohud gamemoderun %command%"));
        assert!(!with.contains("FSR4_UPGRADE=1 %command%"));
        // Le blob ne doit pas être touché
        assert!(with.contains("38000000deadbeef"));
    }

    #[test]
    fn steam_launch_options_reader_reports_missing_and_existing_values() {
        let raw = r#"
"apps"
{
    "42"
    {
        "LastPlayed" "1"
        "LaunchOptions" "gamemoderun %command%"
    }
}
"#;
        assert_eq!(
            parse_steam_launch_options(raw, 42).unwrap(),
            Some("gamemoderun %command%".into())
        );
        assert_eq!(parse_steam_launch_options(raw, 43).unwrap(), None);
    }

    #[test]
    fn steam_user_selection_uses_the_most_recent_login_account() {
        let raw = r#"
"users"
{
    "76561198000000000"
    {
        "AccountName" "old"
        "MostRecent" "0"
    }
    "76561198000000001"
    {
        "AccountName" "current"
        "MostRecent" "1"
    }
}
"#;
        assert_eq!(most_recent_steam_account(raw).as_deref(), Some("39734273"));
    }

    #[test]
    fn steam_launch_options_reader_unescapes_quotes_and_preserves_utf8() {
        let raw = r#"
"apps"
{
    "42"
    {
        "LastPlayed" "1"
        "LaunchOptions" "PROTON_LOG=1 %command% # été \"test\""
    }
}
"#;
        assert_eq!(
            parse_steam_launch_options(raw, 42).unwrap(),
            Some("PROTON_LOG=1 %command% # été \"test\"".into())
        );
    }

    #[test]
    fn steam_update_preserves_custom_commands() {
        let raw = r#"
"apps"
{
    "42"
    {
        "LastPlayed" "1"
        "LaunchOptions" "CUSTOM=1 %command%"
    }
}
"#;
        let Some((updated, added)) =
            safe_steam_options_update(raw, 42, "gamemoderun %command%").unwrap()
        else {
            panic!("GameMode must be merged into existing options");
        };
        assert!(updated.contains("gamemoderun CUSTOM=1 %command%"));
        assert_eq!(added, ["gamemoderun"]);
        assert_eq!(
            safe_steam_options_update(raw, 42, "CUSTOM=1 %command%").unwrap(),
            None
        );
    }
}
