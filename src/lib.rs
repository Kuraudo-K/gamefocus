//! GameFocus — architecture hexagonale (domain / ports / application / adapters).
//!
//! Le domaine ne dépend pas d'eframe, ureq ni sysinfo.
//! Les adapters implémentent les ports ; `main` est le composition root.

pub mod adapters;
pub mod application;
pub mod domain;
pub mod ports;

pub use application::GameFocusService;
pub use domain::{
    AppConfig, FocusAction, FocusRules, FocusSession, FocusSnapshot, GameEntry, GameSource,
    LaunchProfile, ProtonSummary,
};
