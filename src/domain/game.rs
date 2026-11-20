//! Entrées de bibliothèque (Steam / Heroic / Lutris / Faugus / manuel).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameEntry {
    pub app_id: Option<u32>,
    pub name: String,
    pub source: GameSource,
    pub install_path: Option<PathBuf>,
    /// Options de lancement Steam actuelles (si trouvées).
    pub launch_options: Option<String>,
    /// Cover / icône locale si connue (Faugus icon, etc.).
    pub cover_path: Option<PathBuf>,
    /// Id launcher (Heroic appName, Faugus gameid, Lutris slug).
    pub launcher_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameSource {
    Steam,
    Heroic,
    Lutris,
    Faugus,
    Manual,
}

impl GameSource {
    pub fn label(self) -> &'static str {
        match self {
            GameSource::Steam => "Steam",
            GameSource::Heroic => "Heroic",
            GameSource::Lutris => "Lutris",
            GameSource::Faugus => "Faugus",
            GameSource::Manual => "Manuel",
        }
    }

    pub fn badge_rgb(self) -> (u8, u8, u8) {
        match self {
            GameSource::Steam => (102, 192, 244),
            GameSource::Heroic => (57, 181, 74),
            GameSource::Lutris => (255, 158, 61),
            GameSource::Faugus => (180, 120, 255),
            GameSource::Manual => (160, 160, 160),
        }
    }

    pub fn sort_key(self) -> u8 {
        match self {
            GameSource::Steam => 0,
            GameSource::Heroic => 1,
            GameSource::Faugus => 2,
            GameSource::Lutris => 3,
            GameSource::Manual => 4,
        }
    }
}

impl std::fmt::Display for GameSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
    #[default]
    List,
    Grid,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::List => "Liste",
            ViewMode::Grid => "Grille",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    #[default]
    Name,
    Launcher,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Name => "Nom",
            SortMode::Launcher => "Launcher",
        }
    }
}

/// Taille des jaquettes en mode grille (4 niveaux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridCoverSize {
    Small,
    #[default]
    Medium,
    Large,
    XLarge,
}

impl GridCoverSize {
    pub fn label(self) -> &'static str {
        match self {
            GridCoverSize::Small => "Petite",
            GridCoverSize::Medium => "Moyenne",
            GridCoverSize::Large => "Grande",
            GridCoverSize::XLarge => "Très grande",
        }
    }

    /// Largeur carte (hauteur ≈ ×1.5).
    pub fn card_width(self) -> f32 {
        match self {
            GridCoverSize::Small => 96.0,
            GridCoverSize::Medium => 118.0,
            GridCoverSize::Large => 148.0,
            GridCoverSize::XLarge => 224.0,
        }
    }
}

/// Taille des miniatures en mode liste (3 niveaux).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ListCoverSize {
    #[default]
    Normal,
    Large,
    XLarge,
}

impl ListCoverSize {
    pub fn label(self) -> &'static str {
        match self {
            ListCoverSize::Normal => "Normale",
            ListCoverSize::Large => "Plus grande",
            ListCoverSize::XLarge => "Très grande",
        }
    }

    pub fn thumb_size(self) -> (f32, f32) {
        match self {
            ListCoverSize::Normal => (40.0, 54.0),
            ListCoverSize::Large => (56.0, 76.0),
            ListCoverSize::XLarge => (84.0, 114.0),
        }
    }
}

/// Processus en cours (pour réglages / aperçu Focus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningApp {
    pub name: String,
    pub exe: String,
    pub pid_count: usize,
}

/// AppID Steam utilisable pour ProtonDB / jaquettes CDN.
/// Ignore les ids GOG Heroic trop grands ; fallback par nom si besoin.
pub fn steamish_app_id(game: &GameEntry) -> Option<u32> {
    if let Some(id) = game.app_id {
        if id > 0 && id < 50_000_000 {
            return Some(id);
        }
    }
    let n = game.name.to_lowercase();
    if n.contains("cyberpunk") {
        Some(1_091_500)
    } else if n.contains("witcher 3") {
        Some(292_030)
    } else if n.contains("helldivers") {
        Some(553_850)
    } else if n.contains("palworld") {
        Some(1_623_730)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_large_covers_are_larger_than_large_covers() {
        assert_eq!(GridCoverSize::XLarge.card_width(), 224.0);
        assert!(GridCoverSize::XLarge.card_width() > GridCoverSize::Large.card_width());
        assert_eq!(ListCoverSize::XLarge.thumb_size(), (84.0, 114.0));
        assert!(ListCoverSize::XLarge.thumb_size().0 > ListCoverSize::Large.thumb_size().0);
    }

    #[test]
    fn cyberpunk_gog_maps_to_steam_appid() {
        let g = GameEntry {
            app_id: Some(1_423_049_311),
            name: "Cyberpunk 2077".into(),
            source: GameSource::Heroic,
            install_path: None,
            launch_options: None,
            cover_path: None,
            launcher_id: Some("1423049311".into()),
        };
        assert_eq!(steamish_app_id(&g), Some(1_091_500));
    }
}
