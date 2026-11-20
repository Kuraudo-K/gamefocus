//! Options de lancement Lutris (YAML).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_yaml::Value as YamlValue;

use crate::domain::GameEntry;

use super::{
    append_launch_arguments, backup_file, backup_path, remove_managed_arguments,
    split_launch_tokens, GAMEFOCUS_GAME_ARGUMENT_PREFIX,
};

// ── apply / clear / read / preview ──────────────────────────────────────────

pub(super) fn apply_lutris(game: &GameEntry, requested: &str) -> Result<(String, Vec<String>), String> {
    let path = lutris_config_path(game)?;
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut config: YamlValue = serde_yaml::from_str(&raw)
        .map_err(|error| format!("configuration Lutris invalide : {error}"))?;
    let (merged, managed) = merge_lutris_config(&mut config, requested)?;
    if managed.is_empty() {
        return Ok((
            "Les options demandées sont déjà présentes dans Lutris.".into(),
            managed,
        ));
    }
    let updated = serde_yaml::to_string(&config)
        .map_err(|error| format!("sérialisation de la configuration Lutris : {error}"))?;
    let backup = backup_path(&path);
    if !backup.exists() {
        backup_file(&path)?;
    }
    fs::write(&path, updated).map_err(|error| error.to_string())?;
    Ok((format!("Options Lutris fusionnées : {merged}."), managed))
}

pub(super) fn clear_lutris(game: &GameEntry, managed: &[String]) -> Result<String, String> {
    if managed.is_empty() {
        return Ok("Aucune option GameFocus à retirer de Lutris.".into());
    }
    let path = lutris_config_path(game)?;
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut current: YamlValue = serde_yaml::from_str(&raw)
        .map_err(|error| format!("configuration Lutris invalide : {error}"))?;
    let backup = backup_path(&path);
    let original_raw = fs::read_to_string(&backup)
        .map_err(|error| format!("sauvegarde des options Lutris introuvable : {error}"))?;
    let original: YamlValue = serde_yaml::from_str(&original_raw)
        .map_err(|error| format!("sauvegarde Lutris invalide : {error}"))?;
    remove_lutris_managed(&mut current, &original, managed)?;
    let updated = serde_yaml::to_string(&current)
        .map_err(|error| format!("sérialisation de la configuration Lutris : {error}"))?;
    fs::write(&path, updated).map_err(|error| error.to_string())?;
    fs::remove_file(&backup)
        .map_err(|error| format!("suppression de la sauvegarde Lutris : {error}"))?;
    Ok("Options GameFocus retirées de Lutris; les autres réglages sont conservés.".into())
}

pub(super) fn read_lutris_launch_options(game: &GameEntry) -> Result<Option<String>, String> {
    let path = lutris_config_path(game)?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("lecture de la configuration Lutris : {error}"))?;
    let config: YamlValue = serde_yaml::from_str(&raw)
        .map_err(|error| format!("configuration YAML Lutris invalide : {error}"))?;
    let command = lutris_current_command(&config)?;
    Ok((!command.is_empty()).then_some(command))
}

pub(super) fn preview_lutris_options(
    game: &GameEntry,
    requested: &str,
) -> Result<(String, Vec<String>), String> {
    let path = lutris_config_path(game)?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("lecture de la configuration Lutris : {error}"))?;
    let mut config: YamlValue = serde_yaml::from_str(&raw)
        .map_err(|error| format!("configuration Lutris invalide : {error}"))?;
    merge_lutris_config(&mut config, requested)
}

// ── internals ────────────────────────────────────────────────────────────────

fn lutris_config_path(game: &GameEntry) -> Result<PathBuf, String> {
    let slug = game
        .launcher_id
        .as_deref()
        .ok_or_else(|| "Lutris : identifiant du jeu manquant.".to_string())?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME introuvable.".to_string())?;
    let db_paths = [
        home.join(".local/share/lutris/pga.db"),
        home.join(".var/app/net.lutris.Lutris/data/lutris/pga.db"),
    ];
    for db in db_paths.iter().filter(|path| path.is_file()) {
        let script = "import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); r=c.execute('SELECT configpath FROM games WHERE slug=? AND installed=1 ORDER BY lastplayed DESC LIMIT 1',(sys.argv[2],)).fetchone(); print(r[0] if r and r[0] else '')";
        let output = Command::new("python3")
            .args(["-c", script])
            .arg(db)
            .arg(slug)
            .output()
            .map_err(|error| format!("lecture de la base Lutris : {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "lecture de la base Lutris : {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let configured = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !configured.is_empty() {
            let configured = configured.strip_prefix("~/").map_or_else(
                || PathBuf::from(&configured),
                |relative| home.join(relative),
            );
            if configured.is_file() {
                return Ok(configured);
            }
        }
    }
    for directory in [
        home.join(".config/lutris/games"),
        home.join(".var/app/net.lutris.Lutris/config/lutris/games"),
    ] {
        for extension in ["yml", "yaml"] {
            let path = directory.join(format!("{slug}.{extension}"));
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err(format!("Configuration Lutris introuvable pour « {slug} »."))
}

fn yaml_key(key: &str) -> YamlValue {
    YamlValue::String(key.to_string())
}

fn yaml_env_value(value: &YamlValue) -> Result<String, String> {
    match value {
        YamlValue::String(value) => Ok(value.clone()),
        YamlValue::Bool(_) | YamlValue::Number(_) | YamlValue::Null => serde_yaml::to_string(value)
            .map(|value| value.trim().to_string())
            .map_err(|error| format!("valeur d'environnement Lutris invalide : {error}")),
        _ => Err("Valeur d'environnement Lutris non scalaire.".into()),
    }
}

fn lutris_system_mut(config: &mut YamlValue) -> Result<&mut serde_yaml::Mapping, String> {
    let root = config
        .as_mapping_mut()
        .ok_or_else(|| "Racine de configuration Lutris invalide.".to_string())?;
    let key = yaml_key("system");
    if !root.contains_key(&key) {
        root.insert(key.clone(), YamlValue::Mapping(serde_yaml::Mapping::new()));
    }
    root.get_mut(&key)
        .and_then(YamlValue::as_mapping_mut)
        .ok_or_else(|| "Section system Lutris invalide.".to_string())
}

fn lutris_current_command(config: &YamlValue) -> Result<String, String> {
    let mut tokens = Vec::new();
    if let Some(system) = config.get("system") {
        let system = system
            .as_mapping()
            .ok_or_else(|| "Section system Lutris invalide.".to_string())?;
        for (field, token) in [("gamemode", "gamemoderun"), ("mangohud", "mangohud")] {
            if system.get(yaml_key(field)).and_then(YamlValue::as_bool) == Some(true) {
                tokens.push(token.to_string());
            }
        }
        if let Some(prefix) = system
            .get(yaml_key("command_prefix"))
            .and_then(YamlValue::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            tokens.push(prefix.trim().to_string());
        }
        if let Some(environment) = system.get(yaml_key("env")).and_then(YamlValue::as_mapping) {
            let mut variables = Vec::new();
            for (key, value) in environment {
                let key = key
                    .as_str()
                    .ok_or_else(|| "Clé d'environnement Lutris non textuelle.".to_string())?;
                variables.push((key.to_string(), yaml_env_value(value)?));
            }
            variables.sort_by(|left, right| left.0.cmp(&right.0));
            tokens.extend(variables.into_iter().map(|(key, value)| {
                if value.chars().any(char::is_whitespace) {
                    format!("{key}=\"{}\"", value.replace('"', "\\\""))
                } else {
                    format!("{key}={value}")
                }
            }));
        }
    }
    if let Some(args) = config
        .get("game")
        .and_then(|game| game.get("args"))
        .and_then(YamlValue::as_str)
        .filter(|args| !args.trim().is_empty())
    {
        tokens.push("%command%".into());
        tokens.push(args.trim().into());
    }
    if !tokens.iter().any(|token| token == "%command%") && !tokens.is_empty() {
        tokens.push("%command%".into());
    }
    Ok(tokens.join(" "))
}

fn merge_lutris_config(
    config: &mut YamlValue,
    requested: &str,
) -> Result<(String, Vec<String>), String> {
    let requested = split_launch_tokens(requested)?;
    let command_index = requested.iter().position(|token| token == "%command%");
    let game_arguments = command_index
        .map(|index| requested[index + 1..].to_vec())
        .unwrap_or_default();
    let system = lutris_system_mut(config)?;
    let mut managed = Vec::new();
    for token in requested
        .iter()
        .take(command_index.unwrap_or(requested.len()))
        .filter(|token| token.as_str() != "%command%")
    {
        match token.as_str() {
            "gamemoderun" | "mangohud" => {
                let field = if token == "gamemoderun" {
                    "gamemode"
                } else {
                    "mangohud"
                };
                let key = yaml_key(field);
                let active = system.get(&key).and_then(YamlValue::as_bool) == Some(true);
                if !active && !managed.contains(token) {
                    system.insert(key, YamlValue::Bool(true));
                    managed.push(token.clone());
                }
            }
            assignment if assignment.contains('=') => {
                let (name, value) = assignment.split_once('=').unwrap_or_default();
                if name.is_empty() {
                    return Err("Variable d'environnement Lutris invalide.".into());
                }
                let value = value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .or_else(|| {
                        value
                            .strip_prefix('\'')
                            .and_then(|value| value.strip_suffix('\''))
                    })
                    .unwrap_or(value)
                    .replace("\\\"", "\"");
                let environment = system
                    .entry(yaml_key("env"))
                    .or_insert_with(|| YamlValue::Mapping(serde_yaml::Mapping::new()))
                    .as_mapping_mut()
                    .ok_or_else(|| "Section system.env Lutris invalide.".to_string())?;
                let key = yaml_key(name);
                if let Some(existing) = environment.get(&key) {
                    let existing = yaml_env_value(existing)?;
                    if existing != value {
                        return Err(format!(
                            "Lutris contient déjà {name} avec une autre valeur; elle est conservée."
                        ));
                    }
                } else {
                    environment.insert(key, YamlValue::String(value));
                    managed.push(token.clone());
                }
            }
            unsupported => {
                return Err(format!(
                    "Option « {unsupported} » non prise en charge par les champs natifs de Lutris."
                ));
            }
        }
    }
    if !game_arguments.is_empty() {
        let root = config
            .as_mapping_mut()
            .ok_or_else(|| "Racine de configuration Lutris invalide.".to_string())?;
        let game = root
            .entry(yaml_key("game"))
            .or_insert_with(|| YamlValue::Mapping(serde_yaml::Mapping::new()))
            .as_mapping_mut()
            .ok_or_else(|| "Section game Lutris invalide.".to_string())?;
        let key = yaml_key("args");
        let existing = game
            .get(&key)
            .and_then(YamlValue::as_str)
            .unwrap_or_default();
        let (updated, added) = append_launch_arguments(existing, &game_arguments)?;
        if !added.is_empty() {
            game.insert(key, YamlValue::String(updated));
            managed.extend(
                added
                    .into_iter()
                    .map(|arg| format!("{GAMEFOCUS_GAME_ARGUMENT_PREFIX}{arg}")),
            );
        }
    }
    let final_command = lutris_current_command(config)?;
    Ok((final_command, managed))
}

fn remove_lutris_managed(
    current: &mut YamlValue,
    original: &YamlValue,
    managed: &[String],
) -> Result<(), String> {
    for argument in managed
        .iter()
        .filter_map(|token| token.strip_prefix(GAMEFOCUS_GAME_ARGUMENT_PREFIX))
    {
        let root = current
            .as_mapping_mut()
            .ok_or_else(|| "Racine de configuration Lutris invalide.".to_string())?;
        if let Some(game) = root
            .get_mut(yaml_key("game"))
            .and_then(YamlValue::as_mapping_mut)
        {
            let key = yaml_key("args");
            if let Some(args) = game.get(&key).and_then(YamlValue::as_str) {
                let remaining = remove_managed_arguments(args, &[argument.to_string()])?;
                if remaining != args {
                    let had_original_args = original
                        .get("game")
                        .and_then(|game| game.get("args"))
                        .is_some();
                    if remaining.is_empty() && !had_original_args {
                        game.remove(&key);
                    } else {
                        game.insert(key, YamlValue::String(remaining));
                    }
                }
            }
        }
    }
    let system = lutris_system_mut(current)?;
    let empty_original_system = serde_yaml::Mapping::new();
    let original_system = original
        .get("system")
        .and_then(YamlValue::as_mapping)
        .unwrap_or(&empty_original_system);
    for token in managed {
        match token.as_str() {
            "gamemoderun" | "mangohud" => {
                let field = if token == "gamemoderun" {
                    "gamemode"
                } else {
                    "mangohud"
                };
                let key = yaml_key(field);
                if system.get(&key).and_then(YamlValue::as_bool) == Some(true) {
                    restore_yaml_mapping_key(system, original_system, &key);
                }
            }
            assignment if assignment.contains('=') => {
                let (name, value) = assignment.split_once('=').unwrap_or_default();
                let value = value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .or_else(|| {
                        value
                            .strip_prefix('\'')
                            .and_then(|value| value.strip_suffix('\''))
                    })
                    .unwrap_or(value)
                    .replace("\\\"", "\"");
                let env_key = yaml_key("env");
                let current_env = system
                    .get_mut(&env_key)
                    .and_then(YamlValue::as_mapping_mut)
                    .ok_or_else(|| "Section system.env Lutris introuvable.".to_string())?;
                let key = yaml_key(name);
                if current_env.get(&key).and_then(YamlValue::as_str) == Some(value.as_str()) {
                    let original_env = original_system
                        .get(&env_key)
                        .and_then(YamlValue::as_mapping)
                        .cloned()
                        .unwrap_or_default();
                    restore_yaml_mapping_key(current_env, &original_env, &key);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn restore_yaml_mapping_key(
    current: &mut serde_yaml::Mapping,
    original: &serde_yaml::Mapping,
    key: &YamlValue,
) {
    if let Some(value) = original.get(key) {
        current.insert(key.clone(), value.clone());
    } else {
        current.remove(key);
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use serde_yaml::Value as YamlValue;

    use super::*;

    #[test]
    fn lutris_adds_native_options_and_keeps_existing_environment() {
        let mut config: YamlValue = serde_yaml::from_str(
            r#"
game:
  exe: /games/example
  args: --existing
system:
  env:
    CUSTOM: keep
"#,
        )
        .unwrap();
        let (command, managed) = merge_lutris_config(
            &mut config,
            "gamemoderun mangohud PROTON_ENABLE_WAYLAND=1 %command% --test",
        )
        .unwrap();

        assert_eq!(
            command,
            "gamemoderun mangohud CUSTOM=keep PROTON_ENABLE_WAYLAND=1 %command% --existing --test"
        );
        assert_eq!(
            managed,
            [
                "gamemoderun",
                "mangohud",
                "PROTON_ENABLE_WAYLAND=1",
                "__gamefocus_game_argument__:--test"
            ]
        );
        assert_eq!(config["system"]["env"]["CUSTOM"].as_str(), Some("keep"));
        assert_eq!(
            config["system"]["env"]["PROTON_ENABLE_WAYLAND"].as_str(),
            Some("1")
        );
        assert_eq!(config["system"]["gamemode"].as_bool(), Some(true));
        assert_eq!(config["system"]["mangohud"].as_bool(), Some(true));
        assert_eq!(config["game"]["args"].as_str(), Some("--existing --test"));
    }

    #[test]
    fn lutris_clear_restores_only_owned_options_and_allows_reactivation() {
        let original: YamlValue = serde_yaml::from_str(
            r#"
game:
  exe: /games/example
  args: --original
system:
  gamemode: false
  env:
    USER_SETTING: preserve
"#,
        )
        .unwrap();
        let mut active = original.clone();
        let (_, managed) = merge_lutris_config(
            &mut active,
            "gamemoderun mangohud GF_TEST=enabled %command% --test",
        )
        .unwrap();
        active["system"]["env"]["MANUAL_CHANGE"] = YamlValue::String("keep".into());
        active["game"]["args"] = YamlValue::String("--original --test --user-added".into());

        remove_lutris_managed(&mut active, &original, &managed).unwrap();
        assert_eq!(active["system"]["gamemode"].as_bool(), Some(false));
        assert!(active["system"].get("mangohud").is_none());
        assert!(active["system"]["env"].get("GF_TEST").is_none());
        assert_eq!(
            active["system"]["env"]["USER_SETTING"].as_str(),
            Some("preserve")
        );
        assert_eq!(
            active["system"]["env"]["MANUAL_CHANGE"].as_str(),
            Some("keep")
        );
        assert_eq!(
            active["game"]["args"].as_str(),
            Some("--original --user-added")
        );

        let (_, reactivated) = merge_lutris_config(
            &mut active,
            "gamemoderun mangohud GF_TEST=enabled %command% --test",
        )
        .unwrap();
        assert_eq!(reactivated, managed);
    }

    #[test]
    fn lutris_does_not_claim_options_that_were_already_enabled() {
        let mut config: YamlValue = serde_yaml::from_str(
            r#"
system:
  gamemode: true
  env:
    EXISTING: user-value
"#,
        )
        .unwrap();

        let (_, managed) =
            merge_lutris_config(&mut config, "gamemoderun EXISTING=user-value %command%").unwrap();
        assert!(managed.is_empty());
        assert_eq!(
            config["system"]["env"]["EXISTING"].as_str(),
            Some("user-value")
        );
    }

    #[test]
    fn lutris_rejects_conflicting_or_unsupported_options_without_overwriting() {
        let mut config: YamlValue = serde_yaml::from_str(
            r#"
system:
  env:
    PROTON_LOG: "0"
"#,
        )
        .unwrap();
        let original = config.clone();

        assert!(merge_lutris_config(&mut config, "PROTON_LOG=1 %command%").is_err());
        assert_eq!(config, original);
        assert!(merge_lutris_config(&mut config, "arbitrary-wrapper %command%").is_err());
        assert_eq!(config, original);
    }
}
