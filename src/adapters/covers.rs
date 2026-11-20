//! Résolution / cache des jaquettes (Steam local hash/jpg → Faugus → CDN).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

use crate::adapters::steam_appid::SteamAppIdResolver;
use crate::domain::GameEntry;
use crate::ports::SteamAppIdLookup;

pub struct CoverCache {
    textures: HashMap<String, TextureHandle>,
    tried_cdn: HashMap<String, Instant>,
    cache_dir: PathBuf,
    fetch_cdn: bool,
    steam_ids: Arc<dyn SteamAppIdLookup>,
}

impl CoverCache {
    pub fn new() -> Self {
        Self::with_resolver(Arc::new(SteamAppIdResolver::new(6)))
    }

    pub fn with_resolver(steam_ids: Arc<dyn SteamAppIdLookup>) -> Self {
        let cache_dir = directories::ProjectDirs::from("fr", "glf", "gamefocus")
            .map(|d| d.cache_dir().join("covers"))
            .unwrap_or_else(|| PathBuf::from("/tmp/gamefocus-covers"));
        let _ = fs::create_dir_all(&cache_dir);
        Self {
            textures: HashMap::new(),
            tried_cdn: HashMap::new(),
            cache_dir,
            fetch_cdn: true,
            steam_ids,
        }
    }

    pub fn texture_for(&mut self, ctx: &Context, game: &GameEntry) -> Option<TextureHandle> {
        let key = cover_key(game);
        if let Some(tex) = self.textures.get(&key) {
            return Some(tex.clone());
        }
        if let Some(path) = resolve_cover_path(game, &self.cache_dir) {
            if let Some(tex) = load_texture(ctx, &key, &path) {
                self.textures.insert(key.clone(), tex.clone());
                return Some(tex);
            }
        }
        // Faugus / exe : extraire l’icône du .exe (icoextract) une fois
        if let Some(path) = try_extract_exe_icon(game, &self.cache_dir) {
            if let Some(tex) = load_texture(ctx, &key, &path) {
                self.textures.insert(key.clone(), tex.clone());
                return Some(tex);
            }
        }
        // CDN Steam : AppID natif, heuristique, ou recherche Store (Heroic/Faugus/Lutris)
        if self.fetch_cdn {
            if let Some(app_id) = self.steam_ids.resolve(game) {
                let now = Instant::now();
                let allow = self
                    .tried_cdn
                    .get(&key)
                    .map(|t| now.duration_since(*t) > Duration::from_secs(20))
                    .unwrap_or(true);
                if allow {
                    self.tried_cdn.insert(key.clone(), now);
                    if let Some(path) = try_download_steam_cover(app_id, &self.cache_dir) {
                        if let Some(tex) = load_texture(ctx, &key, &path) {
                            self.textures.insert(key.clone(), tex.clone());
                            return Some(tex);
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for CoverCache {
    fn default() -> Self {
        Self::new()
    }
}

fn cover_key(game: &GameEntry) -> String {
    format!(
        "{}:{}:{}",
        game.source.label(),
        game.app_id.unwrap_or(0),
        game.name.to_lowercase()
    )
}

use crate::domain::steamish_app_id;

pub fn resolve_cover_path(game: &GameEntry, cache_dir: &Path) -> Option<PathBuf> {
    if let Some(ref p) = game.cover_path {
        if p.exists() {
            return Some(p.clone());
        }
    }
    if let Some(app_id) = steamish_app_id(game) {
        if let Some(p) = steam_library_cover(app_id) {
            return Some(p);
        }
        let cached = cache_dir.join(format!("{app_id}.jpg"));
        if cached.exists() {
            return Some(cached);
        }
    }
    None
}

fn steam_library_cover(app_id: u32) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dirs = [
        home.join(format!(".local/share/Steam/appcache/librarycache/{app_id}")),
        home.join(format!(".steam/steam/appcache/librarycache/{app_id}")),
        home.join(format!(
            ".var/app/com.valvesoftware.Steam/.local/share/Steam/appcache/librarycache/{app_id}"
        )),
    ];
    for dir in &dirs {
        if !dir.is_dir() {
            continue;
        }
        // Noms classiques (racine)
        for name in [
            "library_600x900.jpg",
            "library_capsule.jpg",
            "portrait.png",
            "header.jpg",
            "logo.png",
        ] {
            let p = dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
        // Steam récent : hash dirs (…/appid/<hash>/library_capsule.jpg)
        if let Some(p) = best_image_in_tree(dir) {
            return Some(p);
        }
    }
    // Ancien layout plat
    for name in [
        format!("{app_id}_library_600x900.jpg"),
        format!("{app_id}_header.jpg"),
    ] {
        let p = home
            .join(".local/share/Steam/appcache/librarycache")
            .join(&name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn best_image_in_tree(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let Ok(rd) = fs::read_dir(&cur) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let preferred = name.contains("600x900")
                || name.contains("portrait")
                || name.contains("library_capsule")
                || name.contains("capsule");
            // Hash blobs sans nom utile : ignorer les tout petits fichiers
            if size < 8_000 && !preferred {
                continue;
            }
            if size < 64 {
                continue;
            }
            let looks_img = name.ends_with(".jpg")
                || name.ends_with(".jpeg")
                || name.ends_with(".png")
                || name.ends_with(".ico")
                || !name.contains('.'); // hash Steam
            if !looks_img {
                continue;
            }
            // Éviter logos / hero blur (trop larges ou inutiles en grille)
            if name.contains("logo") || name.contains("blur") || name.contains("hero") {
                continue;
            }
            let score = size
                + if name.contains("600x900") || name.contains("portrait") {
                    8_000_000
                } else if name.contains("library_capsule") || name.contains("capsule") {
                    6_000_000
                } else if name.contains("header") {
                    1_000_000
                } else {
                    0
                };
            if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) && is_loadable_image(&path) {
                best = Some((score, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn try_extract_exe_icon(game: &GameEntry, cache_dir: &Path) -> Option<PathBuf> {
    let exe = game.install_path.as_ref()?;
    let is_exe = exe
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("exe"))
        .unwrap_or(false);
    if !is_exe || !exe.is_file() {
        return None;
    }
    let dest = cache_dir.join(format!(
        "exeicon_{}.png",
        game.launcher_id
            .as_deref()
            .unwrap_or(&game.name)
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
    ));
    if dest.exists() {
        return Some(dest);
    }
    let _ = fs::create_dir_all(cache_dir);
    // icoextract écrit souvent un vrai .ico même si l’extension est .png
    let dest_ico = dest.with_extension("ico");
    let mut bins = vec![
        "icoextract".into(),
        "/run/current-system/sw/bin/icoextract".into(),
    ];
    if let Ok(out) = std::process::Command::new("sh")
        .args(["-c", "command -v icoextract || true"])
        .output()
    {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !p.is_empty() {
            bins.insert(0, p);
        }
    }
    for bin in &bins {
        for out in [&dest, &dest_ico] {
            let status = std::process::Command::new(bin)
                .args([exe.to_str()?, out.to_str()?])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if status.map(|s| s.success()).unwrap_or(false) && out.exists() {
                if let (Some(home), Some(gid)) =
                    (std::env::var_os("HOME"), game.launcher_id.as_deref())
                {
                    let faugus_icon = PathBuf::from(home)
                        .join(".local/share/faugus-launcher/icons")
                        .join(format!("{gid}.png"));
                    let _ = fs::create_dir_all(faugus_icon.parent().unwrap_or(Path::new(".")));
                    let _ = fs::copy(out, &faugus_icon);
                }
                return Some(out.clone());
            }
        }
    }
    None
}

fn is_loadable_image(path: &Path) -> bool {
    let Ok(reader) = image::ImageReader::open(path) else {
        return false;
    };
    reader
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
        .is_some()
}

fn try_download_steam_cover(app_id: u32, cache_dir: &Path) -> Option<PathBuf> {
    let dest = cache_dir.join(format!("{app_id}.jpg"));
    if dest.exists() {
        return Some(dest);
    }
    let urls = [
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_capsule.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"),
    ];
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(4))
        .user_agent("gamefocus/0.1")
        .build();
    for url in urls {
        let Ok(resp) = agent.get(&url).call() else {
            continue;
        };
        if resp.status() != 200 {
            continue;
        }
        let mut bytes = Vec::new();
        if resp.into_reader().read_to_end(&mut bytes).is_err() || bytes.len() < 1000 {
            continue;
        }
        let _ = fs::create_dir_all(cache_dir);
        if fs::write(&dest, &bytes).is_ok() {
            return Some(dest);
        }
    }
    None
}

fn load_texture(ctx: &Context, key: &str, path: &Path) -> Option<TextureHandle> {
    let bytes = fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(key, color, TextureOptions::LINEAR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::GameSource;
    use tempfile::tempdir;

    #[test]
    fn resolve_uses_explicit_cover_path() {
        let dir = tempdir().unwrap();
        let img = dir.path().join("cover.png");
        let png = [
            0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        fs::write(&img, png).unwrap();
        let game = GameEntry {
            app_id: None,
            name: "Test".into(),
            source: GameSource::Faugus,
            install_path: None,
            launch_options: None,
            cover_path: Some(img.clone()),
            launcher_id: None,
        };
        assert_eq!(
            resolve_cover_path(&game, dir.path()).as_deref(),
            Some(img.as_path())
        );
    }

    #[test]
    fn steam_cover_finds_nested_hash_capsule() {
        let home = tempdir().unwrap();
        let app_id = 55_3850u32;
        let nested = home.path().join(format!(
            ".local/share/Steam/appcache/librarycache/{app_id}/abcd1234"
        ));
        fs::create_dir_all(&nested).unwrap();
        let png = [
            0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let img = nested.join("library_capsule.jpg");
        fs::write(&img, png).unwrap();

        let prev = std::env::var_os("HOME");
        std::env::set_var("HOME", home.path());
        let game = GameEntry {
            app_id: Some(app_id),
            name: "HELLDIVERS".into(),
            source: GameSource::Steam,
            install_path: None,
            launch_options: None,
            cover_path: None,
            launcher_id: None,
        };
        let found = resolve_cover_path(&game, home.path());
        if let Some(h) = prev {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        assert_eq!(found.as_deref(), Some(img.as_path()));
    }
}
