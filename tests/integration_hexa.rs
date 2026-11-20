//! Tests d'intégration : fixtures Steam/Heroic + suggestions + focus dry-run.

use std::path::PathBuf;
use std::sync::Arc;

use gamefocus::adapters::config_json::JsonConfigStore;
use gamefocus::adapters::focus_sys::SysFocusController;
use gamefocus::adapters::heroic_fs::HeroicLibrary;
use gamefocus::adapters::protondb_http::StaticProtonLookup;
use gamefocus::adapters::steam_fs::{parse_library_folders_vdf, SteamLibrary};
use gamefocus::application::GameFocusService;
use gamefocus::domain::proton::parse_proton_summary_json;
use gamefocus::domain::{
    is_heavy_game, suggest_launch_profiles, AppConfig, FocusAction, FocusRules, GameSource,
};
use gamefocus::ports::{ConfigStore, GameLibrary, ProtonLookup};
use tempfile::tempdir;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn steam_fixture_scan_skips_proton() {
    let steam_root = fixtures_root().join("steam");
    let lib = SteamLibrary::with_roots(vec![steam_root]);
    let games = lib.scan().unwrap();
    assert!(games.iter().any(|g| g.name == "Cyberpunk 2077"));
    assert!(games.iter().any(|g| g.name == "Dota 2"));
    assert!(games.iter().any(|g| g.name.contains("Skyrim")));
    assert!(!games.iter().any(|g| g.name.contains("Proton")));
    assert_eq!(
        games
            .iter()
            .find(|g| g.name.contains("Cyberpunk"))
            .unwrap()
            .app_id,
        Some(1091500)
    );
}

#[test]
fn heroic_fixture_scan() {
    let root = fixtures_root().join("heroic");
    let lib = HeroicLibrary::with_roots(vec![root]);
    let games = lib.scan().unwrap();
    assert!(games.iter().any(|g| g.name.contains("Celeste")));
    assert!(games.iter().any(|g| g.name.contains("Witcher")));
    assert!(games.iter().any(|g| g.source == GameSource::Heroic));
}

#[test]
fn protondb_fixture_and_launch_suggestions() {
    let raw = std::fs::read_to_string(fixtures_root().join("protondb_1091500.json")).unwrap();
    let summary = parse_proton_summary_json(1091500, &raw).unwrap();
    assert_eq!(summary.tier, "gold");
    assert!(is_heavy_game("Cyberpunk 2077"));
    let profiles = suggest_launch_profiles(Some(&summary), true);
    assert!(profiles[0].label.contains("GameMode"));
    assert_eq!(profiles[0].command, "gamemoderun %command%");
    assert!(profiles
        .iter()
        .any(|profile| profile.command.contains("gamemoderun")));
}

#[test]
fn vdf_parser_extracts_paths() {
    let raw = r#"
"libraryfolders"
{
	"0" { "path"		"/home/user/.local/share/Steam" }
	"1" { "path"		"/mnt/games/SteamLibrary" }
}
"#;
    let paths = parse_library_folders_vdf(raw);
    assert_eq!(paths.len(), 2);
}

#[test]
fn service_scan_suggest_focus_dry_run() {
    let dir = tempdir().unwrap();
    let cfg_path = dir.path().join("config.json");
    let store = Arc::new(JsonConfigStore::with_path(cfg_path));
    store.save(&AppConfig::default()).unwrap();

    let steam = Arc::new(SteamLibrary::with_roots(
        vec![fixtures_root().join("steam")],
    ));
    let heroic = Arc::new(HeroicLibrary::with_roots(vec![
        fixtures_root().join("heroic")
    ]));

    let raw = std::fs::read_to_string(fixtures_root().join("protondb_1091500.json")).unwrap();
    let summary = parse_proton_summary_json(1091500, &raw).unwrap();
    let proton: Arc<dyn ProtonLookup> = Arc::new(StaticProtonLookup::new(vec![summary]));

    let focus = SysFocusController::new(FocusRules::default())
        .dry_run(true)
        .with_fake_processes(vec![
            (10, "discord".into()),
            (20, "firefox".into()),
            (30, "waypaper".into()),
            (40, "steam".into()),
            (50, "pipewire".into()),
        ]);

    let svc = GameFocusService::new(vec![steam, heroic], proton, Box::new(focus), store);

    let games = svc.scan_games().unwrap();
    assert!(games.len() >= 3);

    let cp = games
        .iter()
        .find(|g| g.name == "Cyberpunk 2077")
        .expect("cyberpunk steam");
    let suggestion = svc.suggest_for_game(cp, true).unwrap();
    assert!(suggestion.proton.is_some());
    assert_eq!(suggestion.recommended, "gamemoderun %command%");
    assert!(suggestion.profiles[0].label.contains("GameMode"));

    let (enter, leave) = svc.demo_focus_cycle().unwrap();
    assert_eq!(enter.action, FocusAction::Enter);
    // firefox + waypaper ; pas discord / steam / pipewire
    assert!(enter.suspended >= 2);
    assert_eq!(leave.action, FocusAction::Leave);
    assert!(!svc.is_focus_active());
}

#[test]
fn bronze_and_silver_profiles_from_fixtures() {
    let bronze_raw = std::fs::read_to_string(fixtures_root().join("protondb_bronze.json")).unwrap();
    let silver_raw = std::fs::read_to_string(fixtures_root().join("protondb_silver.json")).unwrap();
    let bronze = parse_proton_summary_json(1, &bronze_raw).unwrap();
    let silver = parse_proton_summary_json(2, &silver_raw).unwrap();
    assert_eq!(bronze.tier, "bronze");
    assert_eq!(silver.tier, "silver");
    let bp = suggest_launch_profiles(Some(&bronze), false);
    let sp = suggest_launch_profiles(Some(&silver), false);
    assert_eq!(bp[0], sp[0]);
    assert_eq!(bp[0].command, "gamemoderun %command%");
}

#[test]
fn empty_libraries_scan_ok() {
    let dir = tempdir().unwrap();
    let steam = SteamLibrary::with_roots(vec![dir.path().join("nope")]);
    let heroic = HeroicLibrary::with_roots(vec![dir.path().join("nope2")]);
    assert!(steam.scan().unwrap().is_empty());
    assert!(heroic.scan().unwrap().is_empty());
}
