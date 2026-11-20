//! Domaine : modèles et règles purs (aucune I/O, aucun eframe/ureq/sysinfo).

pub mod config;
pub mod focus;
pub mod game;
pub mod launch;
pub mod proton;

pub use config::{
    detect_desktop, detect_desktop_from, hdr_launch_tokens, AppConfig, DesktopKind, LaunchTweaks,
    ThemeMode,
};
pub use focus::{
    aggressive_extra_targets, is_critical_process, process_name_matches, select_focus_targets,
    wallpaper_process_names, FocusAction, FocusRules, FocusSession, FocusSnapshot,
    PausedProcessIdentity, WallpaperRestartCommand,
};
pub use game::{
    steamish_app_id, GameEntry, GameSource, GridCoverSize, ListCoverSize, RunningApp, SortMode,
    ViewMode,
};
pub use launch::{
    is_heavy_game, suggest_launch_command, suggest_launch_profiles,
    suggest_launch_profiles_for_game, LaunchProfile,
};
pub use proton::{tier_kind, tier_label_fr, tier_rgb, ProtonSummary, ProtonTierKind};
