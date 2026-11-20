//! Harness tmpdir : bibliothèque Steam/Heroic + config + Focus dry-run isolés.

mod common;

use std::sync::Arc;

use gamefocus::adapters::config_json::JsonConfigStore;
use gamefocus::adapters::focus_sys::SysFocusController;
use gamefocus::adapters::heroic_fs::HeroicLibrary;
use gamefocus::adapters::protondb_http::StaticProtonLookup;
use gamefocus::adapters::steam_fs::SteamLibrary;
use gamefocus::application::GameFocusService;
use gamefocus::domain::proton::parse_proton_summary_json;
use gamefocus::domain::{
    is_heavy_game, select_focus_targets, AppConfig, FocusAction, FocusRules, GameSource,
};
use gamefocus::ports::{ConfigStore, GameLibrary, ProtonLookup};

use common::{fixtures_root, FakeGameMachine};

fn build_service(machine: &FakeGameMachine) -> GameFocusService {
    let store = Arc::new(JsonConfigStore::with_path(machine.config_path.clone()));
    store.save(&AppConfig::default()).unwrap();

    let steam: Arc<dyn GameLibrary> =
        Arc::new(SteamLibrary::with_roots(vec![machine.steam_root.clone()]));
    let heroic: Arc<dyn GameLibrary> =
        Arc::new(HeroicLibrary::with_roots(vec![machine.heroic_root.clone()]));

    let gold = std::fs::read_to_string(fixtures_root().join("protondb_1091500.json")).unwrap();
    let bronze = std::fs::read_to_string(fixtures_root().join("protondb_bronze.json")).unwrap();
    let silver = std::fs::read_to_string(fixtures_root().join("protondb_silver.json")).unwrap();
    let proton: Arc<dyn ProtonLookup> = Arc::new(StaticProtonLookup::new(vec![
        parse_proton_summary_json(1091500, &gold).unwrap(),
        parse_proton_summary_json(1, &bronze).unwrap(),
        parse_proton_summary_json(2, &silver).unwrap(),
    ]));

    let focus = SysFocusController::new(FocusRules::default())
        .dry_run(true)
        .with_fake_processes(vec![
            (1, "systemd".into()),
            (10, "discord".into()),
            (11, "vesktop".into()),
            (20, "firefox".into()),
            (21, "chrome".into()),
            (22, "chromium".into()),
            (30, "linux-wallpaperengine".into()),
            (31, "waypaper".into()),
            (40, "steam".into()),
            (41, "steamwebhelper".into()),
            (50, "pipewire".into()),
            (51, "wireplumber".into()),
            (60, "slack".into()),
            (70, "gamefocus".into()),
        ]);

    GameFocusService::new(vec![steam, heroic], proton, Box::new(focus), store)
}

#[test]
fn harness_scans_steam_and_heroic_fixtures() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    let games = svc.scan_games().unwrap();

    assert!(
        games.len() >= 5,
        "attendu Steam (Cyberpunk, Dota, Skyrim) + Heroic (Celeste, CP Heroic, Witcher) ; got {}",
        games.len()
    );
    assert!(games.iter().any(|g| g.name == "Cyberpunk 2077"));
    assert!(games.iter().any(|g| g.name == "Dota 2"));
    assert!(games.iter().any(|g| g.name.contains("Skyrim")));
    assert!(games.iter().any(|g| g.name == "Celeste"));
    assert!(games.iter().any(|g| g.name.contains("Witcher")));
    assert!(games.iter().any(|g| g.source == GameSource::Heroic));
    assert!(!games
        .iter()
        .any(|g| g.name.to_lowercase().contains("proton")));
}

#[test]
fn harness_cyberpunk_suggestion_uses_safe_generic_profiles() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    let games = svc.scan_games().unwrap();
    let cp = games
        .iter()
        .find(|g| g.name == "Cyberpunk 2077" && g.source == GameSource::Steam)
        .expect("Cyberpunk Steam");

    assert!(is_heavy_game(&cp.name));
    let suggestion = svc.suggest_for_game(cp, true).unwrap();
    assert!(suggestion.proton.is_some());
    assert_eq!(suggestion.proton.as_ref().unwrap().tier, "gold");
    assert!(suggestion.profiles[0].label.contains("GameMode"));
    assert_eq!(suggestion.recommended, "gamemoderun %command%");
}

#[test]
fn harness_focus_keeps_discord_steam_audio() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    let (enter, leave) = svc.demo_focus_cycle().unwrap();

    assert_eq!(enter.action, FocusAction::Enter);
    // firefox, chrome, chromium, wallpaper×2, slack — pas discord/vesktop/steam/audio/gamefocus
    assert!(
        enter.suspended >= 5,
        "suspended={} message={}",
        enter.suspended,
        enter.message
    );
    assert!(enter.message.contains("Focus") || enter.message.contains("gel"));
    assert_eq!(leave.action, FocusAction::Leave);
    assert!(!svc.is_focus_active());
}

#[test]
fn harness_focus_enter_idempotent_then_leave() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    svc.set_focus_dry_run(true);

    let first = svc.enter_focus().unwrap();
    assert!(svc.is_focus_active());
    let second = svc.enter_focus().unwrap();
    assert!(second.message.to_lowercase().contains("déjà") || second.suspended == first.suspended);
    let leave = svc.leave_focus().unwrap();
    assert_eq!(leave.action, FocusAction::Leave);
    assert!(!svc.is_focus_active());
}

#[test]
fn harness_config_persists_under_tmpdir() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    let mut cfg = svc.load_config();
    cfg.last_search = "skyrim".into();
    cfg.focus.keep.push("obs".into());
    cfg.protondb_timeout_secs = 12;
    svc.save_config(&cfg).unwrap();

    let store = JsonConfigStore::with_path(machine.config_path.clone());
    let loaded = store.load().unwrap();
    assert_eq!(loaded.last_search, "skyrim");
    assert_eq!(loaded.protondb_timeout_secs, 12);
    assert!(loaded.focus.keep.iter().any(|k| k == "obs"));
}

#[test]
fn harness_dedup_same_name_different_sources() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    let games = svc.scan_games().unwrap();
    // Cyberpunk Steam + Cyberpunk Heroic doivent coexister
    let cps: Vec<_> = games
        .iter()
        .filter(|g| g.name.to_lowercase().contains("cyberpunk"))
        .collect();
    assert!(cps.len() >= 2, "sources distinctes attendues : {:?}", cps);
}

#[test]
fn harness_select_targets_pure_matches_controller() {
    let rules = FocusRules::default();
    let procs = [
        (10u32, "discord"),
        (20, "firefox"),
        (30, "waypaper"),
        (40, "steam"),
        (50, "pipewire"),
    ];
    let targets = select_focus_targets(&procs, &rules);
    assert_eq!(targets, vec![20, 30]);
}

#[test]
fn harness_empty_steam_root_ok() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty-steam");
    std::fs::create_dir_all(&empty).unwrap();
    let lib = SteamLibrary::with_roots(vec![empty]);
    let games = lib.scan().unwrap();
    assert!(games.is_empty());
}

#[test]
fn harness_proton_lookup_miss_still_suggests() {
    let machine = FakeGameMachine::new();
    let svc = build_service(&machine);
    let games = svc.scan_games().unwrap();
    let dota = games.iter().find(|g| g.name == "Dota 2").unwrap();
    // pas d'entrée StaticProton pour 570 → proton None, profils de base OK
    let s = svc.suggest_for_game(dota, true).unwrap();
    assert!(s.proton.is_none());
    assert!(!s.profiles.is_empty());
    assert_eq!(s.recommended, "gamemoderun %command%");
}
