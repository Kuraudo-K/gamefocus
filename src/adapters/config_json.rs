//! Persistance JSON de la configuration (~/.config/gamefocus/).

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::domain::AppConfig;
use crate::ports::ConfigStore;

pub struct JsonConfigStore {
    path: PathBuf,
}

impl JsonConfigStore {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            path: Self::default_path()?,
        })
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Result<PathBuf, String> {
        let dirs = ProjectDirs::from("fr", "glf", "gamefocus")
            .ok_or_else(|| "impossible de résoudre le dossier config".to_string())?;
        Ok(dirs.config_dir().join("config.json"))
    }

    pub fn config_dir() -> Result<PathBuf, String> {
        let dirs = ProjectDirs::from("fr", "glf", "gamefocus")
            .ok_or_else(|| "impossible de résoudre le dossier config".to_string())?;
        Ok(dirs.config_dir().to_path_buf())
    }

    pub fn load_from(path: &Path) -> Result<AppConfig, String> {
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("lecture config {}: {e}", path.display()))?;
        serde_json::from_str(&raw).map_err(|e| format!("JSON config invalide: {e}"))
    }

    pub fn save_to(path: &Path, cfg: &AppConfig) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let raw = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
        fs::write(path, raw).map_err(|e| format!("écriture {}: {e}", path.display()))
    }
}

impl Default for JsonConfigStore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            path: PathBuf::from("gamefocus-config.json"),
        })
    }
}

impl ConfigStore for JsonConfigStore {
    fn load(&self) -> Result<AppConfig, String> {
        if !self.path.exists() {
            let cfg = AppConfig::default();
            self.save(&cfg)?;
            return Ok(cfg);
        }
        Self::load_from(&self.path)
    }

    fn save(&self, config: &AppConfig) -> Result<(), String> {
        Self::save_to(&self.path, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = JsonConfigStore::with_path(path);
        let mut cfg = AppConfig::default();
        cfg.last_search = "cyberpunk".into();
        cfg.focus.keep.push("obs".into());
        cfg.launch_options_enabled.push("Steam:1091500".into());
        store.save(&cfg).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.last_search, "cyberpunk");
        assert!(loaded.focus.keep.iter().any(|k| k == "obs"));
        assert_eq!(loaded.launch_options_enabled, ["Steam:1091500"]);
    }
}
