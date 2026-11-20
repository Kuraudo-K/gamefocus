//! Page Réglages.

use egui::{Color32, Frame, Margin, RichText, Rounding, ScrollArea, Stroke};

use crate::adapters::process_list::is_listed;
use crate::adapters::proton_tools::list_proton_tools;
use crate::domain::{is_critical_process, GridCoverSize, ListCoverSize, SortMode, ThemeMode, ViewMode};

use super::{theme, widgets, GameFocusApp, SettingsTab, SourceFilter};

impl GameFocusApp {
    pub(super) fn ui_settings(&mut self, ui: &mut egui::Ui) {
        let config_before = self.config.clone();
        ScrollArea::vertical().show(ui, |ui| {
            widgets::section_title(ui, "Réglages");
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (SettingsTab::Display, "Affichage"),
                    (SettingsTab::Launch, "Lancement"),
                    (SettingsTab::Keep, "À garder"),
                    (SettingsTab::Suspend, "À geler"),
                    (SettingsTab::Running, "En cours"),
                ] {
                    if widgets::setting_choice(ui, self.settings_tab == tab, label).clicked() {
                        self.settings_tab = tab;
                        if tab == SettingsTab::Running {
                            self.refresh_running();
                        }
                        if tab == SettingsTab::Launch {
                            self.proton_tools = list_proton_tools();
                        }
                    }
                }
            });
            ui.separator();

            match self.settings_tab {
                SettingsTab::Display => {
                    widgets::section_title(ui, "Thème");
                    ui.horizontal_wrapped(|ui| {
                        for mode in [ThemeMode::System, ThemeMode::Dark, ThemeMode::Light] {
                            if widgets::setting_choice(ui, self.config.theme_mode == mode, mode.label())
                                .clicked()
                            {
                                self.config.theme_mode = mode;
                                self.save_settings();
                            }
                        }
                    });
                    ui.add_space(8.0);
                    widgets::section_title(ui, "Vue de la bibliothèque");
                    ui.horizontal(|ui| {
                        for mode in [ViewMode::List, ViewMode::Grid] {
                            if widgets::setting_choice(ui, self.config.view_mode == mode, mode.label())
                                .clicked()
                            {
                                self.config.view_mode = mode;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    widgets::section_title(ui, "Taille des jaquettes - Grille");
                    ui.horizontal_wrapped(|ui| {
                        for size in [
                            GridCoverSize::Small,
                            GridCoverSize::Medium,
                            GridCoverSize::Large,
                            GridCoverSize::XLarge,
                        ] {
                            if widgets::setting_choice(ui, self.config.grid_cover_size == size, size.label())
                                .clicked()
                            {
                                self.config.grid_cover_size = size;
                            }
                        }
                    });
                    widgets::section_title(ui, "Taille des jaquettes - Liste");
                    ui.horizontal_wrapped(|ui| {
                        for size in [
                            ListCoverSize::Normal,
                            ListCoverSize::Large,
                            ListCoverSize::XLarge,
                        ] {
                            if widgets::setting_choice(ui, self.config.list_cover_size == size, size.label())
                                .clicked()
                            {
                                self.config.list_cover_size = size;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    widgets::section_title(ui, "Filtre lanceur");
                    ui.horizontal_wrapped(|ui| {
                        for f in [
                            SourceFilter::All,
                            SourceFilter::Steam,
                            SourceFilter::Heroic,
                            SourceFilter::Faugus,
                            SourceFilter::Lutris,
                        ] {
                            if widgets::setting_choice(ui, self.filter == f, f.label()).clicked() {
                                self.filter = f;
                            }
                        }
                    });
                    widgets::section_title(ui, "Tri");
                    ui.horizontal(|ui| {
                        for mode in [SortMode::Name, SortMode::Launcher] {
                            if widgets::setting_choice(ui, self.config.sort_mode == mode, mode.label())
                                .clicked()
                            {
                                self.config.sort_mode = mode;
                            }
                        }
                    });
                    widgets::section_title(ui, "Mode Focus");
                    Frame::none()
                        .fill(theme::panel())
                        .stroke(Stroke::new(1.0_f32, theme::line()))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.checkbox(
                                &mut self.config.focus_preview,
                                "Confirmer avant d'activer Focus",
                            );
                            ui.separator();
                            ui.label(
                                "GameMode s'active uniquement si le jeu est lancé avec gamemoderun.",
                            );
                            ui.separator();
                            ui.checkbox(
                                &mut self.config.focus.aggressive,
                                "Mode console",
                            );
                            ui.label(
                                RichText::new(
                                    "Ajoute aussi les applis de bureau et outils de travail à la \
                                     liste des processus à geler.",
                                )
                                .color(theme::muted()),
                            );
                            ui.separator();
                            ui.checkbox(
                                &mut self.config.focus.pause_wallpapers,
                                "Mettre en pause les fonds d'écran animés",
                            );
                            ui.separator();
                            ui.checkbox(
                                &mut self.config.focus.drop_page_cache,
                                "Demander à Linux de libérer son cache disque récupérable",
                            );
                            ui.label(
                                RichText::new(
                                    "Cette dernière option requiert les droits administrateur; \
                                     elle ne vide ni la mémoire des applis ni la VRAM.",
                                )
                                .color(theme::muted()),
                            );
                        });
                    if widgets::btn(ui, "Enregistrer", theme::acc()).clicked() {
                        self.save_settings();
                        self.do_scan();
                    }
                }
                SettingsTab::Launch => {
                    ScrollArea::vertical().show(ui, |ui| {
                        widgets::section_title(ui, "Proton");
                        ui.add_space(6.0);
                        Frame::none()
                            .fill(theme::panel())
                            .stroke(Stroke::new(1.0_f32, theme::line()))
                            .rounding(Rounding::same(10.0))
                            .inner_margin(Margin::same(12.0))
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    let auto_sel = self.config.launch.proton_tool.is_empty();
                                    if widgets::setting_choice(ui, auto_sel, "Auto (launcher)").clicked() {
                                        self.config.launch.proton_tool.clear();
                                    }
                                    for tool in &self.proton_tools {
                                        let sel = self.config.launch.proton_tool == tool.name;
                                        if widgets::setting_choice(ui, sel, &tool.name).clicked() {
                                            self.config.launch.proton_tool = tool.name.clone();
                                        }
                                    }
                                });
                                ui.checkbox(
                                    &mut self.config.launch.apply_proton_on_launch,
                                    "Appliquer au lancement",
                                );
                            });
                        ui.add_space(8.0);
                        if widgets::btn(
                            ui,
                            "Appliquer à tous les jeux maintenant",
                            Color32::from_rgb(50, 100, 160),
                        )
                        .clicked()
                        {
                            if let Some(tool) = self
                                .proton_tools
                                .iter()
                                .find(|t| t.name == self.config.launch.proton_tool)
                                .cloned()
                            {
                                let (_ok, _fail, msg) =
                                    crate::adapters::apply_proton::apply_proton_to_all(
                                        &self.games,
                                        &tool,
                                    );
                                self.status = msg;
                                self.error = false;
                                self.save_settings();
                            } else {
                                self.status = "Choisis un Proton (pas Auto).".into();
                                self.error = true;
                            }
                        }

                        ui.add_space(16.0);
                        widgets::section_title(ui, "Tweaks");
                        Frame::none()
                            .fill(theme::panel())
                            .stroke(Stroke::new(1.0_f32, theme::line()))
                            .rounding(Rounding::same(10.0))
                            .inner_margin(Margin::same(14.0))
                            .show(ui, |ui| {
                                widgets::on_off_row(
                                    ui,
                                    "HDR (selon le bureau)",
                                    &mut self.config.launch.hdr,
                                );
                                let hdr_hint = match crate::domain::detect_desktop() {
                                    crate::domain::DesktopKind::Gnome => {
                                        "GNOME : PROTON_ENABLE_HDR=1 DXVK_HDR=1 (comme avant)"
                                    }
                                    crate::domain::DesktopKind::Kde => {
                                        "KDE : DXVK_HDR=1 + gamescope -f --hdr-enabled"
                                    }
                                    crate::domain::DesktopKind::Other => {
                                        "Autre DE : DXVK_HDR=1 + gamescope -f --hdr-enabled"
                                    }
                                };
                                ui.label(
                                    egui::RichText::new(hdr_hint)
                                        .small()
                                        .color(theme::muted()),
                                );
                                ui.separator();
                                widgets::on_off_row(
                                    ui,
                                    "FSR 4 (FSR4_UPGRADE=1)",
                                    &mut self.config.launch.fsr,
                                );
                                ui.separator();
                                widgets::on_off_row(
                                    ui,
                                    "Logs Proton (~/steam-APPID.log)",
                                    &mut self.config.launch.proton_log,
                                );
                            });
                        ui.add_space(12.0);
                        if widgets::btn(ui, "Enregistrer", theme::acc()).clicked() {
                            self.save_settings();
                            self.refresh_recommended_with_tweaks();
                        }
                    });
                }
                SettingsTab::Keep => {
                    widgets::section_title(ui, "Exceptions");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_keep)
                                .hint_text("nom du process")
                                .desired_width(220.0),
                        );
                        if widgets::btn(ui, "Ajouter", Color32::from_rgb(40, 120, 90)).clicked() {
                            let n = self.new_keep.clone();
                            self.add_to_keep(&n);
                            self.new_keep.clear();
                        }
                    });
                    ui.add_space(8.0);
                    let mut rm = None;
                    ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        for (i, name) in self.config.focus.keep.iter().enumerate() {
                            Frame::none()
                                .fill(theme::panel())
                                .stroke(Stroke::new(1.0_f32, theme::line()))
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::symmetric(10.0, 7.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(name).strong().color(theme::text()));
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("Retirer").clicked() {
                                                    rm = Some(i);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
                    if let Some(i) = rm {
                        self.config.focus.keep.remove(i);
                        self.save_settings();
                    }
                }
                SettingsTab::Suspend => {
                    widgets::section_title(ui, "À geler");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_suspend)
                                .hint_text("firefox, chrome…")
                                .desired_width(220.0),
                        );
                        if widgets::btn(ui, "Ajouter", Color32::from_rgb(40, 120, 90)).clicked() {
                            let n = self.new_suspend.clone();
                            self.add_to_suspend(&n);
                            self.new_suspend.clear();
                        }
                    });
                    ui.add_space(8.0);
                    let mut rm = None;
                    ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        for (i, name) in self.config.focus.suspend.iter().enumerate() {
                            Frame::none()
                                .fill(theme::panel())
                                .stroke(Stroke::new(1.0_f32, theme::line()))
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::symmetric(10.0, 7.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(name).strong().color(theme::text()));
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button("Retirer").clicked() {
                                                    rm = Some(i);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
                    if let Some(i) = rm {
                        self.config.focus.suspend.remove(i);
                        self.save_settings();
                    }
                }
                SettingsTab::Running => {
                    widgets::section_title(ui, "Applications en cours");
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} processus · {} ms",
                            self.running.len(),
                            self.running_ms
                        ));
                        if ui.button("Rafraîchir").clicked() {
                            self.refresh_running();
                        }
                    });
                    ui.add(
                        egui::TextEdit::singleline(&mut self.running_filter)
                            .hint_text("Filtrer les applications en cours")
                            .desired_width(320.0),
                    );
                    ui.add_space(6.0);
                    #[derive(Clone, Copy)]
                    enum RunningAction {
                        Keep,
                        Suspend,
                    }
                    let mut action = None;
                    ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                        let filter = self.running_filter.to_lowercase();
                        let candidates: Vec<_> = self
                            .running
                            .iter()
                            .filter(|app| !is_critical_process(&app.name))
                            .filter(|app| {
                                filter.is_empty() || app.name.to_lowercase().contains(&filter)
                            })
                            .collect();
                        ui.label(format!("{} application(s)", candidates.len()));
                        for app in candidates {
                            let kept = is_listed(&app.name, &self.config.focus.keep);
                            let sus = is_listed(&app.name, &self.config.focus.suspend);
                            Frame::none()
                                .fill(theme::panel())
                                .stroke(Stroke::new(1.0_f32, theme::line()))
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::symmetric(10.0, 7.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&app.name).strong().color(theme::text()),
                                        );
                                        ui.label(
                                            RichText::new(format!("×{}", app.pid_count))
                                                .size(12.0)
                                                .color(theme::muted()),
                                        );
                                        if kept {
                                            ui.colored_label(theme::ok(), "gardé");
                                        } else if sus {
                                            ui.colored_label(
                                                Color32::from_rgb(193, 113, 30),
                                                "à geler",
                                            );
                                        }
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if !sus && ui.small_button("À geler").clicked() {
                                                    action = Some((
                                                        RunningAction::Suspend,
                                                        app.name.clone(),
                                                    ));
                                                }
                                                if !kept && ui.small_button("Garder").clicked() {
                                                    action = Some((
                                                        RunningAction::Keep,
                                                        app.name.clone(),
                                                    ));
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
                    if let Some((action, name)) = action {
                        match action {
                            RunningAction::Keep => self.add_to_keep(&name),
                            RunningAction::Suspend => self.add_to_suspend(&name),
                        }
                    }
                }
            }
        });
        if self.config != config_before {
            self.config.last_search = self.search.clone();
            if let Err(error) = self.service.save_config(&self.config) {
                self.status = format!("Sauvegarde des réglages : {error}");
                self.error = true;
            }
        }
    }
}
