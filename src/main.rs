//! Composition root : câble domaine ← ports ← adapters + CLI / UI.

use std::sync::Arc;

use gamefocus::adapters::config_json::JsonConfigStore;
use gamefocus::adapters::egui_ui;
use gamefocus::adapters::faugus_fs::FaugusLibrary;
use gamefocus::adapters::focus_sys::SysFocusController;
use gamefocus::adapters::heroic_fs::HeroicLibrary;
use gamefocus::adapters::lutris_fs::LutrisLibrary;
use gamefocus::adapters::protondb_http::{ProtonDbHttp, StaticProtonLookup};
use gamefocus::adapters::steam_fs::SteamLibrary;
use gamefocus::application::GameFocusService;
use gamefocus::domain::ProtonSummary;
use gamefocus::ports::{ConfigStore, GameLibrary, ProtonLookup};

fn print_help() {
    println!(
        "gamefocus — optimisations par jeu + mode Focus (GLF)\n\
         \n\
         Usage :\n\
           gamefocus                 Ouvre l'interface graphique\n\
           gamefocus --help          Cette aide\n\
           gamefocus --scan          Scanne Steam / Heroic (stdout)\n\
           gamefocus --focus-on      Active le mode Focus\n\
           gamefocus --focus-off     Quitte le mode Focus\n\
           gamefocus --demo          Cycle Focus en dry-run + scan démo\n\
         \n\
         Variables :\n\
           GAMEFOCUS_DRY_RUN=1       Focus sans signaux réels\n\
           GAMEFOCUS_DEMO=1          Idem + ProtonDB statique"
    );
}

fn build_service(demo: bool, dry_run: bool) -> Result<Arc<GameFocusService>, String> {
    let config_store: Arc<dyn ConfigStore> = Arc::new(JsonConfigStore::new()?);
    let cfg = config_store.load().unwrap_or_default();

    let steam: Arc<dyn GameLibrary> = Arc::new(SteamLibrary::new(cfg.extra_steam_roots.clone()));
    let heroic: Arc<dyn GameLibrary> = Arc::new(HeroicLibrary::new());
    let lutris: Arc<dyn GameLibrary> = Arc::new(LutrisLibrary::new());
    let faugus: Arc<dyn GameLibrary> = Arc::new(FaugusLibrary::new());

    let proton: Arc<dyn ProtonLookup> = if demo {
        Arc::new(StaticProtonLookup::new(vec![ProtonSummary {
            app_id: 1091500,
            tier: "gold".into(),
            total: 4200,
            trending_tier: Some("platinum".into()),
            best_reported_tier: None,
            confidence: Some("strong".into()),
            score: Some(0.82),
        }]))
    } else {
        Arc::new(ProtonDbHttp::new(cfg.protondb_timeout_secs))
    };

    let mut focus = SysFocusController::new(cfg.focus.clone());
    if !dry_run {
        focus.restore_persisted()?;
    }
    if dry_run || demo {
        focus = focus.dry_run(true);
        if demo {
            focus = focus.with_fake_processes(vec![
                (1, "systemd".into()),
                (10, "discord".into()),
                (20, "firefox".into()),
                (21, "chrome".into()),
                (30, "linux-wallpaperengine".into()),
                (40, "steam".into()),
            ]);
        }
    }

    Ok(Arc::new(GameFocusService::new(
        vec![steam, heroic, lutris, faugus],
        proton,
        Box::new(focus),
        config_store,
    )))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo_env = std::env::var("GAMEFOCUS_DEMO").ok().as_deref() == Some("1");
    let dry_env = std::env::var("GAMEFOCUS_DRY_RUN").ok().as_deref() == Some("1");

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let demo = demo_env || args.iter().any(|a| a == "--demo");
    let dry_run = dry_env || demo;

    let service = match build_service(demo, dry_run) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erreur initialisation : {e}");
            std::process::exit(1);
        }
    };

    if args.is_empty() && !demo {
        if let Err(e) = egui_ui::run(service) {
            eprintln!("Erreur UI : {e}");
            std::process::exit(1);
        }
        return;
    }

    // CLI
    for arg in &args {
        match arg.as_str() {
            "--scan" => match service.scan_games() {
                Ok(games) => {
                    println!("{} jeu(x) :", games.len());
                    for g in games {
                        let id = g
                            .app_id
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| "-".into());
                        println!("  [{}] {} ({})", id, g.name, g.source);
                    }
                }
                Err(e) => {
                    eprintln!("scan : {e}");
                    std::process::exit(1);
                }
            },
            "--focus-on" => match service.enter_focus() {
                Ok(s) => println!("{}", s.message),
                Err(e) => {
                    eprintln!("focus-on : {e}");
                    std::process::exit(1);
                }
            },
            "--focus-off" => match service.leave_focus() {
                Ok(s) => println!("{}", s.message),
                Err(e) => {
                    eprintln!("focus-off : {e}");
                    std::process::exit(1);
                }
            },
            "--demo" => {
                println!("=== Démo GameFocus (dry-run) ===");
                match service.scan_games() {
                    Ok(games) => {
                        println!("Scan : {} jeu(x)", games.len());
                        for g in games.iter().take(8) {
                            println!("  - {} ({})", g.name, g.source);
                        }
                        if let Some(g) = games
                            .iter()
                            .find(|g| g.name.to_lowercase().contains("cyberpunk"))
                            .or_else(|| games.first())
                        {
                            if let Ok(s) = service.suggest_for_game(g, true) {
                                println!("\nSuggestion « {} » : {}", g.name, s.recommended);
                                if let Some(p) = s.proton {
                                    println!("ProtonDB : {} ({} rapports)", p.tier, p.total);
                                }
                            }
                        }
                    }
                    Err(e) => println!("Scan (souvent vide hors machine jeu) : {e}"),
                }
                match service.demo_focus_cycle() {
                    Ok((enter, leave)) => {
                        println!("\nFocus : {}", enter.message);
                        println!("Leave : {}", leave.message);
                        println!("Discord / Steam / audio non ciblés (règles domaine).");
                    }
                    Err(e) => {
                        eprintln!("démo focus : {e}");
                        std::process::exit(1);
                    }
                }
                println!("=== Fin démo ===");
            }
            "-h" | "--help" => {}
            other => {
                eprintln!("Argument inconnu : {other}");
                print_help();
                std::process::exit(2);
            }
        }
    }

    // GAMEFOCUS_DEMO=1 sans args → démo CLI
    if args.is_empty() && demo {
        let _ = service.demo_focus_cycle();
        println!("Démo dry-run OK. Lancez sans GAMEFOCUS_DEMO pour l'UI.");
    }
}
