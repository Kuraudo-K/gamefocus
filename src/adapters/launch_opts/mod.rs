//! Appliquer / retirer les options de lancement dans Steam / Heroic / Faugus.

mod faugus;
mod heroic;
mod lutris;
mod steam;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{GameEntry, GameSource};

pub use steam::set_steam_launch_options;

const GAMEFOCUS_GAME_ARGUMENT_PREFIX: &str = "__gamefocus_game_argument__:";

// ── public API ───────────────────────────────────────────────────────────────

pub fn merge_launch_options(
    existing: Option<&str>,
    requested: &str,
) -> Result<(String, Vec<String>), String> {
    let existing = existing.unwrap_or_default();
    let current = split_launch_tokens(existing)?;
    let requested = split_launch_tokens(requested)?;
    let current_command = current.iter().position(|token| token == "%command%");
    let requested_command = requested.iter().position(|token| token == "%command%");
    let current_prefix = &current[..current_command.unwrap_or(current.len())];
    let requested_prefix_tokens = &requested[..requested_command.unwrap_or(requested.len())];
    let mut requested_prefix = Vec::new();
    for token in requested_prefix_tokens {
        if let Some((name, value)) = environment_assignment(token) {
            for existing in current_prefix
                .iter()
                .chain(requested_prefix.iter())
                .filter_map(|existing| environment_assignment(existing))
                .filter(|(existing_name, _)| *existing_name == name)
            {
                if existing.1 != value {
                    return Err(format!(
                        "La variable {name} existe déjà avec une valeur différente; elle est conservée."
                    ));
                }
            }
        }
        let already_present = current_prefix
            .iter()
            .any(|existing| same_launch_option(existing, token));
        if !already_present && !requested_prefix.contains(token) {
            requested_prefix.push(token.clone());
        }
    }

    let requested_suffix = requested_command
        .map(|index| &requested[index + 1..])
        .unwrap_or_default();
    let current_suffix = current_command
        .map(|index| &current[index + 1..])
        .unwrap_or_default();
    let mut managed = requested_prefix.clone();
    let mut output_suffix = Vec::new();
    for token in current_suffix
        .iter()
        .filter(|token| token.as_str() != "%command%")
    {
        if !output_suffix.contains(token) {
            output_suffix.push(token.clone());
        }
    }
    for token in requested_suffix
        .iter()
        .filter(|token| token.as_str() != "%command%")
    {
        if !output_suffix.contains(token) {
            output_suffix.push(token.clone());
            managed.push(token.clone());
        }
    }

    let mut output = requested_prefix;
    output.extend(current_prefix.iter().cloned());
    output.push("%command%".into());
    output.extend(output_suffix);

    Ok((output.join(" "), managed))
}

pub fn remove_managed_launch_options(current: &str, managed: &[String]) -> Result<String, String> {
    let tokens = split_launch_tokens(current)?;
    let mut remove_counts = HashMap::<String, usize>::new();
    for token in managed.iter().filter(|token| token.as_str() != "%command%") {
        *remove_counts
            .entry(launch_option_identity(token))
            .or_default() += 1;
    }

    let mut output = Vec::with_capacity(tokens.len());
    let mut command_written = false;
    for token in tokens {
        if token == "%command%" {
            if !command_written {
                output.push(token);
                command_written = true;
            }
            continue;
        }
        if let Some(count) = remove_counts.get_mut(&launch_option_identity(&token)) {
            if *count > 0 {
                *count -= 1;
                continue;
            }
        }
        output.push(token);
    }
    if !command_written {
        output.push("%command%".into());
    }
    Ok(output.join(" "))
}

pub fn managed_launch_options_status(
    current: Option<&str>,
    managed: &[String],
) -> Result<(usize, usize), String> {
    let present = present_managed_launch_options(current, managed)?;
    Ok((present.len(), managed.len()))
}

pub fn present_managed_launch_options(
    current: Option<&str>,
    managed: &[String],
) -> Result<Vec<String>, String> {
    let tokens = split_launch_tokens(current.unwrap_or_default())?;
    let command_index = tokens.iter().position(|token| token == "%command%");
    Ok(managed
        .iter()
        .filter(|managed| {
            if let Some(game_argument) = managed.strip_prefix(GAMEFOCUS_GAME_ARGUMENT_PREFIX) {
                let suffix = command_index
                    .map(|index| &tokens[index + 1..])
                    .unwrap_or_default();
                suffix
                    .iter()
                    .any(|token| same_launch_option(token, game_argument))
            } else {
                tokens
                    .iter()
                    .any(|token| same_launch_option(token, managed))
            }
        })
        .cloned()
        .collect())
}

/// Écrit la commande dans le launcher d'origine. Retourne un message statut.
pub fn apply_launch_options(
    game: &GameEntry,
    command: &str,
) -> Result<(String, Vec<String>), String> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err("Commande vide.".into());
    }
    match game.source {
        GameSource::Steam => steam::apply_steam(game, cmd),
        GameSource::Heroic => heroic::apply_heroic(game, cmd),
        GameSource::Faugus => faugus::apply_faugus(game, cmd),
        GameSource::Lutris => lutris::apply_lutris(game, cmd),
        GameSource::Manual => Err("Jeu manuel : rien à écrire.".into()),
    }
}

/// Retire les options précédemment écrites (best-effort).
pub fn clear_launch_options(game: &GameEntry, managed: &[String]) -> Result<String, String> {
    match game.source {
        GameSource::Steam => steam::clear_steam(game, managed),
        GameSource::Heroic => heroic::clear_heroic(game, managed),
        GameSource::Faugus => faugus::clear_faugus(game, managed),
        GameSource::Lutris => lutris::clear_lutris(game, managed),
        GameSource::Manual => Err("Jeu manuel.".into()),
    }
}

/// Lit les options réellement présentes dans le fichier local Steam.
pub fn read_steam_launch_options(game: &GameEntry) -> Result<Option<String>, String> {
    steam::read_steam_launch_options_impl(game)
}

pub fn read_launch_options(game: &GameEntry) -> Result<Option<String>, String> {
    match game.source {
        GameSource::Steam => steam::read_steam_launch_options_impl(game),
        GameSource::Heroic => heroic::read_heroic_launch_options(game),
        GameSource::Faugus => faugus::read_faugus_launch_options(game),
        GameSource::Lutris => lutris::read_lutris_launch_options(game),
        GameSource::Manual => Ok(None),
    }
}

pub fn preview_launch_options(
    game: &GameEntry,
    current: Option<&str>,
    requested: &str,
) -> Result<(String, Vec<String>), String> {
    match game.source {
        GameSource::Faugus => {
            faugus::preview_faugus_options(current.unwrap_or_default(), requested)
        }
        GameSource::Heroic => heroic::preview_heroic_options(game, requested),
        GameSource::Lutris => lutris::preview_lutris_options(game, requested),
        GameSource::Steam | GameSource::Manual => merge_launch_options(current, requested),
    }
}

// ── shared private helpers (child modules access via `super::`) ───────────────

fn environment_assignment(token: &str) -> Option<(&str, &str)> {
    let (name, value) = token.split_once('=')?;
    let mut characters = name.chars();
    let first = characters.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    let value = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value);
    Some((name, value))
}

fn launch_option_identity(token: &str) -> String {
    match environment_assignment(token) {
        Some((name, value)) => format!("environment\0{name}\0{value}"),
        None => token.to_string(),
    }
}

fn same_launch_option(left: &str, right: &str) -> bool {
    launch_option_identity(left) == launch_option_identity(right)
}

fn append_launch_arguments(
    existing: &str,
    requested: &[String],
) -> Result<(String, Vec<String>), String> {
    let mut existing_tokens = split_launch_tokens(existing)?;
    let mut added = Vec::new();
    for token in requested {
        if let Some((name, value)) = environment_assignment(token) {
            for existing in existing_tokens
                .iter()
                .chain(added.iter())
                .filter_map(|existing| environment_assignment(existing))
                .filter(|(existing_name, _)| *existing_name == name)
            {
                if existing.1 != value {
                    return Err(format!(
                        "La variable {name} existe déjà avec une valeur différente; elle est conservée."
                    ));
                }
            }
        }
        let already_present = existing_tokens
            .iter()
            .chain(added.iter())
            .any(|existing| same_launch_option(existing, token));
        if !already_present {
            added.push(token.clone());
        }
    }
    existing_tokens.extend(added.iter().cloned());
    Ok((existing_tokens.join(" "), added))
}

fn remove_managed_arguments(current: &str, managed: &[String]) -> Result<String, String> {
    if managed.is_empty() {
        return Ok(current.to_string());
    }
    let tokens = split_launch_tokens(current)?;
    let mut remove_counts = HashMap::<String, usize>::new();
    for token in managed {
        *remove_counts
            .entry(launch_option_identity(token))
            .or_default() += 1;
    }
    Ok(tokens
        .into_iter()
        .filter(|token| {
            if let Some(count) = remove_counts.get_mut(&launch_option_identity(token)) {
                if *count > 0 {
                    *count -= 1;
                    return false;
                }
            }
            true
        })
        .collect::<Vec<_>>()
        .join(" "))
}

fn split_launch_tokens(command: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;

    for character in command.chars() {
        if escaped {
            token.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            token.push(character);
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            token.push(character);
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        match character {
            '"' | '\'' => {
                quote = Some(character);
                token.push(character);
            }
            character if character.is_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            _ => token.push(character),
        }
    }

    if quote.is_some() {
        return Err("Guillemets non fermés dans les options de lancement.".into());
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.gamefocus.bak",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bak")
    ))
}

fn backup_file(path: &Path) -> Result<(), String> {
    let bak = path.with_extension(format!(
        "{}.gamefocus.bak",
        path.extension().and_then(|e| e.to_str()).unwrap_or("bak")
    ));
    fs::copy(path, &bak)
        .map(|_| ())
        .map_err(|error| format!("sauvegarde préalable {} : {error}", bak.display()))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_clear_only_removes_the_expected_command() {
        assert_eq!(
            remove_managed_launch_options(
                "gamemoderun CUSTOM=1 %command%",
                &["gamemoderun".into()]
            )
            .unwrap(),
            "CUSTOM=1 %command%"
        );
    }

    #[test]
    fn launch_options_merge_handles_empty_custom_and_multiple_environment_values() {
        assert_eq!(
            merge_launch_options(None, "gamemoderun mangohud %command%").unwrap(),
            (
                "gamemoderun mangohud %command%".into(),
                vec!["gamemoderun".into(), "mangohud".into()]
            )
        );
        assert_eq!(
            merge_launch_options(
                Some("PROTON_ENABLE_WAYLAND=1 %command%"),
                "gamemoderun mangohud %command%"
            )
            .unwrap()
            .0,
            "gamemoderun mangohud PROTON_ENABLE_WAYLAND=1 %command%"
        );
        assert_eq!(
            merge_launch_options(
                Some("PROTON_LOG=1 DXVK_HDR=1 %command%"),
                "gamemoderun %command%"
            )
            .unwrap()
            .0,
            "gamemoderun PROTON_LOG=1 DXVK_HDR=1 %command%"
        );
    }

    #[test]
    fn launch_options_merge_deduplicates_command_and_preexisting_gamefocus_tokens() {
        let (active, added) = merge_launch_options(
            Some("gamemoderun PROTON_LOG=1 %command% %command%"),
            "gamemoderun mangohud %command%",
        )
        .unwrap();
        assert_eq!(active, "mangohud gamemoderun PROTON_LOG=1 %command%");
        assert_eq!(added, ["mangohud"]);

        let (deactivated, _) = merge_launch_options(Some(&active), "mangohud %command%").unwrap();
        assert_eq!(deactivated, active);
        let (reactivated, newly_added) =
            merge_launch_options(Some("PROTON_LOG=1 %command%"), "gamemoderun %command%").unwrap();
        assert_eq!(reactivated, "gamemoderun PROTON_LOG=1 %command%");
        assert_eq!(newly_added, ["gamemoderun"]);
    }

    #[test]
    fn removing_owned_tokens_keeps_user_options_and_quotes_intact() {
        assert_eq!(
            remove_managed_launch_options(
                "gamemoderun PROTON_LOG=1 mangohud %command% --config \"a b\"",
                &["gamemoderun".into(), "mangohud".into()]
            )
            .unwrap(),
            "PROTON_LOG=1 %command% --config \"a b\""
        );
        assert_eq!(
            remove_managed_launch_options(
                "gamemoderun gamemoderun %command%",
                &["gamemoderun".into()]
            )
            .unwrap(),
            "gamemoderun %command%"
        );
    }

    #[test]
    fn launch_options_merge_rejects_conflicting_environment_values() {
        assert!(merge_launch_options(
            Some("PROTON_LOG=0 %command%"),
            "gamemoderun PROTON_LOG=1 %command%"
        )
        .is_err());
        assert_eq!(
            merge_launch_options(
                Some("PROTON_LOG=\"1\" %command%"),
                "gamemoderun PROTON_LOG=1 %command%"
            )
            .unwrap()
            .0,
            "gamemoderun PROTON_LOG=\"1\" %command%"
        );
    }

    #[test]
    fn launch_options_merge_keeps_command_arguments_after_command() {
        assert_eq!(
            merge_launch_options(
                Some("CUSTOM=1 %command% --existing"),
                "gamemoderun %command% --new"
            )
            .unwrap(),
            (
                "gamemoderun CUSTOM=1 %command% --existing --new".into(),
                vec!["gamemoderun".into(), "--new".into()]
            )
        );
    }
}
