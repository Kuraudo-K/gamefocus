//! Détection des Proton installés (Steam compatibilitytools + Heroic tools).

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtonTool {
    /// Nom affiché / utilisé par Steam CompatToolMapping & Faugus `runner`.
    pub name: String,
    /// Dossier du tool.
    pub dir: PathBuf,
    /// Binaire `proton` (pour Heroic wineVersion.bin).
    pub proton_bin: PathBuf,
}

/// Liste les Proton disponibles (dédupliqués par nom).
pub fn list_proton_tools() -> Vec<ProtonTool> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let home = match std::env::var_os("HOME").map(PathBuf::from) {
        Some(h) => h,
        None => return out,
    };
    let roots = [
        home.join(".local/share/Steam/compatibilitytools.d"),
        home.join(".steam/steam/compatibilitytools.d"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/compatibilitytools.d"),
        home.join(".config/heroic/tools/proton"),
        home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton"),
        home.join(".local/share/faugus-launcher/components"),
    ];
    for root in &roots {
        scan_root(root, &mut out, &mut seen);
    }
    // Proton Valve stock dans steamapps/common
    for steam in [
        home.join(".local/share/Steam/steamapps/common"),
        home.join(".steam/steam/steamapps/common"),
    ] {
        scan_steam_common(&steam, &mut out, &mut seen);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn scan_root(root: &Path, out: &mut Vec<ProtonTool>, seen: &mut std::collections::HashSet<String>) {
    let Ok(rd) = fs::read_dir(root) else {
        return;
    };
    for entry in rd.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("__") || name == "tmp" {
            continue;
        }
        let proton_bin = dir.join("proton");
        if !proton_bin.is_file() {
            // UMU layout parfois
            continue;
        }
        if seen.insert(name.clone()) {
            out.push(ProtonTool {
                name,
                dir,
                proton_bin,
            });
        }
    }
}

fn scan_steam_common(
    common: &Path,
    out: &mut Vec<ProtonTool>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Ok(rd) = fs::read_dir(common) else {
        return;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.to_lowercase().contains("proton") {
            continue;
        }
        let dir = entry.path();
        let proton_bin = dir.join("proton");
        if proton_bin.is_file() && seen.insert(name.clone()) {
            out.push(ProtonTool {
                name,
                dir,
                proton_bin,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discovers_proton_folder() {
        let dir = tempdir().unwrap();
        let tool = dir.path().join("Proton-Test");
        fs::create_dir_all(&tool).unwrap();
        fs::write(tool.join("proton"), "#!/bin/sh\n").unwrap();
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        scan_root(dir.path(), &mut out, &mut seen);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Proton-Test");
    }
}
