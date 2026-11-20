//! Lancement d'un jeu via Steam / Heroic / Faugus / Lutris.

use std::process::Command;

use crate::domain::{GameEntry, GameSource};

/// Lance le jeu avec le launcher d'origine.
pub fn launch_game(game: &GameEntry) -> Result<String, String> {
    match game.source {
        GameSource::Steam => {
            let id = game.app_id.ok_or_else(|| "pas d'AppID Steam".to_string())?;
            spawn_detached("steam", &[&format!("steam://rungameid/{id}")])?;
            Ok(format!("Steam lance le jeu (appid {id})."))
        }
        GameSource::Heroic => {
            let app = game
                .launcher_id
                .clone()
                .or_else(|| game.app_id.map(|i| i.to_string()))
                .ok_or_else(|| "Heroic : id manquant — ouvre Heroic manuellement".to_string())?;
            let uri = format!("heroic://launch?appName={app}");
            if spawn_detached("xdg-open", &[&uri]).is_err() {
                spawn_detached("heroic", &[&uri])?;
            }
            Ok(format!("Heroic : lancement demandé ({app})."))
        }
        GameSource::Faugus => {
            if let Some(ref gid) = game.launcher_id {
                // CLI officiel : faugus-launcher --game <gameid>
                if spawn_detached("faugus-launcher", &["--game", gid.as_str()]).is_err() {
                    spawn_detached("faugus-run", &["--game", gid.as_str()])?;
                }
                Ok(format!("Faugus lance « {gid} »."))
            } else {
                spawn_detached("faugus-launcher", &[])?;
                Ok("Faugus Launcher ouvert — choisis le jeu dedans.".into())
            }
        }
        GameSource::Lutris => {
            let slug = game
                .launcher_id
                .clone()
                .ok_or_else(|| "Lutris : slug manquant".to_string())?;
            spawn_detached("lutris", &[&format!("lutris:rungame/{slug}")])?;
            Ok(format!("Lutris lance « {slug} »."))
        }
        GameSource::Manual => Err("Jeu manuel : pas de lanceur.".into()),
    }
}

fn spawn_detached(cmd: &str, args: &[&str]) -> Result<(), String> {
    Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("impossible de lancer {cmd}: {e}"))?;
    Ok(())
}
