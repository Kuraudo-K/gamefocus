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

/// Bureau détecté pour choisir la recette HDR (GNOME ≠ KDE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopKind {
    Gnome,
    Kde,
    Other,
}

/// Lit `XDG_CURRENT_DESKTOP` / `DESKTOP_SESSION` (ex. `KDE`, `GNOME`, `ubuntu:GNOME`).
pub fn detect_desktop() -> DesktopKind {
    detect_desktop_from(
        std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref(),
        std::env::var("DESKTOP_SESSION").ok().as_deref(),
    )
}

pub fn detect_desktop_from(
    xdg_current_desktop: Option<&str>,
    desktop_session: Option<&str>,
) -> DesktopKind {
    let blob = format!(
        "{}:{}",
        xdg_current_desktop.unwrap_or(""),
        desktop_session.unwrap_or("")
    )
    .to_lowercase();
    if blob.split(&[':', ';', ','][..]).any(|part| {
        let p = part.trim();
        p == "gnome" || p.starts_with("gnome-") || p.ends_with("-gnome")
    }) {
        return DesktopKind::Gnome;
    }
    if blob.split(&[':', ';', ','][..]).any(|part| {
        let p = part.trim();
        p == "kde" || p == "plasma" || p.starts_with("plasma") || p.contains("plasma")
    }) || blob.contains("kde")
    {
        return DesktopKind::Kde;
    }
    DesktopKind::Other
}

/// Tokens HDR selon le DE (sans HDR bureau).
/// - GNOME : recette d’origine qui marchait (`PROTON_ENABLE_HDR` + `DXVK_HDR`)
/// - KDE / autre : gamescope (chemin validé sur Plasma)
pub fn hdr_launch_tokens(desktop: DesktopKind) -> &'static [&'static str] {
    match desktop {
        DesktopKind::Gnome => &["PROTON_ENABLE_HDR=1", "DXVK_HDR=1"],
        DesktopKind::Kde | DesktopKind::Other => &["DXVK_HDR=1", "gamescope -f --hdr-enabled --"],
    }
}

fn strip_managed_hdr_tokens(command: &str) -> String {
    let mut out = Vec::new();
    let mut tokens = command.split_whitespace().peekable();
    while let Some(token) = tokens.next() {
        match token {
            "DXVK_HDR=1" | "PROTON_ENABLE_HDR=1" | "PROTON_ENABLE_WAYLAND=1" => {}
            "gamescope" => {
                // Retire `gamescope -f --hdr-enabled --` (recette KDE gérée).
                let mut look = Vec::new();
                while look.len() < 3 {
                    if let Some(next) = tokens.peek().copied() {
                        look.push(next);
                        tokens.next();
                    } else {
                        break;
                    }
                }
                if look.as_slice() == ["-f", "--hdr-enabled", "--"] {
                    continue;
                }
                out.push(token.to_string());
                out.extend(look.into_iter().map(str::to_string));
            }
            other => out.push(other.to_string()),
        }
    }
    out.join(" ")
}

impl LaunchTweaks {
    /// Préfixe d’env à coller devant `%command%` (sans wrappers mangohud…).
    pub fn env_prefix(&self) -> String {
        self.env_prefix_for(detect_desktop())
    }

    pub fn env_prefix_for(&self, desktop: DesktopKind) -> String {
        let mut parts = Vec::new();
        if self.hdr {
            parts.extend(hdr_launch_tokens(desktop).iter().copied());
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
    pub fn merge_into_command(&self, command: &str) -> String {
        self.merge_into_command_for(command, detect_desktop())
    }

    /// Comme `merge_into_command`, avec un DE explicite (tests / UI).
    pub fn merge_into_command_for(&self, command: &str, desktop: DesktopKind) -> String {
        let mut cmd = command.trim().to_string();
        if self.hdr {
            cmd = strip_managed_hdr_tokens(&cmd);
        }
        let mut missing = Vec::new();
        if self.hdr {
            for token in hdr_launch_tokens(desktop) {
                if *token == "gamescope -f --hdr-enabled --" {
                    if !cmd.contains("--hdr-enabled") {
                        missing.push(*token);
                    }
                } else if !cmd.split_whitespace().any(|t| t == *token) {
                    missing.push(*token);
                }
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
                cmd
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
    fn detect_desktop_from_xdg() {
        assert_eq!(
            detect_desktop_from(Some("ubuntu:GNOME"), None),
            DesktopKind::Gnome
        );
        assert_eq!(
            detect_desktop_from(Some("KDE"), Some("plasma")),
            DesktopKind::Kde
        );
        assert_eq!(detect_desktop_from(Some("Hyprland"), None), DesktopKind::Other);
    }

    #[test]
    fn merge_hdr_kde_uses_gamescope() {
        let t = LaunchTweaks {
            hdr: true,
            ..LaunchTweaks::default()
        };
        let c = t.merge_into_command_for("mangohud %command%", DesktopKind::Kde);
        assert!(c.contains("DXVK_HDR=1"));
        assert!(c.contains("gamescope -f --hdr-enabled --"));
        assert!(c.contains("mangohud"));
        assert!(!c.contains("PROTON_ENABLE_WAYLAND"));
    }

    #[test]
    fn merge_hdr_gnome_uses_original_proton_vars() {
        let t = LaunchTweaks {
            hdr: true,
            ..LaunchTweaks::default()
        };
        let c = t.merge_into_command_for("mangohud %command%", DesktopKind::Gnome);
        assert!(c.contains("PROTON_ENABLE_HDR=1"));
        assert!(c.contains("DXVK_HDR=1"));
        assert!(!c.contains("PROTON_ENABLE_WAYLAND"));
        assert!(!c.contains("gamescope"));
        assert!(c.contains("mangohud"));
    }

    #[test]
    fn merge_hdr_switches_recipe_when_desktop_changes() {
        let t = LaunchTweaks {
            hdr: true,
            ..LaunchTweaks::default()
        };
        let kde = t.merge_into_command_for("%command%", DesktopKind::Kde);
        let gnome = t.merge_into_command_for(&kde, DesktopKind::Gnome);
        assert!(!gnome.contains("gamescope"));
        assert!(gnome.contains("PROTON_ENABLE_HDR=1"));
        assert!(!gnome.contains("PROTON_ENABLE_WAYLAND"));
        assert_eq!(gnome.matches("DXVK_HDR=1").count(), 1);
    }

    #[test]
    fn merge_does_not_duplicate_existing_env() {
        let t = LaunchTweaks {
            hdr: true,
            fsr: true,
            proton_log: true,
            ..LaunchTweaks::default()
        };
        let base = "DXVK_HDR=1 gamescope -f --hdr-enabled -- mangohud %command%";
        let c = t.merge_into_command_for(base, DesktopKind::Kde);
        assert_eq!(c.matches("DXVK_HDR=1").count(), 1);
        assert_eq!(c.matches("--hdr-enabled").count(), 1);
        assert!(c.contains("FSR4_UPGRADE=1"));
        assert!(c.contains("PROTON_LOG=1"));
        assert!(c.contains("mangohud"));
    }
}
