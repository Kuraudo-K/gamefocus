//! Règles et sélection pure du mode Focus (aucune I/O système).
//!
//! Rappel mémoire :
//! - SIGSTOP (geler) : coupe CPU/IO, **garde** RAM + VRAM → reversible (SIGCONT).
//! - drop_caches : libère surtout le *page cache* disque, pas la VRAM GPU.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FocusRules {
    /// Processus à geler / suspendre (noms courts : firefox, chrome, …).
    pub suspend: Vec<String>,
    /// Jamais toucher (discord, vesktop, pipewire, …).
    pub keep: Vec<String>,
    /// Services utilisateur systemd à stopper temporairement.
    pub stop_user_services: Vec<String>,
    /// Geler (SIGSTOP) les fonds d'écran animés connus (waywallen…).
    pub pause_wallpapers: bool,
    /// Mode console : gèle aussi bureautique / IDE / sync / indexeurs (+ liste étendue).
    #[serde(default = "default_true")]
    pub aggressive: bool,
    /// `sync` + tentative `drop_caches` (page cache RAM — pas la VRAM).
    #[serde(default)]
    pub drop_page_cache: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FocusRules {
    fn default() -> Self {
        Self {
            suspend: default_suspend_list(),
            keep: default_keep_list(),
            stop_user_services: default_stop_services(),
            pause_wallpapers: true,
            aggressive: true,
            drop_page_cache: false,
        }
    }
}

fn default_keep_list() -> Vec<String> {
    vec![
        "discord".into(),
        "vesktop".into(),
        "Discord".into(),
        "steam".into(),
        "steamwebhelper".into(),
        "steam-runtime".into(),
        "pipewire".into(),
        "wireplumber".into(),
        "pulseaudio".into(),
        "gamefocus".into(),
        "gamemoded".into(),
        "gamescope".into(),
        "mangohud".into(),
        "proton".into(),
        "wine".into(),
        "wineserver".into(),
        "umu-run".into(),
        "heroic".into(),
        "lutris".into(),
        "faugus".into(),
        "faugus-launcher".into(),
    ]
}

fn default_suspend_list() -> Vec<String> {
    let mut v = vec![
        // Navigateurs
        "firefox".into(),
        "firefox-bin".into(),
        "firefox-esr".into(),
        "chrome".into(),
        "chromium".into(),
        "brave".into(),
        "brave-browser".into(),
        "msedge".into(),
        "vivaldi".into(),
        "vivaldi-bin".into(),
        "opera".into(),
        "librewolf".into(),
        "waterfox".into(),
        "epiphany".into(),
        "gnome-web".into(),
        "tor-browser".into(),
        "floorp".into(),
        // Messagerie (hors Discord → keep)
        "slack".into(),
        "telegram-desktop".into(),
        "telegramdesktop".into(),
        "element-desktop".into(),
        "signal-desktop".into(),
        "zoom".into(),
        "zoom-client".into(),
        "teams".into(),
        "teams-for-linux".into(),
        "skypeforlinux".into(),
        "whatsapp-desktop".into(),
        "whatsapp-for-linux".into(),
        // Mail / bureautique
        "thunderbird".into(),
        "evolution".into(),
        "geary".into(),
        "libreoffice".into(),
        "soffice.bin".into(),
        "onlyoffice".into(),
        // Sync / cloud
        "dropbox".into(),
        "megasync".into(),
        "nextcloud".into(),
        "kdrive".into(),
        "onedrive".into(),
        "syncthing".into(),
        "rclone".into(),
        "insync".into(),
        // Indexeurs
        "baloo_file".into(),
        "baloo_file_extractor".into(),
        "tracker-miner-fs-3".into(),
        "tracker3".into(),
        "localsearch".into(),
        "localsearch-3".into(),
        // Divers lourds
        "sable".into(),
        "spotify".into(),
        "rhythmbox".into(),
        "obs".into(),
        "obs-studio".into(),
        "qbittorrent".into(),
        "transmission-gtk".into(),
        "transmission-qt".into(),
        "deluge".into(),
    ];
    v.sort();
    v.dedup();
    v
}

fn default_stop_services() -> Vec<String> {
    vec![
        "tracker-miner-fs-3.service".into(),
        "localsearch-3.service".into(),
        "evolution-addressbook-factory.service".into(),
        "evolution-calendar-factory.service".into(),
        "evolution-source-registry.service".into(),
    ]
}

/// Cibles supplémentaires mode console (bureautique / IDE / explorateurs…).
pub fn aggressive_extra_targets() -> &'static [&'static str] {
    &[
        "nautilus",
        "dolphin",
        "thunar",
        "nemo",
        "pcmanfm",
        "caja",
        "gedit",
        "gnome-text-editor",
        "kate",
        "kwrite",
        "code",
        "code-oss",
        "codium",
        "vscodium",
        "cursor",
        "atom",
        "sublime_text",
        "jetbrains-toolbox",
        "idea",
        "phpstorm",
        "webstorm",
        "pycharm",
        "clion",
        "rustrover",
        "gnome-software",
        "plasma-discover",
        "snap-store",
        "update-manager",
        "packagekitd",
        "apk-polkit-server",
        "fwupd",
        "deja-dup",
        "backintime",
        "timeshift",
        "flatpak-system-helper",
        // KDE (hors shell critique)
        "akonadi_control",
        "akonadiserver",
        "kdeconnectd",
        "kdeconnect-indicator",
        "kalendarac",
        "kmail",
        "kontact",
        "discover",
        "plasma-browser-integration-host",
        "fileindexing",
    ]
}

/// Jamais geler — DE, audio, session, GPU stack critique.
pub fn critical_never_touch() -> &'static [&'static str] {
    &[
        "systemd",
        "init",
        "dbus-daemon",
        "dbus-broker",
        "gnome-shell",
        ".gnome-shell-wr",
        "gnome-shell-wrapper",
        "gnome-session",
        "gnome-session-binary",
        "gsd-",
        "gnome-settings-daemon",
        "kwin",
        "kwin_wayland",
        "kwin_x11",
        "plasmashell",
        "plasma_session",
        "plasmalogin",
        "plasmalogin-helper",
        "startplasma-wayland",
        "startplasma-x11",
        "plasma-dbus-run-session-if-needed",
        "ksmserver",
        "kded",
        "kded5",
        "kded6",
        "kglobalaccel",
        "kglobalaccel5",
        "kglobalaccel6",
        "kaccess",
        "kscreenlocker_greet",
        "polkit-kde-authentication-agent-1",
        "org_kde_powerdevil",
        "powerdevil",
        "xembedsniproxy",
        "xdg-desktop-portal-kde",
        "kactivitymanagerd",
        "kwin_wayland_wrapper",
        ".kwin_wayland-w",
        ".plasmashell-wr",
        "xorg",
        "xwayland",
        "wayland",
        "hyprland",
        "sway",
        "river",
        "mutter",
        "cinnamon",
        "muffin",
        "xfce4-session",
        "xfwm4",
        "lxqt-session",
        "openbox",
        "gdm",
        "gdm-session-worker",
        "sddm",
        "lightdm",
        "login",
        "plasmalogin",
        "plasmalogin-helper",
        "sshd",
        "networkmanager",
        "nm-applet",
        "wpa_supplicant",
        "iwd",
        "pipewire",
        "pipewire-pulse",
        "wireplumber",
        "pulseaudio",
        "rtkit-daemon",
        "nvidia-persistenced",
        "nvidia-powerd",
        "nvidia-modprobe",
        "nvidia-sleep",
        "nvidia-resume",
        "xdg-desktop-portal",
        "xdg-document-portal",
        "xdg-permission-store",
        "xdg-desktop-portal-gnome",
        "xdg-desktop-portal-gtk",
        "xdg-desktop-portal-kde",
        "ibus-daemon",
        "fcitx",
        "fcitx5",
        "at-spi-bus-launcher",
        "at-spi2-registryd",
        "polkitd",
        "polkit-gnome",
        "gnome-keyring",
        "kwalletd",
        "ssh-agent",
        "gpg-agent",
        "gamefocus",
        "gamemoded",
        "gamescope",
        "mangohud",
        "steamwebhelper",
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusAction {
    Enter,
    Leave,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FocusSnapshot {
    pub suspended_pids: Vec<u32>,
    pub stopped_services: Vec<String>,
    pub paused_wallpapers: Vec<String>,
    #[serde(default)]
    pub paused_wallpaper_processes: Vec<PausedProcessIdentity>,
    #[serde(default)]
    pub wallpaper_restart_commands: Vec<WallpaperRestartCommand>,
    #[serde(default)]
    pub focus_started_at_unix_secs: Option<u64>,
    /// PIDs stoppés (TERM) pour VRAM — ne pas SIGCONT.
    #[serde(default)]
    pub vram_stopped_pids: Vec<u32>,
    /// PIDs dont l’arrêt propre a été demandé pour rendre la RAM.
    #[serde(default)]
    pub ram_stopped_pids: Vec<u32>,
    pub active: bool,
    #[serde(default)]
    pub dropped_page_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PausedProcessIdentity {
    pub pid: u32,
    pub start_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WallpaperRestartCommand {
    pub pid: u32,
    pub start_time: u64,
    pub executable: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub stopped_for_memory: bool,
}

impl Default for FocusSnapshot {
    fn default() -> Self {
        Self {
            suspended_pids: vec![],
            stopped_services: vec![],
            paused_wallpapers: vec![],
            paused_wallpaper_processes: vec![],
            wallpaper_restart_commands: vec![],
            focus_started_at_unix_secs: None,
            vram_stopped_pids: vec![],
            ram_stopped_pids: vec![],
            active: false,
            dropped_page_cache: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FocusSession {
    pub action: FocusAction,
    pub suspended: usize,
    pub services_stopped: usize,
    pub wallpapers_paused: usize,
    pub message: String,
}

/// Noms de process wallpaper connus (règle métier).
pub fn wallpaper_process_names() -> &'static [&'static str] {
    &[
        "wallpaperengine",
        "linux-wallpaperengine",
        "mpvpaper",
        "waypaper",
        "waywallen",
        "hidamari",
        "komorebi",
        "variety",
        "superpaper",
    ]
}

pub(crate) fn normalize_set(items: &[String]) -> HashSet<String> {
    items.iter().map(|s| s.to_lowercase()).collect()
}

/// Correspondance process ↔ motif : égalité, préfixe `motif-` / `motif.`, pas de
/// `contains` nu (sinon trop de faux positifs sur Nix / Electron).
pub fn process_name_matches(name: &str, pattern: &str) -> bool {
    let n = name.to_lowercase();
    let p = pattern.to_lowercase();
    if p.is_empty() {
        return false;
    }
    // Motif se terminant par `-` → préfixe (ex. gsd-)
    if p.ends_with('-') {
        return n.starts_with(&p);
    }
    n == p
        || n.starts_with(&format!("{p}-"))
        || n.starts_with(&format!("{p}."))
        || n.ends_with(&format!("-{p}"))
        || n.ends_with(&format!(".{p}"))
}

pub(crate) fn matches_any(name: &str, set: &HashSet<String>) -> bool {
    set.iter().any(|k| process_name_matches(name, k))
}

pub fn is_critical_process(name: &str) -> bool {
    let n = name.to_lowercase();
    if critical_never_touch()
        .iter()
        .any(|c| process_name_matches(&n, c))
    {
        return true;
    }
    // Stack jeu / Proton : ne jamais geler pendant une session
    n.contains("steam")
        || n.starts_with("proton")
        || n.contains("wine")
        || n.contains("gamescope")
        || n.contains("mangohud")
}

/// Cibles effectives (suspend + extras agressifs).
pub fn effective_suspend_set(rules: &FocusRules) -> HashSet<String> {
    let mut set = normalize_set(&rules.suspend);
    if rules.aggressive {
        for t in aggressive_extra_targets() {
            set.insert((*t).to_lowercase());
        }
    }
    set
}

/// Sélectionne les PIDs qui seraient gelés (pure, pour tests / dry-run).
pub fn select_focus_targets(process_names: &[(u32, &str)], rules: &FocusRules) -> Vec<u32> {
    let keep = normalize_set(&rules.keep);
    let targets = effective_suspend_set(rules);
    let mut out = Vec::new();
    for (pid, name) in process_names {
        let n = name.to_lowercase();
        if is_critical_process(&n) || matches_any(&n, &keep) || n == "gamefocus" {
            continue;
        }
        if matches_any(&n, &targets) {
            out.push(*pid);
        } else if rules.pause_wallpapers {
            for wp in wallpaper_process_names() {
                if n.contains(wp) {
                    out.push(*pid);
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_discord_suspends_firefox_and_wallpaper() {
        let rules = FocusRules::default();
        let procs = [
            (1u32, "systemd"),
            (10, "discord"),
            (11, "vesktop"),
            (20, "firefox"),
            (21, "chrome"),
            (30, "linux-wallpaperengine"),
            (40, "steam"),
            (50, "gamefocus"),
        ];
        let targets = select_focus_targets(&procs, &rules);
        assert!(targets.contains(&20));
        assert!(targets.contains(&21));
        assert!(targets.contains(&30));
        assert!(!targets.contains(&10));
        assert!(!targets.contains(&11));
        assert!(!targets.contains(&40));
        assert!(!targets.contains(&50));
        assert!(!targets.contains(&1));
    }

    #[test]
    fn aggressive_freezes_nautilus() {
        let rules = FocusRules::default();
        assert!(rules.aggressive);
        let procs = [(70u32, "nautilus"), (71, "code")];
        let targets = select_focus_targets(&procs, &rules);
        assert!(targets.contains(&70));
        assert!(targets.contains(&71));
    }

    #[test]
    fn ollama_is_frozen_when_listed_in_suspend() {
        let mut rules = FocusRules::default();
        rules.suspend.push("ollama".into());
        let procs = [(80u32, "ollama"), (20, "firefox")];
        assert!(select_focus_targets(&procs, &rules).contains(&80));
        assert!(select_focus_targets(&procs, &rules).contains(&20));
    }

    #[test]
    fn kde_shell_stays_critical() {
        assert!(is_critical_process("plasmashell"));
        assert!(is_critical_process(".plasmashell-wr"));
        assert!(is_critical_process("kwin_wayland"));
        assert!(is_critical_process("org_kde_powerdevil"));
        assert!(!is_critical_process("dolphin"));
        assert!(!is_critical_process("baloo_file"));
    }

    #[test]
    fn console_mode_extras_are_not_targets_when_disabled() {
        let mut rules = FocusRules::default();
        rules.aggressive = false;
        rules.suspend.clear();
        let procs = [(70u32, "nautilus"), (71, "code")];
        assert!(select_focus_targets(&procs, &rules).is_empty());
    }

    #[test]
    fn default_keeps_discord() {
        let rules = FocusRules::default();
        assert!(rules.keep.iter().any(|k| k.eq_ignore_ascii_case("discord")));
        assert!(rules.suspend.iter().any(|s| s == "firefox"));
    }

    #[test]
    fn snapshot_serde() {
        let snap = FocusSnapshot {
            suspended_pids: vec![1, 2, 3],
            stopped_services: vec!["foo.service".into()],
            paused_wallpapers: vec!["waypaper".into()],
            paused_wallpaper_processes: vec![PausedProcessIdentity {
                pid: 30,
                start_time: 456,
            }],
            wallpaper_restart_commands: vec![WallpaperRestartCommand {
                pid: 90,
                start_time: 789,
                executable: "/usr/bin/example-app".into(),
                args: vec!["--restore".into()],
                working_dir: Some("/tmp".into()),
                stopped_for_memory: true,
            }],
            focus_started_at_unix_secs: Some(123),
            vram_stopped_pids: vec![9],
            ram_stopped_pids: vec![10],
            active: true,
            dropped_page_cache: true,
        };
        let raw = serde_json::to_string(&snap).unwrap();
        let back: FocusSnapshot = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, snap);
        assert_eq!(
            back.wallpaper_restart_commands[0].executable,
            "/usr/bin/example-app"
        );
    }

    #[test]
    fn snapshot_accepts_older_persisted_shape() {
        let raw = r#"{"suspended_pids":[42],"stopped_services":[],"paused_wallpapers":["linux-wallpaperengine"],"vram_stopped_pids":[],"active":true,"dropped_page_cache":false}"#;
        let snapshot: FocusSnapshot = serde_json::from_str(raw).unwrap();
        assert_eq!(snapshot.paused_wallpaper_processes, Vec::new());
        assert_eq!(snapshot.focus_started_at_unix_secs, None);
    }

    #[test]
    fn pause_wallpapers_can_be_disabled() {
        let mut rules = FocusRules::default();
        rules.pause_wallpapers = false;
        let procs = [(20u32, "firefox"), (30, "waypaper")];
        let targets = select_focus_targets(&procs, &rules);
        assert_eq!(targets, vec![20]);
    }

    #[test]
    fn keep_overrides_suspend_name_overlap() {
        let mut rules = FocusRules::default();
        rules.keep.push("firefox".into());
        let procs = [(20u32, "firefox"), (21, "chrome")];
        let targets = select_focus_targets(&procs, &rules);
        assert!(!targets.contains(&20));
        assert!(targets.contains(&21));
    }

    #[test]
    fn substring_match_on_process_name() {
        let rules = FocusRules::default();
        let procs = [(99u32, "firefox-nightly")];
        let targets = select_focus_targets(&procs, &rules);
        assert_eq!(targets, vec![99]);
    }

    #[test]
    fn audio_stack_never_selected() {
        let rules = FocusRules::default();
        let procs = [(1u32, "pipewire"), (2, "wireplumber"), (3, "pulseaudio")];
        assert!(select_focus_targets(&procs, &rules).is_empty());
    }
}
