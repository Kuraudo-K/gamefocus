//! Ports : traits dont dépendent le domaine/application — jamais les adapters.

use crate::domain::{
    AppConfig, FocusRules, FocusSession, FocusSnapshot, GameEntry, LaunchProfile, ProtonSummary,
};

/// Bibliothèque de jeux (Steam, Heroic, …).
pub trait GameLibrary: Send + Sync {
    fn scan(&self) -> Result<Vec<GameEntry>, String>;
}

/// Consultation ProtonDB (ou cache / mock).
pub trait ProtonLookup: Send + Sync {
    fn lookup(&self, app_id: u32) -> Result<ProtonSummary, String>;
}

/// Contrôle du mode Focus (signaux process, services).
pub trait FocusController: Send {
    fn enter(&mut self) -> Result<FocusSession, String>;
    fn leave(&mut self) -> Result<FocusSession, String>;
    fn is_active(&self) -> bool;
    fn snapshot(&self) -> &FocusSnapshot;
    fn set_dry_run(&mut self, enabled: bool);
    fn set_rules(&mut self, rules: FocusRules);
}

/// Persistance de la configuration.
pub trait ConfigStore: Send + Sync {
    fn load(&self) -> Result<AppConfig, String>;
    fn save(&self, config: &AppConfig) -> Result<(), String>;
}

/// Résolution d’AppID Steam (natif, heuristique, Store) — tous launchers.
pub trait SteamAppIdLookup: Send + Sync {
    fn resolve(&self, game: &GameEntry) -> Option<u32>;
}

/// Résultat d'une suggestion d'options pour un jeu.
#[derive(Debug, Clone)]
pub struct LaunchSuggestion {
    pub game: GameEntry,
    pub proton: Option<ProtonSummary>,
    pub profiles: Vec<LaunchProfile>,
    pub recommended: String,
}
