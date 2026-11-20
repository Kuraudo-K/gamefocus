//! Vues bibliothèque : liste, grille et panneau de détail.

use egui::{
    Color32, FontId, Frame, Margin, RichText, Rounding, ScrollArea, Sense, Stroke, Vec2,
};

use crate::adapters::shared::urlencoding_lite;
use crate::domain::{tier_label_fr, tier_rgb, ProtonTierKind, ViewMode};

use super::{theme, widgets, GameFocusApp};

impl GameFocusApp {
    pub(super) fn ui_library(&mut self, ctx: &egui::Context) {
        // Barre simple prop. 1 : recherche + liste/grille
        egui::TopBottomPanel::top("filters")
            .exact_height(56.0)
            .frame(
                Frame::none()
                    .fill(theme::bg())
                    .stroke(Stroke::new(1.0_f32, theme::line()))
                    .inner_margin(Margin::symmetric(16.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    Frame::none()
                        .fill(theme::panel())
                        .stroke(Stroke::new(1.0_f32, theme::line()))
                        .stroke(Stroke::new(1.0_f32, theme::line()))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .hint_text("Recherche…")
                                    .desired_width(340.0)
                                    .text_color(theme::text())
                                    .frame(false),
                            );
                        });

                    if ui.input(|i| i.key_pressed(egui::Key::F5)) {
                        self.refresh_running();
                        self.do_scan();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let grid_on = self.config.view_mode == ViewMode::Grid;
                        if widgets::view_toggle(ui, widgets::ViewIcon::Grid, grid_on).clicked() {
                            self.config.view_mode = ViewMode::Grid;
                            self.save_settings();
                        }
                        ui.add_space(6.0);
                        let list_on = self.config.view_mode == ViewMode::List;
                        if widgets::view_toggle(ui, widgets::ViewIcon::List, list_on).clicked() {
                            self.config.view_mode = ViewMode::List;
                            self.save_settings();
                        }
                    });
                });
            });

        match self.config.view_mode {
            ViewMode::List => {
                // Liste à gauche (largeur fixe), détail à droite
                egui::SidePanel::left("games_side")
                    .resizable(true)
                    .default_width(340.0)
                    .width_range(260.0..=480.0)
                    .show(ctx, |ui| {
                        self.ui_game_list(ui, ctx);
                    });
                egui::CentralPanel::default().show(ctx, |ui| {
                    ScrollArea::vertical()
                        .id_salt("detail_scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            self.ui_detail(ui, ctx);
                        });
                });
            }
            ViewMode::Grid => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.ui_game_grid(ui, ctx);
                });
            }
        }
    }

    pub(super) fn ui_game_list(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let indices = self.filtered_indices();
        if indices.is_empty() {
            ui.label("Aucun jeu — Rescanner.");
            return;
        }
        let mut clicked = None;
        ScrollArea::vertical()
            .id_salt("list_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for idx in indices {
                    let sel = self.selected == Some(idx);
                    let game = &self.games[idx];
                    let tex = self.covers.texture_for(ctx, game);

                    let stroke = if sel {
                        Stroke::new(1.5_f32, theme::acc())
                    } else {
                        Stroke::NONE
                    };
                    let fill = if sel {
                        theme::acc_dim()
                    } else {
                        theme::panel()
                    };

                    let resp = Frame::none()
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(8.0, 6.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (tw, th) = self.config.list_cover_size.thumb_size();
                                widgets::cover_fixed(ui, tex.as_ref(), tw, th);
                                ui.add_space(8.0);
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&game.name).size(14.0).strong().color(
                                            if sel { Color32::WHITE } else { theme::text() },
                                        ),
                                    );
                                    ui.label(
                                        RichText::new(game.source.label()).size(12.0).color(
                                            if sel { Color32::WHITE } else { theme::muted() },
                                        ),
                                    );
                                });
                            });
                        })
                        .response
                        .interact(Sense::click());

                    if resp.clicked() {
                        clicked = Some(idx);
                    }
                    ui.add_space(4.0);
                }
            });
        if let Some(idx) = clicked {
            self.select_game(idx);
        }
    }

    pub(super) fn ui_game_grid(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let indices = self.filtered_indices();
        let mut clicked = None;

        // Largeur réelle de la fenêtre (ScrollArea donne sinon une largeur infinie
        // → une seule ligne sans retour à la ligne).
        let avail_w = ui.available_width();
        let card_w = self.config.grid_cover_size.card_width();
        let gap = 10.0_f32;
        let cols = ((avail_w + gap) / (card_w + gap)).floor().max(1.0) as usize;

        ScrollArea::vertical()
            .id_salt("grid_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_width(avail_w);
                egui::Grid::new("game_grid_cells")
                    .num_columns(cols)
                    .spacing([gap, gap])
                    .show(ui, |ui| {
                        for (n, idx) in indices.into_iter().enumerate() {
                            let sel = self.selected == Some(idx);
                            let game = &self.games[idx];
                            let tex = self.covers.texture_for(ctx, game);
                            let stroke = if sel {
                                Stroke::new(2.0_f32, theme::acc())
                            } else {
                                Stroke::new(1.0_f32, theme::line())
                            };

                            // Carte simple prop. 1 : jaquette + nom en bas
                            let resp = Frame::none()
                                .fill(theme::panel())
                                .stroke(stroke)
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::same(0.0))
                                .show(ui, |ui| {
                                    ui.set_width(card_w);
                                    let cover_h = card_w * 1.5;
                                    let (rect, _) = ui.allocate_exact_size(
                                        Vec2::new(card_w, cover_h),
                                        Sense::hover(),
                                    );
                                    if let Some(tex) = tex.as_ref() {
                                        egui::Image::new(tex)
                                            .fit_to_exact_size(rect.size())
                                            .rounding(Rounding::same(8.0))
                                            .paint_at(ui, rect);
                                    } else {
                                        ui.painter().rect_filled(
                                            rect,
                                            Rounding::same(8.0),
                                            theme::panel(),
                                        );
                                    }
                                    // Nom en overlay bas
                                    let label = if game.name.chars().count() > 14 {
                                        format!(
                                            "{}…",
                                            game.name.chars().take(13).collect::<String>()
                                        )
                                    } else {
                                        game.name.clone()
                                    };
                                    let label_rect = egui::Rect::from_min_max(
                                        egui::pos2(rect.left(), rect.bottom() - 26.0),
                                        rect.right_bottom(),
                                    );
                                    ui.painter().rect_filled(
                                        label_rect,
                                        Rounding::ZERO,
                                        Color32::from_black_alpha(170),
                                    );
                                    ui.painter().text(
                                        egui::pos2(rect.left() + 8.0, rect.bottom() - 10.0),
                                        egui::Align2::LEFT_BOTTOM,
                                        label,
                                        FontId::proportional(12.0),
                                        Color32::WHITE,
                                    );
                                })
                                .response
                                .interact(Sense::click());

                            if resp.clicked() {
                                clicked = Some(idx);
                            }
                            if (n + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
        if let Some(idx) = clicked {
            self.select_game(idx);
        }
    }

    pub(super) fn ui_detail(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.refresh_selected_launch_options(false);
        let Some(idx) = self.selected else {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(RichText::new("Choisis un jeu").size(16.0));
            });
            return;
        };

        let game = self.games[idx].clone();
        let tex = self.covers.texture_for(ctx, &game);
        let current_launch_options = self.steam_options_value.clone();
        let options_key = widgets::game_config_key(&game);
        let managed_options = self
            .config
            .launch_options_managed
            .get(&options_key)
            .cloned()
            .unwrap_or_default();
        let (managed_present, managed_total) =
            match crate::adapters::launch_opts::managed_launch_options_status(
                current_launch_options.as_deref(),
                &managed_options,
            ) {
                Ok(status) => status,
                Err(error) => {
                    self.steam_options_error = Some(error);
                    (0, managed_options.len())
                }
            };
        let options_enabled = managed_total > 0 && managed_present == managed_total;
        let options_partial = managed_present > 0 && !options_enabled;
        let options_modified = current_launch_options
            .as_deref()
            .is_some_and(|options| !options.trim().is_empty())
            && !options_enabled
            && !options_partial;
        let preview = crate::adapters::launch_opts::preview_launch_options(
            &game,
            current_launch_options.as_deref(),
            &self.recommended,
        );
        let (final_options, new_options) = match preview {
            Ok((final_options, added)) => (Some(final_options), added.join(" ")),
            Err(error) => {
                self.steam_options_error = Some(error);
                (None, String::new())
            }
        };

        // Détail style prop. 2 (dans coque prop. 1)
        Frame::none()
            .fill(theme::panel())
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::same(18.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::cover_fixed(ui, tex.as_ref(), 80.0, 108.0);
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(game.source.label())
                                .size(12.0)
                                .color(theme::muted()),
                        );
                        ui.label(
                            RichText::new(&game.name)
                                .size(28.0)
                                .strong()
                                .color(theme::text()),
                        );
                        if let Some(path) = &game.install_path {
                            ui.label(
                                RichText::new(path.display().to_string())
                                    .size(11.0)
                                    .color(theme::muted()),
                            );
                        }
                        match &self.proton {
                            Some(p) => {
                                let kind = p.tier_kind();
                                let (r, g, b) = tier_rgb(kind);
                                ui.label(
                                    RichText::new(format!(
                                        "ProtonDB · {} · {} rapports · {}",
                                        p.tier.to_uppercase(),
                                        p.total,
                                        tier_label_fr(kind)
                                    ))
                                    .strong()
                                    .color(Color32::from_rgb(r, g, b)),
                                );
                            }
                            None => {
                                let (r, g, b) = tier_rgb(ProtonTierKind::Untested);
                                ui.label(
                                    RichText::new("ProtonDB · jamais testé / pas de rapport")
                                        .color(Color32::from_rgb(r, g, b)),
                                );
                            }
                        }
                    });
                });

                ui.add_space(12.0);
                widgets::section_title(ui, "Options de lancement");
                Frame::none()
                    .fill(theme::bg())
                    .stroke(Stroke::new(1.0_f32, theme::line()))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        widgets::launch_options_row(ui, "Options GameFocus", &new_options);
                        widgets::launch_options_row(
                            ui,
                            "Options déjà présentes",
                            current_launch_options.as_deref().unwrap_or("Aucune"),
                        );
                        widgets::launch_options_row(
                            ui,
                            "Résultat après activation",
                            final_options
                                .as_deref()
                                .unwrap_or("Options non disponibles"),
                        );
                        if let Some(error) = &self.steam_options_error {
                            ui.colored_label(
                                theme::stop(),
                                format!("Lecture impossible : {error}"),
                            );
                        }
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                if options_enabled {
                                    theme::ok()
                                } else if options_partial {
                                    Color32::from_rgb(220, 150, 40)
                                } else if options_modified {
                                    Color32::from_rgb(220, 150, 40)
                                } else {
                                    theme::muted()
                                },
                                if options_enabled {
                                    "Options GameFocus actives"
                                } else if options_partial {
                                    "Options GameFocus partiellement actives"
                                } else if options_modified {
                                    "Options personnelles conservées"
                                } else {
                                    "Aucune option GameFocus active"
                                },
                            );
                            if widgets::btn(ui, "Copier le résultat", theme::panel()).clicked() {
                                if let Some(options) = &final_options {
                                    ctx.copy_text(options.clone());
                                }
                            }
                        });
                    });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if widgets::btn(ui, "Lancer", theme::acc_dim()).clicked() {
                        self.launch_selected();
                    }
                    if widgets::btn(ui, "Lancer + Focus", theme::acc()).clicked() {
                        self.launch_with_focus();
                    }
                    if options_enabled || options_partial {
                        if widgets::btn(ui, "Retirer options", theme::stop()).clicked() {
                            self.clear_options();
                        }
                    } else if self.steam_options_error.is_some() {
                        ui.add_enabled(false, egui::Button::new("Vérifier les options d'abord"));
                    } else if options_modified
                        && final_options.as_deref() == current_launch_options.as_deref()
                    {
                        ui.label(RichText::new("Aucune option manquante").color(theme::muted()));
                    } else if widgets::btn(ui, "Activer les options", theme::acc()).clicked() {
                        self.activate_options();
                    }
                });

                ui.add_space(8.0);
                widgets::section_title(ui, "Sites");
                ui.horizontal_wrapped(|ui| {
                    let q = urlencoding_lite(&game.name);
                    if let Some(id) = self.service.resolve_steam_app_id(&game) {
                        if widgets::btn(ui, "ProtonDB", Color32::from_rgb(60, 100, 70)).clicked() {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(format!("https://www.protondb.com/app/{id}"))
                                .spawn();
                        }
                        if ui.link("SteamDB").clicked() {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(format!("https://steamdb.info/app/{id}/"))
                                .spawn();
                        }
                    } else {
                        if widgets::btn(ui, "ProtonDB (recherche)", Color32::from_rgb(60, 100, 70))
                            .clicked()
                        {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(format!("https://www.protondb.com/?q={q}"))
                                .spawn();
                        }
                    }
                    if ui.link("ProtonReady").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(format!("https://protonready.com/?q={q}"))
                            .spawn();
                    }
                    if ui.link("WineHQ").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(format!(
                                "https://appdb.winehq.org/objectManager.php?sClass=application&sTitle=true&iSearchFrom=0&iItemsPerPage=25&sOrderBy=appName&bAscendant=true&sName={q}"
                            ))
                            .spawn();
                    }
                    if ui.link("AreWeAntiCheatYet").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(format!("https://areweanticheatyet.com/?q={q}"))
                            .spawn();
                    }
                    if ui.link("PCGamingWiki").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(format!(
                                "https://www.pcgamingwiki.com/w/index.php?search={q}"
                            ))
                            .spawn();
                    }
                    if let Some(id) = self.service.resolve_steam_app_id(&game) {
                        if ui.link("Discussions Steam").clicked() {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(format!(
                                    "https://steamcommunity.com/app/{id}/discussions/"
                                ))
                                .spawn();
                        }
                    }
                    if ui.link("Documentation Proton (Valve)").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg("https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ")
                            .spawn();
                    }
                    if ui.link("GameMode (Feral)").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg("https://github.com/FeralInteractive/gamemode")
                            .spawn();
                    }
                    if ui.link("Variables Mesa").clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg("https://docs.mesa3d.org/envvars.html")
                            .spawn();
                    }
                    if ui.link("Actualiser ProtonDB").clicked() {
                        self.select_game(idx);
                    }
                });
            });
    }
}
