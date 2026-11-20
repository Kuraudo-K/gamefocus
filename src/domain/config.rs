//! Configuration applicative (modèle pur).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::focus::FocusRules;
use super::game::{GridCoverSize, ListCoverSize, SortMode, ViewMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Thème système",
            Self::Dark => "Sombre",
            Self::Light => "Clair",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub focus: FocusRules,
    /// Chemins Steam supplémentaires (bibliothèques externes).
    pub extra_steam_roots: Vec<PathBuf>,
    /// Timeout HTTP ProtonDB (secondes).
    pub protondb_timeout_secs: u64,
    /// Dernier filtre de recherche UI.
    pub last_search: String,
    /// Affichage bibliothèque : liste ou grille avec images.
    #[serde(default)]
    pub view_mode: ViewMode,
    /// Tri : nom ou launcher.
    #[serde(default)]
    pub sort_mode: SortMode,
    /// Afficher l’aperçu des applis gelées avant Focus (désactivé = 1 clic).
    #[serde(default)]
    pub focus_preview: bool,
    /// Tweaks de lancement simples (HDR, Proton global…).
    #[serde(default)]
    pub launch: LaunchTweaks,
    /// Taille jaquettes grille (S / M / L).
    #[serde(default)]
    pub grid_cover_size: GridCoverSize,
    /// Taille miniatures liste (normale / plus grande).
    #[serde(default)]
    pub list_cover_size: ListCoverSize,
    /// Thème de l’interface.
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// Jeux pour lesquels les options de lancement ont été écrites.
    #[serde(default)]
    pub launch_options_enabled: Vec<String>,
    /// Tokens ajoutés par GameFocus par jeu, pour ne retirer que ceux-ci.
    #[serde(default)]
    pub launch_options_managed: BTreeMap<String, Vec<String>>,
}

/// Options utiles en français — volontairement peu nombreuses (≠ GOverlay).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchTweaks {
    /// Nom du Proton (dossier compatibilitytools). Vide = laisser le launcher décider.
    #[serde(default)]
    pub proton_tool: String,
    /// Appliquer ce Proton à chaque « Lancer » depuis GameFocus.
    #[serde(default = "default_true")]
    pub apply_proton_on_launch: bool,
    /// HDR via Proton / DXVK (écrans compatibles).
    #[serde(default)]
    pub hdr: bool,
    /// Journal Proton (~/steam-*.log) pour diagnostiquer.
    #[serde(default)]
    pub proton_log: bool,
    /// FSR 4 Proton/AMD (`FSR4_UPGRADE=1`) — pas le vieux FSR Wine.
    #[serde(default)]
    pub fsr: bool,
}

fn default_true() -> bool {
    true
}

impl Default for LaunchTweaks {
    fn default() -> Self {
        Self {
            proton_tool: String::new(),
            apply_proton_on_launch: true,
            hdr: false,
            proton_log: false,
            fsr: false,
        }
    }
}

impl LaunchTweaks {
    /// Préfixe d’env à coller devant `%command%` (sans wrappers mangohud…).
    pub fn env_prefix(&self) -> String {
        let mut parts = Vec::new();
        if self.hdr {
            parts.push("PROTON_ENABLE_HDR=1");
            parts.push("DXVK_HDR=1");
        }
        if self.proton_log {
            parts.push("PROTON_LOG=1");
        }
        if self.fsr {
            parts.push("FSR4_UPGRADE=1");
        }
        parts.join(" ")
    }

    /// Fusionne le préfixe tweaks dans une commande existante.
    /// N’ajoute que les variables absentes (évite les doublons HDR / FSR / log).
    pub fn merge_into_command(&self, command: &str) -> String {
        let cmd = command.trim();
        let mut missing = Vec::new();
        if self.hdr {
            if !cmd.contains("PROTON_ENABLE_HDR=1") {
                missing.push("PROTON_ENABLE_HDR=1");
            }
            if !cmd.contains("DXVK_HDR=1") {
                missing.push("DXVK_HDR=1");
            }
        }
        if self.proton_log && !cmd.contains("PROTON_LOG=1") {
            missing.push("PROTON_LOG=1");
        }
        if self.fsr && !cmd.contains("FSR4_UPGRADE=1") {
            missing.push("FSR4_UPGRADE=1");
        }
        if missing.is_empty() {
            return if cmd.is_empty() {
                String::new()
            } else {
                cmd.to_string()
            };
        }
        let prefix = missing.join(" ");
        if cmd.is_empty() {
            format!("{prefix} %command%")
        } else {
            format!("{prefix} {cmd}")
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            focus: FocusRules::default(),
            extra_steam_roots: vec![],
            protondb_timeout_secs: 8,
            last_search: String::new(),
            view_mode: ViewMode::List,
            sort_mode: SortMode::Name,
            focus_preview: false,
            launch: LaunchTweaks::default(),
            grid_cover_size: GridCoverSize::Medium,
            list_cover_size: ListCoverSize::Normal,
            theme_mode: ThemeMode::System,
            launch_options_enabled: vec![],
            launch_options_managed: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_sensible_timeout() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.protondb_timeout_secs, 8);
        assert!(cfg.extra_steam_roots.is_empty());
        assert_eq!(cfg.view_mode, ViewMode::List);
        assert!(!cfg.focus_preview);
        assert_eq!(cfg.theme_mode, ThemeMode::System);
        assert!(cfg.launch.proton_tool.is_empty());
        assert!(cfg.launch.apply_proton_on_launch);
    }

    #[test]
    fn serde_roundtrip_new_fields() {
        let raw = r#"{"focus":{"suspend":[],"keep":["discord"],"stop_user_services":[],"use_gamemode":true,"pause_wallpapers":true},"extra_steam_roots":[],"protondb_timeout_secs":8,"last_search":"","auto_focus_on_launch":true}"#;
        let cfg: AppConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.view_mode, ViewMode::List);
        assert!(!cfg.focus_preview);
        assert_eq!(cfg.theme_mode, ThemeMode::System);
        assert!(!cfg.launch.hdr);
    }

    #[test]
    fn theme_and_launch_focus_settings_roundtrip() {
        let cfg = AppConfig {
            theme_mode: ThemeMode::Light,
            launch_options_enabled: vec!["Steam:1091500".into()],
            launch_options_managed: BTreeMap::new(),
            ..AppConfig::default()
        };
        let decoded: AppConfig =
            serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(decoded.theme_mode, ThemeMode::Light);
        assert_eq!(decoded.launch_options_enabled, ["Steam:1091500"]);
        assert!(decoded.launch_options_managed.is_empty());
    }

    #[test]
    fn merge_hdr_into_command() {
        let t = LaunchTweaks {
            hdr: true,
            ..LaunchTweaks::default()
        };
        let c = t.merge_into_command("mangohud %command%");
        assert!(c.starts_with("PROTON_ENABLE_HDR=1"));
        assert!(c.contains("DXVK_HDR=1"));
        assert!(c.contains("mangohud"));
    }

    #[test]
    fn merge_does_not_duplicate_existing_env() {
        let t = LaunchTweaks {
            hdr: true,
            fsr: true,
            proton_log: true,
            ..LaunchTweaks::default()
        };
        let base = "PROTON_ENABLE_HDR=1 DXVK_HDR=1 mangohud %command%";
        let c = t.merge_into_command(base);
        assert_eq!(c.matches("PROTON_ENABLE_HDR=1").count(), 1);
        assert_eq!(c.matches("DXVK_HDR=1").count(), 1);
        assert!(c.contains("FSR4_UPGRADE=1"));
        assert!(c.contains("PROTON_LOG=1"));
        assert!(c.contains("mangohud"));
    }
}
