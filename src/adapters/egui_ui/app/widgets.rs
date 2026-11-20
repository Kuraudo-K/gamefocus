//! Widgets réutilisables et helpers de mise en page.

use egui::{
    Color32, FontId, Frame, Margin, RichText, Rounding, Sense, Stroke, TextureHandle, Vec2,
    ViewportCommand,
};

use crate::domain::GameEntry;

use super::theme;

pub(super) fn game_config_key(game: &GameEntry) -> String {
    format!(
        "{}:{}",
        game.source.label(),
        game.launcher_id
            .as_deref()
            .map(str::to_owned)
            .or_else(|| game.app_id.map(|id| id.to_string()))
            .unwrap_or_else(|| game.name.to_lowercase())
    )
}

pub(super) fn launch_options_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(15.0)
                .strong()
                .color(theme::text()),
        );
        Frame::none()
            .fill(theme::panel())
            .stroke(Stroke::new(1.0_f32, theme::line()))
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::symmetric(10.0, 7.0))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(value)
                            .monospace()
                            .size(15.0)
                            .color(theme::text()),
                    )
                    .wrap(),
                );
            });
        ui.add_space(6.0);
    });
}

pub(super) fn format_memory_bytes(bytes: u64) -> String {
    let mib = 1024 * 1024;
    if bytes >= mib {
        format!("{}.{:01} MiB", bytes / mib, (bytes % mib) * 10 / mib)
    } else {
        format!("{} KiB", bytes / 1024)
    }
}

pub(super) fn btn(ui: &mut egui::Ui, label: &str, fill: Color32) -> egui::Response {
    let on_acc = fill == theme::acc() || fill == theme::acc_dim();
    let text = if fill == theme::panel() {
        theme::text()
    } else {
        Color32::WHITE
    };
    let stroke = if on_acc {
        Stroke::new(1.5_f32, theme::acc())
    } else if fill == theme::stop() {
        Stroke::new(1.5_f32, theme::stop())
    } else {
        Stroke::new(1.5_f32, theme::line())
    };
    ui.add(
        egui::Button::new(RichText::new(label).size(14.0).strong().color(text))
            .fill(fill)
            .stroke(stroke)
            .min_size(Vec2::new(0.0, 38.0))
            .rounding(Rounding::same(8.0)),
    )
}

pub(super) fn setting_choice(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let text = if selected {
        Color32::WHITE
    } else {
        theme::text()
    };
    ui.selectable_label(selected, RichText::new(label).color(text))
}

pub(super) fn nav_btn(ui: &mut egui::Ui, icon: &str, label: &str, on: bool) -> egui::Response {
    let fill = if on { theme::acc() } else { theme::panel() };
    let text = if on { Color32::WHITE } else { theme::text() };
    let stroke = if on {
        Stroke::new(2.0_f32, theme::acc())
    } else {
        Stroke::new(1.5_f32, Color32::from_rgb(70, 85, 105))
    };
    ui.add(
        egui::Button::new(RichText::new(icon).size(22.0).strong().color(text))
            .fill(fill)
            .stroke(stroke)
            .min_size(Vec2::splat(44.0))
            .rounding(Rounding::same(10.0)),
    )
    .on_hover_text(label)
}

pub(super) fn nav_text_btn(ui: &mut egui::Ui, label: &str, on: bool) -> egui::Response {
    let fill = if on { theme::acc() } else { theme::panel() };
    let text = if on { Color32::WHITE } else { theme::text() };
    let stroke = if on {
        Stroke::new(2.0_f32, theme::acc())
    } else {
        Stroke::new(1.5_f32, Color32::from_rgb(70, 85, 105))
    };
    ui.add(
        egui::Button::new(RichText::new(label).size(15.0).strong().color(text))
            .fill(fill)
            .stroke(stroke)
            .min_size(Vec2::new(128.0, 42.0))
            .rounding(Rounding::same(10.0)),
    )
}

/// Icônes Liste (3 traits) / Grille (4 cases) — dessinées, comme prop. 1.
#[derive(Clone, Copy)]
pub(super) enum ViewIcon {
    List,
    Grid,
}

pub(super) fn view_toggle(ui: &mut egui::Ui, kind: ViewIcon, on: bool) -> egui::Response {
    let size = Vec2::new(32.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on { theme::acc() } else { theme::panel() };
    let stroke = if on {
        Stroke::new(1.5_f32, theme::acc())
    } else {
        Stroke::new(1.5_f32, Color32::from_rgb(70, 85, 105))
    };
    let ink = if on { Color32::WHITE } else { theme::text() };
    let painter = ui.painter();
    painter.rect(rect, Rounding::same(6.0), fill, stroke);
    match kind {
        ViewIcon::List => {
            // ≡ trois barres horizontales
            let x0 = rect.left() + 8.0;
            let x1 = rect.right() - 8.0;
            let cy = rect.center().y;
            for dy in [-6.0_f32, 0.0, 6.0] {
                painter.line_segment(
                    [egui::pos2(x0, cy + dy), egui::pos2(x1, cy + dy)],
                    Stroke::new(2.2_f32, ink),
                );
            }
        }
        ViewIcon::Grid => {
            // ▦ quatre petits carrés
            let gap = 3.0_f32;
            let cell = 7.0_f32;
            let total = cell * 2.0 + gap;
            let ox = rect.center().x - total * 0.5;
            let oy = rect.center().y - total * 0.5;
            for row in 0..2 {
                for col in 0..2 {
                    let r = egui::Rect::from_min_size(
                        egui::pos2(
                            ox + col as f32 * (cell + gap),
                            oy + row as f32 * (cell + gap),
                        ),
                        Vec2::splat(cell),
                    );
                    painter.rect_stroke(r, Rounding::same(1.5), Stroke::new(1.8_f32, ink));
                }
            }
        }
    }
    resp.on_hover_text(match kind {
        ViewIcon::List => "Liste",
        ViewIcon::Grid => "Grille",
    })
}

/// Boutons ON / OFF bien visibles (réglages Lancement).
pub(super) fn on_off_row(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(14.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let off_fill = if !*value {
                theme::stop()
            } else {
                theme::neutral()
            };
            let on_fill = if *value {
                theme::acc()
            } else {
                theme::neutral()
            };
            if ui
                .add(
                    egui::Button::new(RichText::new("OFF").size(13.0).strong().color(if !*value {
                        Color32::WHITE
                    } else {
                        theme::muted()
                    }))
                    .fill(off_fill)
                    .min_size(Vec2::new(56.0, 34.0))
                    .rounding(Rounding::same(8.0)),
                )
                .clicked()
            {
                *value = false;
            }
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("ON").size(13.0).strong().color(if *value {
                        Color32::WHITE
                    } else {
                        theme::muted()
                    }))
                    .fill(on_fill)
                    .min_size(Vec2::new(56.0, 34.0))
                    .rounding(Rounding::same(8.0)),
                )
                .clicked()
            {
                *value = true;
            }
        });
    });
}

pub(super) fn win_btn(ui: &mut egui::Ui, label: &str, fill: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(16.0))
            .fill(fill)
            .min_size(Vec2::new(36.0, 28.0))
            .rounding(Rounding::same(6.0)),
    )
}

/// Bords redimensionnables (fenêtre sans décorations natives — GNOME/KDE Wayland).
pub(super) fn handle_window_resize_edges(ctx: &egui::Context) {
    use egui::ResizeDirection;
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }
    let (rect, pointer, primary) = ctx.input(|i| {
        (
            i.screen_rect(),
            i.pointer.hover_pos(),
            i.pointer.button_pressed(egui::PointerButton::Primary),
        )
    });
    let Some(pos) = pointer else {
        return;
    };
    let grip = 6.0_f32;
    let on_left = pos.x <= rect.left() + grip;
    let on_right = pos.x >= rect.right() - grip;
    let on_top = pos.y <= rect.top() + grip;
    let on_bottom = pos.y >= rect.bottom() - grip;

    let (dir, cursor) = match (on_left, on_right, on_top, on_bottom) {
        (true, false, true, false) => (
            Some(ResizeDirection::NorthWest),
            egui::CursorIcon::ResizeNwSe,
        ),
        (false, true, true, false) => (
            Some(ResizeDirection::NorthEast),
            egui::CursorIcon::ResizeNeSw,
        ),
        (true, false, false, true) => (
            Some(ResizeDirection::SouthWest),
            egui::CursorIcon::ResizeNeSw,
        ),
        (false, true, false, true) => (
            Some(ResizeDirection::SouthEast),
            egui::CursorIcon::ResizeNwSe,
        ),
        (false, false, true, false) => (
            Some(ResizeDirection::North),
            egui::CursorIcon::ResizeVertical,
        ),
        (false, false, false, true) => (
            Some(ResizeDirection::South),
            egui::CursorIcon::ResizeVertical,
        ),
        (true, false, false, false) => (
            Some(ResizeDirection::West),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (false, true, false, false) => (
            Some(ResizeDirection::East),
            egui::CursorIcon::ResizeHorizontal,
        ),
        _ => (None, egui::CursorIcon::Default),
    };

    if let Some(dir) = dir {
        ctx.set_cursor_icon(cursor);
        if primary {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}

/// Image fixe — taille verrouillée pour ne pas exploser le layout.
pub(super) fn cover_fixed(ui: &mut egui::Ui, tex: Option<&TextureHandle>, w: f32, h: f32) {
    let size = Vec2::new(w, h);
    if let Some(tex) = tex {
        ui.add(
            egui::Image::new(tex)
                .fit_to_exact_size(size)
                .maintain_aspect_ratio(true)
                .bg_fill(theme::panel())
                .rounding(Rounding::same(6.0)),
        );
    } else {
        let (rect, _resp) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter()
            .rect_filled(rect, Rounding::same(6.0), theme::panel());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            FontId::proportional(16.0),
            theme::muted(),
        );
    }
}

pub(super) fn section_title(ui: &mut egui::Ui, title: &str) {
    Frame::none()
        .fill(theme::section())
        .stroke(Stroke::new(1.0_f32, theme::line()))
        .rounding(Rounding::same(7.0))
        .inner_margin(Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(14.0)
                    .strong()
                    .color(theme::text()),
            );
        });
    ui.add_space(5.0);
}
