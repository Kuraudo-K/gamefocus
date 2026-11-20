//! Options de lancement Heroic (JSON GamesConfig).

use std::fs;

use serde_json::{json, Value};

use crate::adapters::shared::find_heroic_game_config;
use crate::domain::GameEntry;

use super::{
    append_launch_arguments, backup_file, environment_assignment,
    remove_managed_arguments, split_launch_tokens, GAMEFOCUS_GAME_ARGUMENT_PREFIX,
};

// ── apply / clear / read / preview ──────────────────────────────────────────

pub(super) fn apply_heroic(game: &GameEntry, cmd: &str) -> Result<(String, Vec<String>), String> {
    let app = game
        .launcher_id
        .clone()
        .or_else(|| game.app_id.map(|i| i.to_string()))
        .ok_or_else(|| "Heroic : id manquant".to_string())?;
    let path = find_heroic_game_config(&app)
        .ok_or_else(|| format!("GamesConfig Heroic introuvable pour {app}"))?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let entry = heroic_game_entry_mut(&mut v, &app)?;
    let (merged, managed) = merge_heroic_entry(entry, cmd)?;
    if managed.is_empty() {
        return Ok((
            "Les options demandées sont déjà présentes dans Heroic.".into(),
            managed,
        ));
    }
    backup_file(&path)?;
    fs::write(
        &path,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((format!("Options Heroic fusionnées : {merged}."), managed))
}

pub(super) fn clear_heroic(game: &GameEntry, managed: &[String]) -> Result<String, String> {
    if managed.is_empty() {
        return Ok("Aucune option GameFocus à retirer de Heroic.".into());
    }
    let app = game
        .launcher_id
        .clone()
        .or_else(|| game.app_id.map(|i| i.to_string()))
        .ok_or_else(|| "Heroic : id manquant".to_string())?;
    let path = find_heroic_game_config(&app)
        .ok_or_else(|| format!("GamesConfig Heroic introuvable pour {app}"))?;
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let entry = heroic_game_entry_mut(&mut v, &app)?;
    if !clear_heroic_entry(entry, managed)? {
        return Ok("Aucune option GameFocus à retirer dans Heroic.".into());
    }
    backup_file(&path)?;
    fs::write(
        &path,
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok("Options GameFocus retirées de Heroic; les autres wrappers sont conservés.".into())
}

pub(super) fn read_heroic_launch_options(game: &GameEntry) -> Result<Option<String>, String> {
    let app = game
        .launcher_id
        .clone()
        .or_else(|| game.app_id.map(|id| id.to_string()))
        .ok_or_else(|| "Heroic : id manquant".to_string())?;
    let path = find_heroic_game_config(&app)
        .ok_or_else(|| format!("GamesConfig Heroic introuvable pour {app}"))?;
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    let entry = heroic_game_entry(&value, &app)?;
    let command = heroic_entry_command(entry)?;
    Ok((command != "%command%").then_some(command))
}

pub(super) fn preview_heroic_options(
    game: &GameEntry,
    requested: &str,
) -> Result<(String, Vec<String>), String> {
    let app = game
        .launcher_id
        .clone()
        .or_else(|| game.app_id.map(|id| id.to_string()))
        .ok_or_else(|| "Heroic : id manquant".to_string())?;
    let path = find_heroic_game_config(&app)
        .ok_or_else(|| format!("GamesConfig Heroic introuvable pour {app}"))?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("lecture de la configuration Heroic : {error}"))?;
    let mut config: Value = serde_json::from_str(&raw)
        .map_err(|error| format!("configuration Heroic invalide : {error}"))?;
    let entry = heroic_game_entry_mut(&mut config, &app)?;
    merge_heroic_entry(entry, requested)
}

// ── internals ────────────────────────────────────────────────────────────────

fn merge_heroic_entry(entry: &mut Value, requested: &str) -> Result<(String, Vec<String>), String> {
    let tokens = split_launch_tokens(requested)?;
    let command_index = tokens.iter().position(|token| token == "%command%");
    let before_command = tokens
        .iter()
        .take(command_index.unwrap_or(tokens.len()))
        .cloned()
        .collect::<Vec<_>>();
    let after_command = command_index
        .map(|index| tokens[index + 1..].to_vec())
        .unwrap_or_default();
    let entry = entry
        .as_object_mut()
        .ok_or_else(|| "Entrée jeu Heroic invalide.".to_string())?;
    let mut managed = Vec::new();
    let env_key = "enviromentOptions";
    for token in before_command {
        match token.as_str() {
            "gamemoderun" => {
                if entry.get("useGameMode").and_then(Value::as_bool) != Some(true) {
                    entry.insert("useGameMode".into(), json!(true));
                    managed.push(token);
                }
            }
            "mangohud" => {
                let field = if entry.contains_key("showMangohud") || !entry.contains_key("showFps")
                {
                    "showMangohud"
                } else {
                    "showFps"
                };
                if entry.get(field).and_then(Value::as_bool) != Some(true) {
                    entry.insert(field.into(), json!(true));
                    managed.push(token);
                }
            }
            assignment if environment_assignment(assignment).is_some() => {
                let (name, value) = environment_assignment(assignment).unwrap();
                let env = entry.entry(env_key).or_insert_with(|| json!([]));
                let env = env.as_array_mut().ok_or_else(|| {
                    "enviromentOptions Heroic invalide; rien n'a été modifié.".to_string()
                })?;
                let existing = env
                    .iter()
                    .find(|item| item.get("key").and_then(Value::as_str) == Some(name));
                if let Some(existing) = existing {
                    if existing.get("value").and_then(Value::as_str) != Some(value) {
                        return Err(format!(
                            "Heroic contient déjà {name} avec une autre valeur; elle est conservée."
                        ));
                    }
                } else {
                    env.push(json!({ "key": name, "value": value }));
                    managed.push(token);
                }
            }
            unsupported => {
                return Err(format!(
                    "Option « {unsupported} » non prise en charge par les champs natifs de Heroic."
                ));
            }
        }
    }
    let current_args = entry
        .get("launcherArgs")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (updated_args, added_args) = append_launch_arguments(current_args, &after_command)?;
    if !added_args.is_empty() {
        entry.insert("launcherArgs".into(), json!(updated_args));
        managed.extend(
            added_args
                .into_iter()
                .map(|arg| format!("{GAMEFOCUS_GAME_ARGUMENT_PREFIX}{arg}")),
        );
    }
    let merged = heroic_entry_command(&Value::Object(entry.clone()))?;
    Ok((merged, managed))
}

fn clear_heroic_entry(entry: &mut Value, managed: &[String]) -> Result<bool, String> {
    let entry = entry
        .as_object_mut()
        .ok_or_else(|| "Entrée jeu Heroic invalide.".to_string())?;
    let mut changed = false;
    for (flag, field) in [("gamemoderun", "useGameMode")] {
        if managed.iter().any(|token| token == flag)
            && entry.get(field).and_then(Value::as_bool) == Some(true)
        {
            entry.insert(field.into(), json!(false));
            changed = true;
        }
    }
    if managed.iter().any(|token| token == "mangohud") {
        for field in ["showMangohud", "showFps"] {
            if entry.get(field).and_then(Value::as_bool) == Some(true) {
                entry.insert(field.into(), json!(false));
                changed = true;
                break;
            }
        }
    }
    let managed_env: Vec<_> = managed
        .iter()
        .filter_map(|token| environment_assignment(token))
        .collect();
    if !managed_env.is_empty() {
        if let Some(env) = entry
            .get_mut("enviromentOptions")
            .and_then(Value::as_array_mut)
        {
            let before = env.len();
            env.retain(|item| {
                !managed_env.iter().any(|(name, value)| {
                    item.get("key").and_then(Value::as_str) == Some(*name)
                        && item.get("value").and_then(Value::as_str) == Some(*value)
                })
            });
            changed |= env.len() != before;
        }
    }
    let managed_args: Vec<_> = managed
        .iter()
        .filter_map(|token| token.strip_prefix(GAMEFOCUS_GAME_ARGUMENT_PREFIX))
        .map(str::to_string)
        .collect();
    if !managed_args.is_empty() {
        let current = entry
            .get("launcherArgs")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let remaining = remove_managed_arguments(current, &managed_args)?;
        if remaining != current {
            entry.insert("launcherArgs".into(), json!(remaining));
            changed = true;
        }
    }
    if let Some(env) = entry.get("enviromentOptions").and_then(Value::as_array) {
        if env.is_empty() {
            entry.remove("enviromentOptions");
        }
    }
    if entry.get("launcherArgs").and_then(Value::as_str) == Some("") {
        entry.remove("launcherArgs");
    }
    return Ok(changed);
}

fn heroic_game_entry<'a>(root: &'a Value, app: &str) -> Result<&'a Value, String> {
    let object = root
        .as_object()
        .ok_or_else(|| "GamesConfig Heroic invalide".to_string())?;
    if let Some(entry) = object.get(app).filter(|entry| entry.is_object()) {
        return Ok(entry);
    }
    let matches: Vec<_> = object
        .values()
        .filter(|entry| {
            entry.is_object()
                && ["appName", "app_name", "appId", "app_id", "id"]
                    .iter()
                    .any(|key| entry.get(*key).and_then(Value::as_str) == Some(app))
        })
        .collect();
    match matches.as_slice() {
        [entry] => Ok(*entry),
        [] if object.len() == 1 => object
            .values()
            .next()
            .filter(|entry| entry.is_object())
            .ok_or_else(|| format!("Entrée de jeu Heroic « {app} » introuvable.")),
        [] => Err(format!("Entrée de jeu Heroic « {app} » introuvable.")),
        _ => Err(format!(
            "Plusieurs entrées Heroic correspondent à « {app} »."
        )),
    }
}

fn heroic_game_entry_mut<'a>(root: &'a mut Value, app: &str) -> Result<&'a mut Value, String> {
    let object = root
        .as_object_mut()
        .ok_or_else(|| "GamesConfig Heroic invalide".to_string())?;
    if object.get(app).is_some_and(Value::is_object) {
        return object
            .get_mut(app)
            .ok_or_else(|| "Entrée Heroic disparue.".into());
    }
    let keys: Vec<String> = object
        .iter()
        .filter(|(_, entry)| {
            entry.is_object()
                && ["appName", "app_name", "appId", "app_id", "id"]
                    .iter()
                    .any(|key| entry.get(*key).and_then(Value::as_str) == Some(app))
        })
        .map(|(key, _)| key.clone())
        .collect();
    match keys.as_slice() {
        [key] => object
            .get_mut(key)
            .ok_or_else(|| "Entrée Heroic disparue.".into()),
        [] if object.len() == 1 => {
            let key = object
                .keys()
                .next()
                .cloned()
                .ok_or_else(|| format!("Entrée de jeu Heroic « {app} » introuvable."))?;
            object
                .get_mut(&key)
                .filter(|entry| entry.is_object())
                .ok_or_else(|| format!("Entrée de jeu Heroic « {app} » introuvable."))
        }
        [] => Err(format!("Entrée de jeu Heroic « {app} » introuvable.")),
        _ => Err(format!(
            "Plusieurs entrées Heroic correspondent à « {app} »."
        )),
    }
}

fn heroic_entry_command(entry: &Value) -> Result<String, String> {
    let object = entry
        .as_object()
        .ok_or_else(|| "Entrée Heroic invalide.".to_string())?;
    let mut options = Vec::new();
    if object.get("useGameMode").and_then(Value::as_bool) == Some(true) {
        options.push("gamemoderun".to_string());
    }
    if ["showMangohud", "showFps"]
        .iter()
        .any(|field| object.get(*field).and_then(Value::as_bool) == Some(true))
    {
        options.push("mangohud".to_string());
    }
    if let Some(env) = object.get("enviromentOptions") {
        for variable in env
            .as_array()
            .ok_or_else(|| "enviromentOptions Heroic invalide.".to_string())?
        {
            let key = variable
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| "Clé d'environnement Heroic invalide.".to_string())?;
            let value = variable
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| "Valeur d'environnement Heroic invalide.".to_string())?;
            if value.chars().any(char::is_whitespace) {
                options.push(format!("{key}=\"{}\"", value.replace('"', "\\\"")));
            } else {
                options.push(format!("{key}={value}"));
            }
        }
    }
    if let Some(args) = object.get("launcherArgs").and_then(Value::as_str) {
        if !args.trim().is_empty() {
            options.push("%command%".to_string());
            options.push(args.trim().to_string());
        }
    }
    if !options.iter().any(|token| token == "%command%") {
        options.push("%command%".into());
    }
    Ok(options.join(" "))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn heroic_uses_native_fields_and_preserves_user_settings() {
        let mut entry = json!({
            "appName": "Example",
            "wrapperOptions": [
                {"exe": "custom-wrapper", "args": "--user-setting"}
            ],
            "launcherArgs": "--existing",
            "enviromentOptions": [{"key": "USER_ENV", "value": "keep"}],
            "useGameMode": false,
            "showFps": false
        });
        let original = entry.clone();
        let (merged, managed) = merge_heroic_entry(
            &mut entry,
            "gamemoderun mangohud PROTON_ENABLE_WAYLAND=1 %command% --test",
        )
        .unwrap();
        assert_eq!(
            managed,
            [
                "gamemoderun",
                "mangohud",
                "PROTON_ENABLE_WAYLAND=1",
                "__gamefocus_game_argument__:--test"
            ]
        );
        assert_eq!(entry["useGameMode"], json!(true));
        assert_eq!(entry["showFps"], json!(true));
        assert!(entry.get("showMangohud").is_none());
        assert_eq!(
            entry["enviromentOptions"][0],
            json!({"key": "USER_ENV", "value": "keep"})
        );
        assert_eq!(entry["launcherArgs"].as_str(), Some("--existing --test"));
        assert!(merged.contains("gamemoderun"));
        assert!(merged.contains("mangohud"));
        assert!(merged.contains("PROTON_ENABLE_WAYLAND=1"));
        assert_eq!(
            entry["wrapperOptions"][0]["args"].as_str(),
            Some("--user-setting")
        );

        entry["launcherArgs"] = json!("--existing --test --user-added");
        entry["enviromentOptions"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "key": "USER_ADDED",
                "value": "preserve"
            }));
        assert!(clear_heroic_entry(&mut entry, &managed).unwrap());
        assert_eq!(entry["useGameMode"], json!(false));
        assert_eq!(entry["showFps"], json!(false));
        assert_eq!(
            entry["launcherArgs"].as_str(),
            Some("--existing --user-added")
        );
        assert_eq!(
            entry["enviromentOptions"],
            json!([
                {"key": "USER_ENV", "value": "keep"},
                {"key": "USER_ADDED", "value": "preserve"}
            ])
        );
        assert_eq!(entry["wrapperOptions"], original["wrapperOptions"]);

        let (_, reactivated) =
            merge_heroic_entry(&mut entry, "gamemoderun mangohud %command% --test").unwrap();
        assert!(reactivated.contains(&"gamemoderun".to_string()));
        assert!(reactivated.contains(&"mangohud".to_string()));
        assert!(reactivated.contains(&"__gamefocus_game_argument__:--test".to_string()));
    }
}
