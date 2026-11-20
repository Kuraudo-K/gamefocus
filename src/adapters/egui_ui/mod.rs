//! Interface egui (légère, Wayland/X11 via eframe/glow).

mod app;
mod desktop;

pub use app::GameFocusApp;

use std::sync::Arc;

use crate::application::GameFocusService;

pub fn run(service: Arc<GameFocusService>) -> Result<(), String> {
    let (commands, commands_rx) = desktop::DesktopCommandSender::channel();
    let instance = match desktop::acquire_single_instance(commands.clone())
        .map_err(|error| format!("Instance GameFocus : {error}"))?
    {
        desktop::InstanceStatus::Primary(guard) => guard,
        desktop::InstanceStatus::AlreadyRunning => return Ok(()),
    };
    // Pas d’icône de notification / systray : fermer la fenêtre quitte l’app.

    // Pas de décorations natives (barre GNOME/KDE différente de l’UI) :
    // une seule chrome dessinée par l’app — marche Wayland GNOME + KDE.
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("com.gamefocus.GameFocus")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([860.0, 560.0])
            .with_title("GameFocus")
            .with_icon({
                let icon = image::load_from_memory(include_bytes!(
                    "../../../assets/branding/gamefocus-icon.png"
                ))
                .expect("L’icône intégrée de GameFocus doit être une image valide")
                .into_rgba8();
                egui::IconData {
                    width: icon.width(),
                    height: icon.height(),
                    rgba: icon.into_raw(),
                }
            })
            .with_decorations(false)
            .with_resizable(true),
        ..Default::default()
    };
    let result = eframe::run_native(
        "GameFocus",
        opts,
        Box::new(move |cc| {
            Ok(Box::new(GameFocusApp::new(
                cc,
                service,
                commands.clone(),
                commands_rx,
            )))
        }),
    )
    .map_err(|error| error.to_string());
    drop(instance);
    result
}
