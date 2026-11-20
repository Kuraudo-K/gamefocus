//! Cas d'usage : orchestre les ports. Aucun eframe / ureq / sysinfo.

use std::sync::{Arc, Mutex};

use crate::adapters::steam_appid::SteamAppIdResolver;
use crate::domain::{
    is_heavy_game, suggest_launch_command, suggest_launch_profiles_for_game, AppConfig, FocusRules,
    FocusSession, FocusSnapshot, GameEntry, LaunchProfile, ProtonSummary,
};
use crate::ports::{
    ConfigStore, FocusController, GameLibrary, LaunchSuggestion, ProtonLookup, SteamAppIdLookup,
};

/// Service applicatif principal GameFocus.
pub struct GameFocusService {
    libraries: Vec<Arc<dyn GameLibrary>>,
    proton: Arc<dyn ProtonLookup>,
    focus: Mutex<Box<dyn FocusController>>,
    config: Arc<dyn ConfigStore>,
    /// Résolution AppID Steam par nom (Heroic/Faugus/Lutris).
    steam_ids: Arc<dyn SteamAppIdLookup>,
}

impl GameFocusService {
    pub fn new(
        libraries: Vec<Arc<dyn GameLibrary>>,
        proton: Arc<dyn ProtonLookup>,
        focus: Box<dyn FocusController>,
        config: Arc<dyn ConfigStore>,
    ) -> Self {
        Self::with_steam_ids(
            libraries,
            proton,
            focus,
            config,
            Arc::new(SteamAppIdResolver::new(6)),
        )
    }

    pub fn with_steam_ids(
        libraries: Vec<Arc<dyn GameLibrary>>,
        proton: Arc<dyn ProtonLookup>,
        focus: Box<dyn FocusController>,
        config: Arc<dyn ConfigStore>,
        steam_ids: Arc<dyn SteamAppIdLookup>,
    ) -> Self {
        Self {
            libraries,
            proton,
            focus: Mutex::new(focus),
            config,
            steam_ids,
        }
    }

    pub fn steam_ids(&self) -> Arc<dyn SteamAppIdLookup> {
        Arc::clone(&self.steam_ids)
    }

    /// AppID Steam pour ProtonDB / liens — tous launchers.
    pub fn resolve_steam_app_id(&self, game: &GameEntry) -> Option<u32> {
        self.steam_ids.resolve(game)
    }

    pub fn load_config(&self) -> AppConfig {
        let mut cfg = self.config.load().unwrap_or_default();
        crate::domain::dedupe_focus_action_lists(&mut cfg.focus);
        cfg
    }

    pub fn save_config(&self, cfg: &AppConfig) -> Result<(), String> {
        let mut cleaned = cfg.clone();
        crate::domain::dedupe_focus_action_lists(&mut cleaned.focus);
        self.config.save(&cleaned)
    }

    /// Scanne toutes les bibliothèques branchées et déduplique.
    pub fn scan_games(&self) -> Result<Vec<GameEntry>, String> {
        let mut games = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for lib in &self.libraries {
            for g in lib.scan()? {
                let key = (
                    g.app_id.unwrap_or(0),
                    g.name.to_lowercase(),
                    format!("{:?}", g.source),
                );
                if seen.insert(key) {
                    games.push(g);
                }
            }
        }

        games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(games)
    }

    pub fn lookup_proton(&self, app_id: u32) -> Result<ProtonSummary, String> {
        self.proton.lookup(app_id)
    }

    pub fn suggest_for_game(
        &self,
        game: &GameEntry,
        fetch_proton: bool,
    ) -> Result<LaunchSuggestion, String> {
        // Échec ProtonDB non bloquant. AppID : Steam natif, heuristique, ou recherche Store
        // (Heroic GOG / Faugus / Lutris — pas seulement Cyberpunk).
        let proton = if fetch_proton {
            self.resolve_steam_app_id(game)
                .and_then(|id| self.proton.lookup(id).ok())
        } else {
            None
        };

        let heavy = is_heavy_game(&game.name);
        let profiles = suggest_launch_profiles_for_game(proton.as_ref(), &game.name);
        let recommended = suggest_launch_command(proton.as_ref(), heavy);

        Ok(LaunchSuggestion {
            game: game.clone(),
            proton,
            profiles,
            recommended,
        })
    }

    pub fn profiles_from_summary(
        summary: Option<&ProtonSummary>,
        game_name: &str,
    ) -> Vec<LaunchProfile> {
        suggest_launch_profiles_for_game(summary, game_name)
    }

    pub fn enter_focus(&self) -> Result<FocusSession, String> {
        let mut focus = self
            .focus
            .lock()
            .map_err(|_| "verrou focus empoisonné".to_string())?;
        focus.enter()
    }

    /// Met à jour les règles puis active le Focus (après aperçu UI).
    pub fn enter_focus_with_rules(&self, rules: FocusRules) -> Result<FocusSession, String> {
        let mut focus = self
            .focus
            .lock()
            .map_err(|_| "verrou focus empoisonné".to_string())?;
        focus.set_rules(rules);
        focus.enter()
    }

    pub fn leave_focus(&self) -> Result<FocusSession, String> {
        let mut focus = self
            .focus
            .lock()
            .map_err(|_| "verrou focus empoisonné".to_string())?;
        focus.leave()
    }

    pub fn is_focus_active(&self) -> bool {
        self.focus.lock().map(|f| f.is_active()).unwrap_or(false)
    }

    pub fn focus_snapshot(&self) -> FocusSnapshot {
        self.focus
            .lock()
            .map(|f| f.snapshot().clone())
            .unwrap_or_default()
    }

    pub fn set_focus_dry_run(&self, enabled: bool) {
        if let Ok(mut focus) = self.focus.lock() {
            focus.set_dry_run(enabled);
        }
    }

    /// Démo dry-run : entre puis quitte le focus sans signaux réels.
    pub fn demo_focus_cycle(&self) -> Result<(FocusSession, FocusSession), String> {
        self.set_focus_dry_run(true);
        let enter = self.enter_focus()?;
        let leave = self.leave_focus()?;
        Ok((enter, leave))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        select_focus_targets, FocusAction, FocusRules, FocusSnapshot, GameSource, ProtonSummary,
    };
    use crate::ports::LaunchSuggestion;

    struct MemConfig {
        cfg: std::sync::Mutex<AppConfig>,
    }

    impl ConfigStore for MemConfig {
        fn load(&self) -> Result<AppConfig, String> {
            Ok(self.cfg.lock().unwrap().clone())
        }
        fn save(&self, config: &AppConfig) -> Result<(), String> {
            *self.cfg.lock().unwrap() = config.clone();
            Ok(())
        }
    }

    struct FakeLib {
        games: Vec<GameEntry>,
    }

    impl GameLibrary for FakeLib {
        fn scan(&self) -> Result<Vec<GameEntry>, String> {
            Ok(self.games.clone())
        }
    }

    struct FakeProton {
        summary: Option<ProtonSummary>,
    }

    impl ProtonLookup for FakeProton {
        fn lookup(&self, app_id: u32) -> Result<ProtonSummary, String> {
            self.summary
                .clone()
                .filter(|s| s.app_id == app_id)
                .ok_or_else(|| "introuvable".into())
        }
    }

    struct DryFocus {
        snap: FocusSnapshot,
        dry: bool,
        rules: FocusRules,
        fake_procs: Vec<(u32, String)>,
    }

    impl FocusController for DryFocus {
        fn enter(&mut self) -> Result<FocusSession, String> {
            if self.snap.active {
                return Ok(FocusSession {
                    action: FocusAction::Enter,
                    suspended: self.snap.suspended_pids.len(),
                    services_stopped: 0,
                    wallpapers_paused: 0,
                    message: "déjà actif".into(),
                });
            }
            let refs: Vec<(u32, &str)> = self
                .fake_procs
                .iter()
                .map(|(p, n)| (*p, n.as_str()))
                .collect();
            let pids = select_focus_targets(&refs, &self.rules);
            self.snap = FocusSnapshot {
                suspended_pids: pids.clone(),
                stopped_services: self.rules.stop_user_services.clone(),
                paused_wallpapers: vec![],
                paused_wallpaper_processes: vec![],
                wallpaper_restart_commands: vec![],
                focus_started_at_unix_secs: None,
                vram_stopped_pids: vec![],
                ram_stopped_pids: vec![],
                active: true,
                dropped_page_cache: false,
            };
            Ok(FocusSession {
                action: FocusAction::Enter,
                suspended: pids.len(),
                services_stopped: self.snap.stopped_services.len(),
                wallpapers_paused: 0,
                message: format!("dry={} — {} gelés", self.dry, pids.len()),
            })
        }

        fn leave(&mut self) -> Result<FocusSession, String> {
            let n = self.snap.suspended_pids.len();
            self.snap = FocusSnapshot::default();
            Ok(FocusSession {
                action: FocusAction::Leave,
                suspended: n,
                services_stopped: 0,
                wallpapers_paused: 0,
                message: format!("{n} repris"),
            })
        }

        fn is_active(&self) -> bool {
            self.snap.active
        }

        fn snapshot(&self) -> &FocusSnapshot {
            &self.snap
        }

        fn set_dry_run(&mut self, enabled: bool) {
            self.dry = enabled;
        }

        fn set_rules(&mut self, rules: FocusRules) {
            self.rules = rules;
        }
    }

    fn sample_service() -> GameFocusService {
        let games = vec![
            GameEntry {
                app_id: Some(1091500),
                name: "Cyberpunk 2077".into(),
                source: GameSource::Steam,
                install_path: None,
                launch_options: None,
                cover_path: None,
                launcher_id: None,
            },
            GameEntry {
                app_id: Some(570),
                name: "Dota 2".into(),
                source: GameSource::Steam,
                install_path: None,
                launch_options: None,
                cover_path: None,
                launcher_id: None,
            },
        ];
        let proton = ProtonSummary {
            app_id: 1091500,
            tier: "gold".into(),
            total: 5000,
            trending_tier: Some("platinum".into()),
            best_reported_tier: None,
            confidence: Some("strong".into()),
            score: Some(0.8),
        };
        GameFocusService::new(
            vec![Arc::new(FakeLib { games })],
            Arc::new(FakeProton {
                summary: Some(proton),
            }),
            Box::new(DryFocus {
                snap: FocusSnapshot::default(),
                dry: true,
                rules: FocusRules::default(),
                fake_procs: vec![
                    (10, "discord".into()),
                    (20, "firefox".into()),
                    (30, "waypaper".into()),
                ],
            }),
            Arc::new(MemConfig {
                cfg: std::sync::Mutex::new(AppConfig::default()),
            }),
        )
    }

    #[test]
    fn scan_and_suggest() {
        let svc = sample_service();
        let games = svc.scan_games().unwrap();
        assert_eq!(games.len(), 2);
        let cp = games.iter().find(|g| g.name.contains("Cyberpunk")).unwrap();
        let LaunchSuggestion {
            profiles,
            recommended,
            proton,
            ..
        } = svc.suggest_for_game(cp, true).unwrap();
        assert!(proton.as_ref().unwrap().tier == "gold");
        assert!(profiles[0].label.contains("GameMode"));
        assert_eq!(recommended, "gamemoderun %command%");
        assert!(profiles[0].command.contains("gamemoderun"));
    }

    #[test]
    fn dry_run_focus_cycle() {
        let svc = sample_service();
        let (enter, leave) = svc.demo_focus_cycle().unwrap();
        assert_eq!(enter.action, FocusAction::Enter);
        assert!(enter.suspended >= 1); // firefox + waypaper
        assert!(!enter.message.to_lowercase().contains("discord"));
        assert_eq!(leave.action, FocusAction::Leave);
        assert!(!svc.is_focus_active());
    }

    #[test]
    fn config_roundtrip_via_port() {
        let svc = sample_service();
        let mut cfg = svc.load_config();
        cfg.last_search = "cyber".into();
        svc.save_config(&cfg).unwrap();
        assert_eq!(svc.load_config().last_search, "cyber");
    }

    #[test]
    fn suggest_without_fetching_proton() {
        let svc = sample_service();
        let games = svc.scan_games().unwrap();
        let cp = &games[0];
        let s = svc.suggest_for_game(cp, false).unwrap();
        assert!(s.proton.is_none());
        assert!(!s.profiles.is_empty());
    }

    #[test]
    fn scan_deduplicates_identical_entries() {
        let games = vec![
            GameEntry {
                app_id: Some(570),
                name: "Dota 2".into(),
                source: GameSource::Steam,
                install_path: None,
                launch_options: None,
                cover_path: None,
                launcher_id: None,
            },
            GameEntry {
                app_id: Some(570),
                name: "Dota 2".into(),
                source: GameSource::Steam,
                install_path: None,
                launch_options: None,
                cover_path: None,
                launcher_id: None,
            },
        ];
        let svc = GameFocusService::new(
            vec![Arc::new(FakeLib { games })],
            Arc::new(FakeProton { summary: None }),
            Box::new(DryFocus {
                snap: FocusSnapshot::default(),
                dry: true,
                rules: FocusRules::default(),
                fake_procs: vec![],
            }),
            Arc::new(MemConfig {
                cfg: std::sync::Mutex::new(AppConfig::default()),
            }),
        );
        assert_eq!(svc.scan_games().unwrap().len(), 1);
    }

    #[test]
    fn profiles_from_summary_helper() {
        let s = ProtonSummary {
            app_id: 1,
            tier: "platinum".into(),
            total: 9,
            trending_tier: None,
            best_reported_tier: None,
            confidence: None,
            score: None,
        };
        let profiles = GameFocusService::profiles_from_summary(Some(&s), "Celeste");
        assert!(profiles[0].label.contains("GameMode"));
    }
}
