//! Options de lancement Faugus (games.json).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::adapters::shared::faugus_games_json;
use crate::domain::GameEntry;

use super::{
    append_launch_arguments, backup_file, remove_managed_arguments, same_launch_option,
    split_launch_tokens, GAMEFOCUS_GAME_ARGUMENT_PREFIX,
};

// ── apply / clear / read / preview ──────────────────────────────────────────

pub(super) fn apply_faugus(game: &GameEntry, cmd: &str) -> Result<(String, Vec<String>), String> {
    let gid = game
        .launcher_id
        .as_deref()
        .ok_or_else(|| "Faugus : gameid manquant".to_string())?;
    let path = faugus_games_json().ok_or_else(|| "games.json Faugus introuvable".to_string())?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let arr = v
        .as_array_mut()
        .ok_or_else(|| "games.json : tableau attendu".to_string())?;
    let index = arr
        .iter()
        .position(|item| item.get("gameid").and_then(Value::as_str) == Some(gid))
        .ok_or_else(|| format!("Jeu Faugus « {gid} » introuvable dans games.json"))?;
    let (merged, managed, original) = merge_faugus_item(&mut arr[index], cmd)?;
    if managed.is_empty() {
        return Ok((
            "Les options demandées sont déjà présentes dans Faugus.".into(),
            managed,
        ));
    }

    let mut backups = load_faugus_backups(&path)?;
    if backups.get(gid).is_some_and(|saved| saved == &original) {
        backups.remove(gid);
    }
    if !backups.contains_key(gid) {
        backups.insert(gid.to_string(), original);
        save_faugus_backups(&path, &backups)?;
    }

    backup_file(&path)?;
    fs::write(
        &path,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((format!("Options Faugus fusionnées : {merged}."), managed))
}

pub(super) fn clear_faugus(game: &GameEntry, managed: &[String]) -> Result<String, String> {
    if managed.is_empty() {
        return Ok("Aucune option GameFocus à retirer de Faugus.".into());
    }
    let gid = game
        .launcher_id
        .as_deref()
        .ok_or_else(|| "Faugus : gameid manquant".to_string())?;
    let path = faugus_games_json().ok_or_else(|| "games.json Faugus introuvable".to_string())?;
    let mut backups = load_faugus_backups(&path)?;
    let original = backups
        .get(gid)
        .cloned()
        .ok_or_else(|| "Aucune sauvegarde GameFocus Faugus; options conservées.".to_string())?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let arr = v
        .as_array_mut()
        .ok_or_else(|| "games.json : tableau attendu".to_string())?;
    let index = arr
        .iter()
        .position(|item| item.get("gameid").and_then(Value::as_str) == Some(gid))
        .ok_or_else(|| format!("Jeu Faugus « {gid} » introuvable"))?;
    clear_faugus_item(&mut arr[index], &original, managed)?;
    backup_file(&path)?;
    fs::write(
        &path,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    backups.remove(gid);
    save_faugus_backups(&path, &backups)?;
    Ok("Options GameFocus retirées de Faugus; les autres réglages sont conservés.".into())
}

pub(super) fn read_faugus_launch_options(game: &GameEntry) -> Result<Option<String>, String> {
    let gid = game
        .launcher_id
        .as_deref()
        .ok_or_else(|| "Faugus : gameid manquant".to_string())?;
    let path = faugus_games_json().ok_or_else(|| "games.json Faugus introuvable".to_string())?;
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let item = value
        .as_array()
        .and_then(|games| {
            games
                .iter()
                .find(|item| item.get("gameid").and_then(Value::as_str) == Some(gid))
        })
        .ok_or_else(|| format!("Jeu Faugus « {gid} » introuvable dans games.json"))?;
    let command = faugus_command(item)?;
    Ok((command != "%command%").then_some(command))
}

pub(super) fn preview_faugus_options(
    current: &str,
    requested: &str,
) -> Result<(String, Vec<String>), String> {
    let current = split_launch_tokens(current)?;
    let requested = split_launch_tokens(requested)?;
    let (current_mangohud, current_gamemode, current_arguments, current_game_arguments) =
        faugus_split_options(&current);
    let (requested_mangohud, requested_gamemode, requested_arguments, requested_game_arguments) =
        faugus_split_options(&requested);
    let (arguments, added_arguments) =
        append_launch_arguments(&current_arguments.join(" "), &requested_arguments)?;
    let (game_arguments, added_game_arguments) =
        append_launch_arguments(&current_game_arguments.join(" "), &requested_game_arguments)?;
    let mut managed = Vec::new();
    let request_command = requested.iter().position(|token| token == "%command%");
    for (index, token) in requested
        .iter()
        .enumerate()
        .filter(|(_, token)| token.as_str() != "%command%")
    {
        match token.as_str() {
            "mangohud" if requested_mangohud && !current_mangohud => {
                managed.push("mangohud".into());
            }
            "gamemoderun" | "gamemode" | "faugus-gamemoderun"
                if requested_gamemode && !current_gamemode =>
            {
                if !managed.iter().any(|managed| managed == "gamemoderun") {
                    managed.push("gamemoderun".into());
                }
            }
            _ if added_arguments
                .iter()
                .any(|added| same_launch_option(added, token)) =>
            {
                managed.push(token.clone());
            }
            _ if request_command.is_some_and(|command| index > command)
                && added_game_arguments
                    .iter()
                    .any(|added| same_launch_option(added, token)) =>
            {
                managed.push(format!("{GAMEFOCUS_GAME_ARGUMENT_PREFIX}{token}"));
            }
            _ => {}
        }
    }
    let mut result = Vec::new();
    if !arguments.is_empty() {
        result.push(arguments);
    }
    if current_gamemode || requested_gamemode {
        result.push("gamemoderun".into());
    }
    if current_mangohud || requested_mangohud {
        result.push("mangohud".into());
    }
    result.push("%command%".into());
    result.extend(game_arguments.split_whitespace().map(str::to_string));
    Ok((result.join(" "), managed))
}

// ── internals ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct FaugusOriginal {
    pub(super) mangohud: Option<Value>,
    pub(super) gamemode: Option<Value>,
    pub(super) launch_arguments: Option<Value>,
    #[serde(default)]
    pub(super) game_arguments: Option<Value>,
}

fn faugus_backup_path(games_path: &Path) -> PathBuf {
    games_path.with_extension("gamefocus-backup.json")
}

fn load_faugus_backups(games_path: &Path) -> Result<HashMap<String, FaugusOriginal>, String> {
    let path = faugus_backup_path(games_path);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("lecture {} : {error}", path.display()))?;
    serde_json::from_str(&raw).map_err(|error| format!("sauvegarde Faugus invalide : {error}"))
}

fn save_faugus_backups(
    games_path: &Path,
    backups: &HashMap<String, FaugusOriginal>,
) -> Result<(), String> {
    let path = faugus_backup_path(games_path);
    let content = serde_json::to_string_pretty(backups).map_err(|error| error.to_string())?;
    fs::write(&path, content).map_err(|error| format!("écriture {} : {error}", path.display()))
}

fn faugus_command(item: &Value) -> Result<String, String> {
    let mut tokens = Vec::new();
    if let Some(arguments) = item
        .get("launch_arguments")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        tokens.push(arguments.trim());
    }
    if faugus_flag_is_enabled(item, "gamemode") {
        tokens.push("gamemoderun");
    }
    if faugus_flag_is_enabled(item, "mangohud") {
        tokens.push("mangohud");
    }
    tokens.push("%command%");
    if let Some(arguments) = item
        .get("game_arguments")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        tokens.push(arguments.trim());
    }
    Ok(tokens.join(" "))
}

fn merge_faugus_item(
    item: &mut Value,
    requested: &str,
) -> Result<(String, Vec<String>, FaugusOriginal), String> {
    let object = item
        .as_object()
        .ok_or_else(|| "Entrée Faugus invalide.".to_string())?;
    for field in ["launch_arguments", "game_arguments"] {
        if object
            .get(field)
            .is_some_and(|arguments| !arguments.is_string())
        {
            return Err(format!(
                "{field} Faugus n'est pas une chaîne; rien n'a été modifié."
            ));
        }
    }
    let original = FaugusOriginal {
        mangohud: object.get("mangohud").cloned(),
        gamemode: object.get("gamemode").cloned(),
        launch_arguments: object.get("launch_arguments").cloned(),
        game_arguments: object.get("game_arguments").cloned(),
    };
    let current_arguments = object
        .get("launch_arguments")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let requested = split_launch_tokens(requested)?;
    let (desired_mangohud, desired_gamemode, requested_arguments, requested_game_arguments) =
        faugus_split_options(&requested);
    let current_mangohud = faugus_flag_is_enabled(item, "mangohud");
    let current_gamemode = faugus_flag_is_enabled(item, "gamemode");
    let (arguments, managed_arguments) =
        append_launch_arguments(&current_arguments, &requested_arguments)?;
    let current_game_arguments = object
        .get("game_arguments")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let (game_arguments, managed_game_arguments) =
        append_launch_arguments(&current_game_arguments, &requested_game_arguments)?;
    let mut managed = Vec::new();
    let request_command = requested.iter().position(|token| token == "%command%");
    for (index, token) in requested
        .iter()
        .enumerate()
        .filter(|(_, token)| token.as_str() != "%command%")
    {
        match token.as_str() {
            "mangohud" if desired_mangohud && !current_mangohud => {
                if !managed.contains(token) {
                    managed.push("mangohud".to_string());
                }
            }
            "gamemoderun" | "gamemode" | "faugus-gamemoderun"
                if desired_gamemode && !current_gamemode =>
            {
                if !managed.iter().any(|managed| managed == "gamemoderun") {
                    managed.push("gamemoderun".to_string());
                }
            }
            _ if managed_arguments
                .iter()
                .any(|added| same_launch_option(added, token)) =>
            {
                managed.push(token.clone());
            }
            _ if request_command.is_some_and(|command| index > command)
                && managed_game_arguments
                    .iter()
                    .any(|added| same_launch_option(added, token)) =>
            {
                managed.push(format!("{GAMEFOCUS_GAME_ARGUMENT_PREFIX}{token}"));
            }
            _ => {}
        }
    }
    if managed.is_empty() {
        return Ok((faugus_command(item)?, managed, original));
    }

    let object = item
        .as_object_mut()
        .ok_or_else(|| "Entrée Faugus invalide.".to_string())?;
    if desired_mangohud {
        object.insert("mangohud".into(), json!(true));
    }
    if desired_gamemode {
        object.insert("gamemode".into(), json!(true));
    }
    if current_arguments != arguments {
        object.insert("launch_arguments".into(), json!(arguments));
    }
    if current_game_arguments != game_arguments {
        object.insert("game_arguments".into(), json!(game_arguments));
    }
    let merged = faugus_command(item)?;
    Ok((merged, managed, original))
}

fn clear_faugus_item(
    item: &mut Value,
    original: &FaugusOriginal,
    managed: &[String],
) -> Result<(), String> {
    let managed_arguments: Vec<_> = managed
        .iter()
        .filter(|token| {
            !token.starts_with(GAMEFOCUS_GAME_ARGUMENT_PREFIX)
                && token.as_str() != "mangohud"
                && token.as_str() != "gamemoderun"
        })
        .cloned()
        .collect();
    let managed_game_arguments: Vec<_> = managed
        .iter()
        .filter_map(|token| token.strip_prefix(GAMEFOCUS_GAME_ARGUMENT_PREFIX))
        .map(str::to_string)
        .collect();
    let current_arguments = item
        .get("launch_arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let remaining_args = remove_managed_arguments(current_arguments, &managed_arguments)?;
    let current_game_arguments = item
        .get("game_arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let remaining_game_arguments =
        remove_managed_arguments(current_game_arguments, &managed_game_arguments)?;
    if managed.iter().any(|token| token == "mangohud") && faugus_flag_is_enabled(item, "mangohud") {
        restore_faugus_field(item, "mangohud", original.mangohud.clone());
    }
    if managed.iter().any(|token| token == "gamemoderun")
        && faugus_flag_is_enabled(item, "gamemode")
    {
        restore_faugus_field(item, "gamemode", original.gamemode.clone());
    }
    let current_args = item
        .get("launch_arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if current_args != remaining_args {
        if remaining_args.is_empty() && original.launch_arguments.is_none() {
            restore_faugus_field(item, "launch_arguments", None);
        } else {
            item["launch_arguments"] = json!(remaining_args);
        }
    }
    let current_game_args = item
        .get("game_arguments")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if current_game_args != remaining_game_arguments {
        if remaining_game_arguments.is_empty() && original.game_arguments.is_none() {
            restore_faugus_field(item, "game_arguments", None);
        } else {
            item["game_arguments"] = json!(remaining_game_arguments);
        }
    }
    Ok(())
}

fn faugus_split_options(tokens: &[String]) -> (bool, bool, Vec<String>, Vec<String>) {
    let mut mangohud = false;
    let mut gamemode = false;
    let mut arguments = Vec::new();
    let command_index = tokens.iter().position(|token| token == "%command%");
    for token in tokens.iter().take(command_index.unwrap_or(tokens.len())) {
        match token.as_str() {
            "mangohud" => mangohud = true,
            "gamemoderun" | "gamemode" | "faugus-gamemoderun" => gamemode = true,
            _ => arguments.push(token.clone()),
        }
    }
    let game_arguments = command_index
        .map(|index| tokens[index + 1..].to_vec())
        .unwrap_or_default();
    (mangohud, gamemode, arguments, game_arguments)
}

fn faugus_flag_is_enabled(item: &Value, key: &str) -> bool {
    match item.get(key) {
        None => false,
        Some(value) if value.as_bool().is_some() => value.as_bool() == Some(true),
        Some(value) if value.as_str().is_some() => {
            let value = value.as_str().unwrap_or_default().trim();
            !matches!(
                value.to_ascii_lowercase().as_str(),
                "" | "false" | "0" | "off" | "no"
            )
        }
        Some(_) => true,
    }
}

fn restore_faugus_field(item: &mut Value, key: &str, value: Option<Value>) {
    if let Some(object) = item.as_object_mut() {
        if let Some(value) = value {
            object.insert(key.to_string(), value);
        } else {
            object.remove(key);
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn faugus_activation_and_removal_preserve_user_arguments() {
        let mut game = json!({
            "gamemode": false,
            "mangohud": false,
            "launch_arguments": "CUSTOM=1 --title \"my game\""
        });
        let original = game.clone();
        let (_, managed, saved) =
            merge_faugus_item(&mut game, "gamemoderun mangohud PROTON_LOG=1 %command%").unwrap();
        assert_eq!(
            game["launch_arguments"].as_str(),
            Some("CUSTOM=1 --title \"my game\" PROTON_LOG=1")
        );
        assert_eq!(game["gamemode"], json!(true));
        assert_eq!(game["mangohud"], json!(true));
        assert_eq!(
            faugus_command(&game).unwrap(),
            "CUSTOM=1 --title \"my game\" PROTON_LOG=1 gamemoderun mangohud %command%"
        );

        clear_faugus_item(&mut game, &saved, &managed).unwrap();
        assert_eq!(game, original);

        let (_, managed, saved) =
            merge_faugus_item(&mut game, "gamemoderun mangohud PROTON_LOG=1 %command%").unwrap();
        clear_faugus_item(&mut game, &saved, &managed).unwrap();
        assert_eq!(game, original);
    }

    #[test]
    fn faugus_string_flags_activate_and_restore_the_native_values() {
        let mut game = json!({
            "gamemode": "",
            "mangohud": "False",
            "launch_arguments": ""
        });
        assert!(!faugus_flag_is_enabled(&game, "gamemode"));
        assert!(!faugus_flag_is_enabled(&game, "mangohud"));

        let original = game.clone();
        let (_, managed, saved) =
            merge_faugus_item(&mut game, "gamemoderun mangohud %command%").unwrap();
        assert_eq!(game["gamemode"], json!(true));
        assert_eq!(game["mangohud"], json!(true));

        clear_faugus_item(&mut game, &saved, &managed).unwrap();
        assert_eq!(game, original);
    }

    #[test]
    fn faugus_clear_keeps_arguments_added_by_the_user_after_activation() {
        let mut game = json!({
            "gamemode": false,
            "launch_arguments": "--title \"my game\""
        });
        let (_, managed, saved) =
            merge_faugus_item(&mut game, "gamemoderun CUSTOM=1 %command%").unwrap();
        game["launch_arguments"] = json!("--title \"my game\" CUSTOM=1 USER_ADDED=1");
        clear_faugus_item(&mut game, &saved, &managed).unwrap();
        assert_eq!(
            game["launch_arguments"].as_str(),
            Some("--title \"my game\" USER_ADDED=1")
        );
        assert_eq!(game.get("gamemode"), Some(&json!(false)));
    }

    #[test]
    fn faugus_preview_matches_native_fields_and_preserves_argument_order() {
        assert_eq!(
            preview_faugus_options(
                "CUSTOM=1 gamemoderun %command%",
                "mangohud PROTON_LOG=1 %command%"
            )
            .unwrap(),
            (
                "CUSTOM=1 PROTON_LOG=1 gamemoderun mangohud %command%".into(),
                vec!["mangohud".into(), "PROTON_LOG=1".into()]
            )
        );
    }

    #[test]
    fn faugus_separates_launcher_and_game_arguments_for_enable_clear_reenable() {
        let mut game = json!({
            "gamemode": false,
            "launch_arguments": "USER_ENV=1",
            "game_arguments": "--existing"
        });
        let original = game.clone();
        let (_, managed, saved) =
            merge_faugus_item(&mut game, "gamemoderun USER_ENV2=2 %command% --new").unwrap();
        assert_eq!(game["launch_arguments"], json!("USER_ENV=1 USER_ENV2=2"));
        assert_eq!(game["game_arguments"], json!("--existing --new"));
        assert_eq!(
            faugus_command(&game).unwrap(),
            "USER_ENV=1 USER_ENV2=2 gamemoderun %command% --existing --new"
        );
        clear_faugus_item(&mut game, &saved, &managed).unwrap();
        assert_eq!(game, original);

        let (_, managed, saved) =
            merge_faugus_item(&mut game, "gamemoderun %command% --new").unwrap();
        clear_faugus_item(&mut game, &saved, &managed).unwrap();
        assert_eq!(game, original);
    }

    #[test]
    fn faugus_restore_preserves_original_values_and_missing_fields() {
        let original = FaugusOriginal {
            mangohud: Some(json!(true)),
            gamemode: None,
            launch_arguments: Some(json!("CUSTOM=1")),
            game_arguments: None,
        };
        let mut item = json!({
            "mangohud": false,
            "gamemode": true,
            "launch_arguments": "PROTON_LOG=1"
        });

        restore_faugus_field(&mut item, "mangohud", original.mangohud.clone());
        restore_faugus_field(&mut item, "gamemode", original.gamemode.clone());
        restore_faugus_field(
            &mut item,
            "launch_arguments",
            original.launch_arguments.clone(),
        );

        assert_eq!(item.get("mangohud"), original.mangohud.as_ref());
        assert!(item.get("gamemode").is_none());
        assert_eq!(
            item.get("launch_arguments"),
            original.launch_arguments.as_ref()
        );
    }
}
