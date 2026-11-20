//! Palette de l'interface, application du thème et détection système.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Color32, Stroke, Vec2};

static DARK: AtomicBool = AtomicBool::new(true);

pub(super) fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

fn color(dark: (u8, u8, u8), light: (u8, u8, u8)) -> Color32 {
    let (r, g, b) = if DARK.load(Ordering::Relaxed) {
        dark
    } else {
        light
    };
    Color32::from_rgb(r, g, b)
}

pub(super) fn bg() -> Color32 {
    color((18, 21, 28), (235, 239, 238))
}
pub(super) fn panel() -> Color32 {
    color((26, 31, 42), (247, 248, 245))
}
pub(super) fn section() -> Color32 {
    color((24, 29, 39), (222, 231, 228))
}
pub(super) fn line() -> Color32 {
    color((42, 51, 68), (169, 184, 183))
}
pub(super) fn text() -> Color32 {
    color((232, 237, 244), (34, 48, 51))
}
pub(super) fn muted() -> Color32 {
    color((139, 151, 168), (76, 94, 97))
}
pub(super) fn acc() -> Color32 {
    color((42, 157, 143), (29, 112, 102))
}
pub(super) fn acc_dim() -> Color32 {
    color((30, 122, 112), (25, 96, 88))
}
pub(super) fn stop() -> Color32 {
    color((200, 91, 91), (166, 52, 52))
}
pub(super) fn ok() -> Color32 {
    color((98, 214, 138), (35, 112, 67))
}
pub(super) fn neutral() -> Color32 {
    color((40, 46, 56), (228, 234, 232))
}

pub(super) fn apply_theme(ctx: &egui::Context, dark: bool) {
    set_dark(dark);
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "GameFocus Sans".to_owned(),
        egui::FontData::from_static(include_bytes!("../../../../assets/DejaVuSans.ttf")),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .expect("default proportional font family")
        .insert(0, "GameFocus Sans".to_owned());
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.widgets.inactive.bg_fill = panel();
    style.visuals.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(36, 48, 58)
    } else {
        Color32::from_rgb(216, 229, 225)
    };
    style.visuals.widgets.active.bg_fill = acc_dim();
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.panel_fill = bg();
    style.visuals.window_fill = panel();
    style.visuals.extreme_bg_color = if dark {
        Color32::from_rgb(14, 16, 20)
    } else {
        Color32::from_rgb(230, 236, 234)
    };
    style.visuals.override_text_color = Some(text());
    style.visuals.selection.bg_fill = acc_dim();
    style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    if !dark {
        let separator = Stroke::new(1.0_f32, line());
        style.visuals.widgets.inactive.bg_stroke = separator;
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, acc());
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, acc_dim());
        style.visuals.window_stroke = separator;
        style.visuals.widgets.noninteractive.bg_stroke = separator;
    }
    ctx.set_style(style);
}

/// Détecte le thème clair/sombre du bureau.
/// Ordre : portail XDG → KDE Plasma → gsettings (GNOME) → GTK_THEME.
pub(super) fn system_prefers_dark() -> Option<bool> {
    if let Some(dark) = portal_prefers_dark() {
        return Some(dark);
    }
    if let Some(dark) = kde_prefers_dark() {
        return Some(dark);
    }
    if let Some(dark) = gnome_prefers_dark() {
        return Some(dark);
    }
    gtk_theme_prefers_dark()
}

fn portal_prefers_dark() -> Option<bool> {
    // org.freedesktop.appearance color-scheme : 0=no pref, 1=dark, 2=light
    let output = std::process::Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Ex. : `(<<uint32 1>>,)` ou `(<<1>>,)`
    if text.contains("uint32 1") || text.contains("<<1>>") || text.contains("<1>") {
        return Some(true);
    }
    if text.contains("uint32 2") || text.contains("<<2>>") || text.contains("<2>") {
        return Some(false);
    }
    None
}

fn gnome_prefers_dark() -> Option<bool> {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let scheme = String::from_utf8_lossy(&output.stdout).to_lowercase();
    if scheme.contains("prefer-dark") {
        return Some(true);
    }
    if scheme.contains("prefer-light") {
        return Some(false);
    }
    if scheme.contains("default") {
        let theme = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
            .output()
            .ok()?;
        if theme.status.success() {
            let theme = String::from_utf8_lossy(&theme.stdout).to_lowercase();
            return Some(theme.contains("dark"));
        }
    }
    None
}

fn gtk_theme_prefers_dark() -> Option<bool> {
    let theme = std::env::var_os("GTK_THEME")?;
    let theme = theme.to_string_lossy().to_lowercase();
    if theme.is_empty() {
        return None;
    }
    Some(theme.contains("dark"))
}

fn kde_prefers_dark() -> Option<bool> {
    let home = std::env::var_os("HOME")?;
    let kdeglobals = PathBuf::from(home).join(".config/kdeglobals");
    let contents = std::fs::read_to_string(kdeglobals).ok()?;
    let lower = contents.to_lowercase();

    // Plasma 5/6 : LookAndFeelPackage=org.kde.breezedark.desktop
    if let Some(pkg) = ini_value(&lower, "lookandfeelpackage") {
        if pkg.contains("dark") {
            return Some(true);
        }
        if pkg.contains("light") || pkg.contains("breeze.desktop") {
            return Some(false);
        }
    }

    // ColorScheme=BreezeDark (parfois absent en Plasma 6, remplacé par le hash)
    if let Some(scheme) = ini_value(&lower, "colorscheme") {
        if scheme.contains("dark") {
            return Some(true);
        }
        if scheme.contains("light") || scheme == "breeze" {
            return Some(false);
        }
    }

    // Fallback fiable : luminance du fond de fenêtre
    window_background_is_dark(&contents)
}

fn ini_value<'a>(contents_lower: &'a str, key: &str) -> Option<&'a str> {
    contents_lower.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
            .map(str::trim)
    })
}

fn window_background_is_dark(contents: &str) -> Option<bool> {
    let block = contents
        .split("[Colors:Window]")
        .nth(1)?
        .split('[')
        .next()?;
    let rgb = block.lines().find_map(|line| {
        line.trim()
            .strip_prefix("BackgroundNormal=")
            .map(str::trim)
    })?;
    let mut parts = rgb.split(',').filter_map(|p| p.trim().parse::<f32>().ok());
    let r = parts.next()?;
    let g = parts.next()?;
    let b = parts.next()?;
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    Some(lum < 128.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_luminance_detects_dark_breeze() {
        let sample = "\
[Colors:Window]
BackgroundNormal=32,35,38
ForegroundNormal=252,252,252
";
        assert_eq!(window_background_is_dark(sample), Some(true));
    }

    #[test]
    fn window_luminance_detects_light_breeze() {
        let sample = "\
[Colors:Window]
BackgroundNormal=252,252,252
ForegroundNormal=35,38,41
";
        assert_eq!(window_background_is_dark(sample), Some(false));
    }

    #[test]
    fn kde_lookandfeel_dark_parsed() {
        let sample = "\
[KDE]
LookAndFeelPackage=org.kde.breezedark.desktop
";
        assert_eq!(
            ini_value(&sample.to_lowercase(), "lookandfeelpackage"),
            Some("org.kde.breezedark.desktop")
        );
    }
}
