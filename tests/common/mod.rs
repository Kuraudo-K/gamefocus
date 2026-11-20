//! Helpers partagés pour les tests d'intégration (tmpdir / fixtures).

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("mkdir dst");
    for entry in fs::read_dir(src).expect("read_dir") {
        let entry = entry.expect("entry");
        let ty = entry.file_type().expect("ft");
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).expect("copy");
        }
    }
}

/// Arborescence type « machine de jeu » sous un tmpdir (sans muter HOME process).
pub struct FakeGameMachine {
    pub tmp: TempDir,
    pub steam_root: PathBuf,
    pub heroic_root: PathBuf,
    pub config_path: PathBuf,
}

impl FakeGameMachine {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let steam_root = tmp.path().join("Steam");
        let heroic_root = tmp.path().join("heroic");
        let config_path = tmp.path().join("config/gamefocus/config.json");

        copy_dir(&fixtures_root().join("steam"), &steam_root);
        copy_dir(&fixtures_root().join("heroic"), &heroic_root);
        fs::create_dir_all(config_path.parent().unwrap()).expect("config dir");

        let vdf = format!(
            r#""libraryfolders"
{{
	"0"
	{{
		"path"		"{}"
		"label"		""
	}}
}}
"#,
            steam_root.display()
        );
        fs::write(steam_root.join("steamapps/libraryfolders.vdf"), vdf).expect("vdf");

        Self {
            tmp,
            steam_root,
            heroic_root,
            config_path,
        }
    }
}
