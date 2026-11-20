//! Interface stable : SidePanel + CentralPanel (pas de ui.columns).

use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{
    FontId, Frame, Margin, RichText, Rounding, ScrollArea, Sense, Stroke, TextureHandle,
    Vec2, ViewportCommand,
};

use crate::adapters::covers::CoverCache;
use crate::adapters::egui_ui::desktop::{DesktopCommand, DesktopCommandSender};
use crate::adapters::process_list::{is_listed, list_running_apps, preview_focus_targets};
use crate::adapters::proton_tools::{list_proton_tools, ProtonTool};
use crate::application::GameFocusService;
use crate::domain::{
    is_critical_process, AppConfig, GameEntry, GameSource, ProtonSummary, RunningApp, SortMode,
    ThemeMode, ViewMode,
};

mod theme;
mod widgets;
mod ui_library;
mod ui_focus;
mod ui_settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceFilter {
    All,
    Steam,
    Heroic,
    Lutris,
    Faugus,
}

impl SourceFilter {
    fn label(self) -> &'static str {
        match self {
            SourceFilter::All => "Tous",
            SourceFilter::Steam => "Steam",
            SourceFilter::Heroic => "Heroic",
            SourceFilter::Lutris => "Lutris",
            SourceFilter::Faugus => "Faugus",
        }
    }

    fn matches(self, source: GameSource) -> bool {
        match self {
            SourceFilter::All => true,
            SourceFilter::Steam => source == GameSource::Steam,
            SourceFilter::Heroic => source == GameSource::Heroic,
            SourceFilter::Lutris => source == GameSource::Lutris,
            SourceFilter::Faugus => source == GameSource::Faugus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Library,
    GamePage,
    Settings,
    FocusPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Display,
    Launch,
    Keep,
    Suspend,
    Running,
}

pub struct GameFocusApp {
    brand_logo: TextureHandle,
    service: Arc<GameFocusService>,
    games: Vec<GameEntry>,
    search: String,
    filter: SourceFilter,
    selected: Option<usize>,
    proton: Option<ProtonSummary>,
    recommended: String,
    steam_options_game: Option<String>,
    steam_options_value: Option<String>,
    steam_options_error: Option<String>,
    steam_options_checked_at: Instant,
    status: String,
    config: AppConfig,
    focus_active: bool,
    screen: Screen,
    settings_tab: SettingsTab,
    error: bool,
    counts: SourceCounts,
    covers: CoverCache,
    running: Vec<RunningApp>,
    running_ms: u128,
    focus_preview_list: Vec<RunningApp>,
    new_keep: String,
    new_suspend: String,
    proton_tools: Vec<ProtonTool>,
    running_filter: String,
    applied_dark_theme: bool,
    system_theme_checked_at: Instant,
    launch_after_focus: bool,
    desktop_commands: std::sync::mpsc::Receiver<DesktopCommand>,
    quitting: bool,
}

#[derive(Default, Clone, Copy)]
struct SourceCounts {
    steam: usize,
    heroic: usize,
    lutris: usize,
    faugus: usize,
}

impl GameFocusApp {
    pub(super) fn new(
        cc: &eframe::CreationContext<'_>,
        service: Arc<GameFocusService>,
        desktop_sender: DesktopCommandSender,
        desktop_commands: std::sync::mpsc::Receiver<DesktopCommand>,
    ) -> Self {
        desktop_sender.set_context(cc.egui_ctx.clone());
        let logo_image = image::load_from_memory(include_bytes!(
            "../../../../assets/branding/gamefocus-logo.png"
        ))
        .expect("Le logo intégré de GameFocus doit être une image valide")
        .into_rgba8();
        let logo_size = [logo_image.width() as usize, logo_image.height() as usize];
        let brand_logo = cc.egui_ctx.load_texture(
            "gamefocus-brand-logo",
            egui::ColorImage::from_rgba_unmultiplied(logo_size, logo_image.as_raw()),
            egui::TextureOptions::LINEAR,
        );
        let config = service.load_config();
        let dark_theme = match config.theme_mode {
            ThemeMode::System => theme::system_prefers_dark().unwrap_or(true),
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
        };
        theme::apply_theme(&cc.egui_ctx, dark_theme);
        let search = config.last_search.clone();
        let focus_active = service.is_focus_active();
        let steam_ids = service.steam_ids();
        let (running, running_ms) = list_running_apps();
        let mut app = Self {
            brand_logo,
            service,
            games: vec![],
            search,
            filter: SourceFilter::All,
            selected: None,
            proton: None,
            recommended: String::new(),
            steam_options_game: None,
            steam_options_value: None,
            steam_options_error: None,
            steam_options_checked_at: Instant::now(),
            status: format!("Scan fond : {} applis · {} ms", running.len(), running_ms),
            config,
            focus_active,
            screen: Screen::Library,
            settings_tab: SettingsTab::Display,
            error: false,
            counts: SourceCounts::default(),
            covers: CoverCache::with_resolver(steam_ids),
            running,
            running_ms,
            focus_preview_list: vec![],
            new_keep: String::new(),
            new_suspend: String::new(),
            proton_tools: list_proton_tools(),
            running_filter: String::new(),
            applied_dark_theme: dark_theme,
            system_theme_checked_at: Instant::now(),
            launch_after_focus: false,
            desktop_commands,
            quitting: false,
        };
        app.do_scan();
        app
    }

    fn refresh_recommended_with_tweaks(&mut self) {
        if self.recommended.is_empty() {
            return;
        }
        // Remet la base sans prefix tweaks : on re-fetch suggestion si possible
        if let Some(idx) = self.selected {
            if let Some(game) = self.games.get(idx).cloned() {
                if let Ok(s) = self.service.suggest_for_game(&game, false) {
                    self.recommended = self.config.launch.merge_into_command(&s.recommended);
                    return;
                }
            }
        }
        self.recommended = self.config.launch.merge_into_command(&self.recommended);
    }

    fn go_home(&mut self) {
        self.launch_after_focus = false;
        self.screen = Screen::Library;
        self.selected = None;
        self.recommended.clear();
        self.proton = None;
        self.status = format!("Bibliothèque — {} jeux", self.games.len());
        self.error = false;
    }

    fn refresh_running(&mut self) {
        let (running, ms) = list_running_apps();
        self.running_ms = ms;
        self.running = running;
        self.status = format!("Applis en fond : {} ({} ms)", self.running.len(), ms);
        self.error = false;
    }

    pub(super) fn do_scan(&mut self) {
        match self.service.scan_games() {
            Ok(mut games) => {
                match self.config.sort_mode {
                    SortMode::Name => {
                        games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                    SortMode::Launcher => {
                        games.sort_by(|a, b| {
                            a.source
                                .sort_key()
                                .cmp(&b.source.sort_key())
                                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                        });
                    }
                }
                self.counts = SourceCounts {
                    steam: games
                        .iter()
                        .filter(|g| g.source == GameSource::Steam)
                        .count(),
                    heroic: games
                        .iter()
                        .filter(|g| g.source == GameSource::Heroic)
                        .count(),
                    lutris: games
                        .iter()
                        .filter(|g| g.source == GameSource::Lutris)
                        .count(),
                    faugus: games
                        .iter()
                        .filter(|g| g.source == GameSource::Faugus)
                        .count(),
                };
                let n = games.len();
                self.games = games;
                // Resélectionner par nom si possible
                if let Some(idx) = self.selected {
                    if idx >= self.games.len() {
                        self.selected = None;
                    }
                }
                self.status = format!(
                    "{n} jeux — Steam {} · Heroic {} · Faugus {} · Lutris {}",
                    self.counts.steam, self.counts.heroic, self.counts.faugus, self.counts.lutris
                );
                self.error = false;
            }
            Err(e) => {
                self.status = format!("Erreur scan : {e}");
                self.error = true;
            }
        }
    }

    fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search.to_lowercase();
        self.games
            .iter()
            .enumerate()
            .filter(|(_, g)| {
                self.filter.matches(g.source)
                    && (q.is_empty() || g.name.to_lowercase().contains(&q))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn select_game(&mut self, idx: usize) {
        self.selected = Some(idx);
        // En grille → page dédiée ; en liste → panneau droit
        if self.config.view_mode == ViewMode::Grid {
            self.screen = Screen::GamePage;
        } else {
            self.screen = Screen::Library;
        }
        self.refresh_selected_launch_options(true);
        if let Some(game) = self.games.get(idx).cloned() {
            match self.service.suggest_for_game(&game, true) {
                Ok(s) => {
                    self.proton = s.proton;
                    self.recommended = self.config.launch.merge_into_command(&s.recommended);
                    self.status = format!("{} sélectionné", game.name);
                    self.error = false;
                }
                Err(e) => {
                    self.status = format!("Erreur : {e}");
                    self.error = true;
                }
            }
        }
    }

    fn refresh_selected_launch_options(&mut self, force: bool) {
        let Some(game) = self
            .selected
            .and_then(|index| self.games.get(index))
            .cloned()
        else {
            self.steam_options_game = None;
            self.steam_options_value = None;
            self.steam_options_error = None;
            return;
        };
        let key = widgets::game_config_key(&game);
        if !force
            && self.steam_options_game.as_deref() == Some(&key)
            && self.steam_options_checked_at.elapsed() < Duration::from_secs(2)
        {
            return;
        }
        self.steam_options_game = Some(key);
        self.steam_options_checked_at = Instant::now();
        match crate::adapters::launch_opts::read_launch_options(&game) {
            Ok(options) => {
                self.steam_options_value = options;
                self.steam_options_error = None;
            }
            Err(error) => {
                self.steam_options_value = None;
                self.steam_options_error = Some(error);
            }
        }
    }

    fn launch_selected(&mut self) {
        let Some(idx) = self.selected else {
            self.status = "Choisis un jeu d'abord.".into();
            self.error = true;
            return;
        };
        let game = self.games[idx].clone();
        let mut notes = Vec::new();
        // ------ Proton global ------
        if self.config.launch.apply_proton_on_launch && !self.config.launch.proton_tool.is_empty() {
            if let Some(tool) = self
                .proton_tools
                .iter()
                .find(|t| t.name == self.config.launch.proton_tool)
            {
                match crate::adapters::apply_proton::apply_proton_for_game(&game, tool) {
                    Ok(m) => notes.push(m),
                    Err(e) => notes.push(e),
                }
            }
        }
        match crate::adapters::launch::launch_game(&game) {
            Ok(msg) => {
                notes.push(msg);
                self.status = notes.join(" · ");
                self.error = false;
            }
            Err(e) => {
                self.status = e;
                self.error = true;
            }
        }
    }

    fn launch_with_focus(&mut self) {
        if !self.focus_active {
            if self.config.focus_preview {
                self.launch_after_focus = true;
                self.open_focus_tab();
                return;
            }
            self.confirm_focus();
            if self.error {
                return;
            }
        }
        self.launch_selected();
        if !self.error {
            self.status = format!("Focus ON → {}", self.status);
        }
    }

    fn show_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    fn quit_gamefocus(&mut self, ctx: &egui::Context) {
        if self.focus_active {
            self.stop_focus();
            if self.error {
                return;
            }
        }
        self.quitting = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn clear_options(&mut self) {
        let Some(idx) = self.selected else {
            self.status = "Choisis un jeu d'abord.".into();
            self.error = true;
            return;
        };
        let game = self.games[idx].clone();
        let key = widgets::game_config_key(&game);
        let managed = self
            .config
            .launch_options_managed
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let current = match crate::adapters::launch_opts::read_launch_options(&game) {
            Ok(current) => current,
            Err(error) => {
                self.status = format!("Impossible de vérifier les options actuelles : {error}");
                self.error = true;
                return;
            }
        };
        let present = match crate::adapters::launch_opts::present_managed_launch_options(
            current.as_deref(),
            &managed,
        ) {
            Ok(present) => present,
            Err(error) => {
                self.status = format!("Options existantes illisibles : {error}");
                self.error = true;
                return;
            }
        };
        if present.is_empty() {
            match self.set_launch_options_managed(&game, None) {
                Ok(()) => {
                    self.refresh_selected_launch_options(true);
                    self.status = "Aucune option GameFocus active dans le lanceur.".into();
                    self.error = false;
                }
                Err(error) => {
                    self.status = error;
                    self.error = true;
                }
            }
            return;
        }
        match crate::adapters::launch_opts::clear_launch_options(&game, &present) {
            Ok(msg) => match self.set_launch_options_managed(&game, None) {
                Ok(()) => {
                    self.refresh_selected_launch_options(true);
                    self.status = msg;
                    self.error = false;
                }
                Err(e) => {
                    let restore_command = format!("{} %command%", present.join(" "));
                    let restore =
                        crate::adapters::launch_opts::apply_launch_options(&game, &restore_command);
                    self.status = match restore {
                        Ok((restore_message, _)) => {
                            format!("{e} Les options ont été réappliquées : {restore_message}")
                        }
                        Err(restore_error) => format!(
                            "{e} Échec de restauration des options : {restore_error}. {msg}"
                        ),
                    };
                    self.error = true;
                }
            },
            Err(e) => {
                self.status = e;
                self.error = true;
            }
        }
    }

    fn activate_options(&mut self) {
        let Some(idx) = self.selected else {
            self.status = "Choisis un jeu d'abord.".into();
            self.error = true;
            return;
        };
        if self.recommended.is_empty() {
            self.status = "Commande de lancement indisponible.".into();
            self.error = true;
            return;
        }
        if self.recommended.trim() == "%command%" {
            self.status =
                "Aucune option supplémentaire n'est recommandée pour ce jeu; commande standard conservée."
                    .into();
            self.error = false;
            return;
        }
        let game = self.games[idx].clone();
        let current = match crate::adapters::launch_opts::read_launch_options(&game) {
            Ok(current) => current,
            Err(error) => {
                self.status = format!(
                    "Impossible de lire les options actuelles; aucune modification faite : {error}"
                );
                self.error = true;
                return;
            }
        };
        let key = widgets::game_config_key(&game);
        let previously_managed = self
            .config
            .launch_options_managed
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let retained_managed = match crate::adapters::launch_opts::present_managed_launch_options(
            current.as_deref(),
            &previously_managed,
        ) {
            Ok(retained) => retained,
            Err(error) => {
                self.status = format!("Options existantes illisibles : {error}");
                self.error = true;
                return;
            }
        };
        match crate::adapters::launch_opts::apply_launch_options(&game, &self.recommended) {
            Ok((message, newly_managed)) => {
                let mut managed = retained_managed;
                for token in &newly_managed {
                    if !managed.contains(token) {
                        managed.push(token.clone());
                    }
                }
                let owned = (!managed.is_empty()).then_some(managed.clone());
                match self.set_launch_options_managed(&game, owned) {
                    Ok(()) => {
                        self.refresh_selected_launch_options(true);
                        self.status = message;
                        self.error = false;
                    }
                    Err(error) => {
                        let rollback = crate::adapters::launch_opts::clear_launch_options(
                            &game,
                            &newly_managed,
                        );
                        self.status = match rollback {
                            Ok(rollback) => format!(
                                "{error} Les nouvelles options ont été annulées : {rollback}"
                            ),
                            Err(rollback_error) => format!(
                                "{error} Échec de l'annulation des options : {rollback_error}"
                            ),
                        };
                        self.error = true;
                    }
                }
            }
            Err(e) => {
                self.status = e;
                self.error = true;
            }
        }
    }

    fn set_launch_options_managed(
        &mut self,
        game: &GameEntry,
        managed: Option<Vec<String>>,
    ) -> Result<(), String> {
        let key = widgets::game_config_key(game);
        let previous = self.config.clone();
        self.config
            .launch_options_enabled
            .retain(|saved| saved != &key);
        if let Some(managed) = managed.filter(|managed| !managed.is_empty()) {
            self.config.launch_options_enabled.push(key.clone());
            self.config.launch_options_managed.insert(key, managed);
        } else {
            self.config.launch_options_managed.remove(&key);
        }
        if let Err(error) = self.service.save_config(&self.config) {
            self.config = previous;
            return Err(format!("options appliquées, état non enregistré : {error}"));
        }
        Ok(())
    }

    /// Ouvre l'onglet Focus (ne lance pas le mode).
    fn open_focus_tab(&mut self) {
        if !self.focus_active {
            let (list, ms) = preview_focus_targets(&self.config.focus);
            self.focus_preview_list = list;
            self.status = format!(
                "Onglet Focus — {} à geler ({} ms)",
                self.focus_preview_list.len(),
                ms
            );
        } else {
            self.status = "Focus actif — Arrêter depuis cette page.".into();
        }
        self.screen = Screen::FocusPreview;
        self.error = false;
    }

    fn stop_focus(&mut self) {
        self.launch_after_focus = false;
        match self.service.leave_focus() {
            Ok(s) => {
                self.focus_active = false;
                self.screen = Screen::FocusPreview;
                let (list, _) = preview_focus_targets(&self.config.focus);
                self.focus_preview_list = list;
                self.status = s.message;
                self.error = false;
            }
            Err(e) => {
                self.status = format!("Focus OFF : {e}");
                self.error = true;
            }
        }
    }

    fn confirm_focus(&mut self) {
        if let Err(e) = self.service.save_config(&self.config) {
            self.status = format!("Sauvegarde des réglages : {e}");
            self.error = true;
            return;
        }
        match self
            .service
            .enter_focus_with_rules(self.config.focus.clone())
        {
            Ok(s) => {
                self.focus_active = true;
                // Page Focus centrée (prop. 5) tant que c'est actif
                self.screen = Screen::FocusPreview;
                self.status = s.message;
                self.error = false;
                if self.launch_after_focus {
                    self.launch_after_focus = false;
                    self.launch_selected();
                }
            }
            Err(e) => {
                self.status = format!("Focus ON : {e}");
                self.error = true;
            }
        }
    }

    pub(super) fn save_settings(&mut self) {
        self.config.last_search = self.search.clone();
        match self.service.save_config(&self.config) {
            Ok(()) => {
                self.status = "Réglages enregistrés.".into();
                self.error = false;
            }
            Err(e) => {
                self.status = format!("Sauvegarde : {e}");
                self.error = true;
            }
        }
    }

    fn add_to_keep(&mut self, name: &str) {
        let n = name.trim().to_lowercase();
        if n.is_empty() {
            return;
        }
        if !self
            .config
            .focus
            .keep
            .iter()
            .any(|k| k.eq_ignore_ascii_case(&n))
        {
            self.config.focus.keep.push(n.clone());
        }
        self.config
            .focus
            .suspend
            .retain(|s| !s.eq_ignore_ascii_case(&n));
        self.save_settings();
        self.status = format!("Exception : « {n} » ne sera pas gelé.");
    }

    fn add_to_suspend(&mut self, name: &str) {
        let n = name.trim().to_lowercase();
        if n.is_empty() {
            return;
        }
        if is_listed(&n, &self.config.focus.keep) || is_critical_process(&n) {
            self.status = format!("« {n} » est protégé et ne sera pas gelé.");
            self.error = true;
            return;
        }
        if !self
            .config
            .focus
            .suspend
            .iter()
            .any(|k| k.eq_ignore_ascii_case(&n))
        {
            self.config.focus.suspend.push(n.clone());
        }
        self.save_settings();
        self.status = format!("« {n} » sera gelé en Focus.");
    }
}

impl eframe::App for GameFocusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(command) = self.desktop_commands.try_recv() {
            match command {
                DesktopCommand::Show => self.show_window(ctx),
            }
        }

        if ctx.input(|input| input.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.quit_gamefocus(ctx);
        }

        widgets::handle_window_resize_edges(ctx);
        let dark = match self.config.theme_mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => {
                if self.system_theme_checked_at.elapsed() >= Duration::from_secs(2) {
                    if let Some(dark) = theme::system_prefers_dark() {
                        self.applied_dark_theme = dark;
                    }
                    self.system_theme_checked_at = Instant::now();
                }
                self.applied_dark_theme
            }
        };
        if dark != self.applied_dark_theme || ctx.style().visuals.dark_mode != dark {
            self.applied_dark_theme = dark;
            theme::apply_theme(ctx, dark);
        }
        if self.config.theme_mode == ThemeMode::System {
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        if self.screen == Screen::GamePage
            && self
                .selected
                .and_then(|index| self.games.get(index))
                .is_some_and(|game| game.source == GameSource::Steam)
        {
            ctx.request_repaint_after(Duration::from_secs(2));
        }

        // Warm covers (limité)
        for idx in self.filtered_indices().into_iter().take(24) {
            if let Some(g) = self.games.get(idx).cloned() {
                let _ = self.covers.texture_for(ctx, &g);
            }
        }

        egui::TopBottomPanel::bottom("bar")
            .exact_height(72.0)
            .frame(
                Frame::none()
                    .fill(theme::bg())
                    .stroke(Stroke::new(1.0_f32, theme::line()))
                    .inner_margin(Margin::symmetric(14.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let home_on = matches!(self.screen, Screen::Library | Screen::GamePage);
                    let focus_on = self.screen == Screen::FocusPreview;
                    let set_on = self.screen == Screen::Settings;
                    if widgets::nav_text_btn(ui, "Accueil", home_on).clicked() {
                        self.go_home();
                    }
                    if widgets::nav_text_btn(ui, "Mode Focus", focus_on || self.focus_active).clicked() {
                        self.open_focus_tab();
                    }
                    if widgets::nav_text_btn(ui, "Réglages", set_on).clicked() {
                        self.screen = Screen::Settings;
                        self.refresh_running();
                    }
                });
                let color = if self.error {
                    theme::stop()
                } else if self.focus_active {
                    theme::ok()
                } else {
                    theme::muted()
                };
                ui.label(RichText::new(&self.status).size(12.0).color(color));
            });

        // ── Titre (sans chrome natif) ──
        egui::TopBottomPanel::top("titlebar")
            .exact_height(48.0)
            .frame(
                Frame::none()
                    .fill(theme::bg())
                    .inner_margin(Margin::symmetric(7.0, 0.0))
                    .stroke(Stroke::new(1.0_f32, theme::line())),
            )
            .show(ctx, |ui| {
                let height = ui.available_height();
                ui.set_min_height(height);
                ui.horizontal_centered(|ui| {
                    // Logo transparent (pas de fond noir — OK en thème clair)
                    let title_resp = ui.add(
                        egui::Image::new(&self.brand_logo)
                            .fit_to_exact_size(Vec2::new(174.0, 44.0))
                            .sense(Sense::click_and_drag()),
                    );
                    if title_resp.drag_started_by(egui::PointerButton::Primary) {
                        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    if title_resp.double_clicked() {
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }

                    let avail = ui.available_width();
                    let drag_w = (avail - 200.0).max(40.0);
                    let (drag_rect, drag_resp) = ui.allocate_exact_size(
                        Vec2::new(drag_w, height - 4.0),
                        Sense::click_and_drag(),
                    );
                    ui.painter()
                        .rect_filled(drag_rect, Rounding::ZERO, theme::bg());
                    if self.focus_active {
                        ui.painter().text(
                            drag_rect.left_center() + Vec2::new(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            "FOCUS ON",
                            FontId::proportional(13.0),
                            theme::ok(),
                        );
                    }
                    if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    if drag_resp.double_clicked() {
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::win_btn(ui, "×", theme::stop()).clicked() {
                            self.quit_gamefocus(ui.ctx());
                        }
                        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                        let max_lbl = if maximized { "❐" } else { "□" };
                        if widgets::win_btn(ui, max_lbl, theme::panel()).clicked() {
                            ui.ctx()
                                .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                        }
                        if widgets::win_btn(ui, "—", theme::panel()).clicked() {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    });
                });
            });

        egui::SidePanel::left("navigation")
            .exact_width(64.0)
            .resizable(false)
            .frame(
                Frame::none()
                    .fill(theme::bg())
                    .stroke(Stroke::new(1.0_f32, theme::line()))
                    .inner_margin(Margin::symmetric(8.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let home_on = matches!(self.screen, Screen::Library | Screen::GamePage);
                    let focus_on = self.screen == Screen::FocusPreview;
                    let set_on = self.screen == Screen::Settings;
                    if widgets::nav_btn(ui, "⌂", "Accueil", home_on).clicked() {
                        self.go_home();
                    }
                    ui.add_space(10.0);
                    if widgets::nav_btn(ui, "F", "Mode Focus", focus_on || self.focus_active).clicked() {
                        self.open_focus_tab();
                    }
                    ui.add_space(10.0);
                    if widgets::nav_btn(ui, "⚙", "Réglages", set_on).clicked() {
                        self.screen = Screen::Settings;
                        self.refresh_running();
                    }
                });
            });

        match self.screen {
            Screen::Settings => {
                egui::CentralPanel::default().show(ctx, |ui| self.ui_settings(ui));
            }
            Screen::FocusPreview => {
                egui::CentralPanel::default().show(ctx, |ui| self.ui_focus_page(ui));
            }
            Screen::GamePage => {
                // Pas de bouton « Retour » — Accueil / rail ⌂
                egui::CentralPanel::default().show(ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        self.ui_detail(ui, ctx);
                    });
                });
            }
            Screen::Library => {
                self.ui_library(ctx);
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.focus_active {
            if let Err(error) = self.service.leave_focus() {
                eprintln!("Restauration du mode Focus à la fermeture : {error}");
            }
            self.focus_active = false;
        }
        self.config.last_search = self.search.clone();
        let _ = self.service.save_config(&self.config);
    }
}
