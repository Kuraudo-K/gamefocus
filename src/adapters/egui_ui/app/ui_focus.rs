//! Page Mode Focus (proposition 5).

use egui::{Color32, Frame, Margin, RichText, Rounding, ScrollArea, Stroke, Vec2};

use crate::adapters::process_list::preview_focus_targets;

use super::{theme, GameFocusApp};

impl GameFocusApp {
    /// Page Focus centrée (proposition 5), couleurs prop. 1.
    pub(super) fn ui_focus_page(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            let box_w = 520.0_f32.min(ui.available_width() - 24.0);

            if self.focus_active {
                ui.label(
                    RichText::new("FOCUS ON")
                        .size(42.0)
                        .strong()
                        .color(theme::ok()),
                );
            } else {
                ui.label(
                    RichText::new("MODE FOCUS")
                        .size(36.0)
                        .strong()
                        .color(theme::acc()),
                );
            }

            ui.add_space(12.0);
            Frame::none()
                .fill(theme::panel())
                .stroke(Stroke::new(1.0_f32, theme::line()))
                .rounding(Rounding::same(12.0))
                .inner_margin(Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_width(box_w - 24.0);
                    if self.focus_preview_list.is_empty() {
                        ui.label(
                            RichText::new(if self.focus_active {
                                "Focus actif — rien de listé à afficher."
                            } else {
                                "Rien à geler pour l'instant — tu peux confirmer quand même."
                            })
                            .color(theme::muted()),
                        );
                    }
                    let mut keep = None;
                    ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for app in &self.focus_preview_list {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&app.name).size(14.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if !self.focus_active && ui.small_button("Garder").clicked()
                                        {
                                            keep = Some(app.name.clone());
                                        }
                                        ui.label(
                                            RichText::new(if self.focus_active {
                                                "gelé"
                                            } else {
                                                "à geler"
                                            })
                                            .size(13.0)
                                            .color(theme::muted()),
                                        );
                                    },
                                );
                            });
                            ui.separator();
                        }
                        // Exceptions visibles
                        for name in self.config.focus.keep.iter().take(6) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(name).size(14.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new("gardé").size(13.0).color(theme::ok()),
                                        );
                                    },
                                );
                            });
                        }
                    });
                    if let Some(name) = keep {
                        self.add_to_keep(&name);
                        let (list, ms) = preview_focus_targets(&self.config.focus);
                        self.focus_preview_list = list;
                        self.status = format!("Aperçu à jour ({} ms)", ms);
                    }
                });

            ui.add_space(20.0);
            // Même emplacement : Lancer ↔ Arrêter
            let (label, fill) = if self.focus_active {
                ("Arrêter", theme::stop())
            } else if self.launch_after_focus {
                ("Confirmer et lancer", theme::acc())
            } else {
                ("Lancer", theme::acc())
            };
            let resp = ui.add(
                egui::Button::new(
                    RichText::new(label)
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(fill)
                .stroke(Stroke::new(
                    2.0_f32,
                    if self.focus_active {
                        theme::stop()
                    } else {
                        theme::acc()
                    },
                ))
                .min_size(Vec2::new(box_w, 48.0))
                .rounding(Rounding::same(10.0)),
            );
            if resp.clicked() {
                if self.focus_active {
                    self.stop_focus();
                } else {
                    self.confirm_focus();
                }
            }
        });
    }
}
