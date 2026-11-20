//! Mode Focus via sysinfo (SIGSTOP/SIGCONT) + systemctl user.
//!
//! - **Geler** = SIGSTOP (CPU off, RAM/VRAM gardées) → SIGCONT à l’arrêt.
//! - **stop_for_vram / stop_for_ram** = TERM, puis relance à la sortie du Focus.
//! - **drop_page_cache** = sync + tentative drop_caches (page cache, pas GPU).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{Pid, Process, ProcessStatus, ProcessesToUpdate, Signal, System, ThreadKind};

use crate::adapters::config_json::JsonConfigStore;
use crate::adapters::process_list::{
    current_uid, list_vram_by_pid, process_protection_reason, MIN_RAM_RELEASE_BYTES,
};
use crate::domain::focus::{matches_any, normalize_set};
use crate::domain::{
    is_critical_process, select_focus_targets, select_ram_stop_targets, select_vram_stop_targets,
    wallpaper_process_names, FocusAction, FocusRules, FocusSession, FocusSnapshot,
    PausedProcessIdentity, WallpaperRestartCommand,
};
use crate::ports::FocusController;

const WALLPAPER_RESTART_AFTER: Duration = Duration::from_secs(5 * 60);

fn should_restart_wallpapers(elapsed: Duration) -> bool {
    elapsed >= WALLPAPER_RESTART_AFTER
}

fn should_launch_wallpaper_restart(
    stopped_for_memory: bool,
    same_process_running: bool,
    refresh_paused_wallpapers: bool,
    is_video_wallpaper: bool,
) -> bool {
    if stopped_for_memory {
        !same_process_running
    } else {
        is_video_wallpaper || (refresh_paused_wallpapers && same_process_running)
    }
}

fn is_video_wallpaper_command(restart: &WallpaperRestartCommand) -> bool {
    let executable = restart.executable.to_lowercase();
    let command = std::iter::once(executable.as_str())
        .chain(restart.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    [
        "wallpaperengine",
        "linux-wallpaperengine",
        "mpvpaper",
        "waywallen-video",
        "waywallen-wescene",
        "hidamari",
        "komorebi",
    ]
    .iter()
    .any(|engine| command.contains(engine))
}

fn wait_for_process_exit(
    sys: &mut System,
    restart: &WallpaperRestartCommand,
) -> Result<(), String> {
    let pid = Pid::from_u32(restart.pid);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        std::thread::sleep(Duration::from_millis(100));
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let still_running = sys.process(pid).is_some_and(|process| {
            process.start_time() == restart.start_time && process.status() != ProcessStatus::Zombie
        });
        if !still_running {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{} ne s’est pas arrêté; relance évitée pour ne pas créer un doublon",
                restart.executable
            ));
        }
    }
}

fn stop_process_before_restart(
    sys: &mut System,
    restart: &WallpaperRestartCommand,
) -> Result<(), String> {
    let pid = Pid::from_u32(restart.pid);
    let Some(process) = sys.process(pid) else {
        return Ok(());
    };
    if process.start_time() != restart.start_time {
        return Ok(());
    }
    if !process.kill_with(Signal::Term).unwrap_or(false) {
        return Err(format!("{} n’a pas pu être arrêté", restart.executable));
    }
    wait_for_process_exit(sys, restart)
}

fn restart_args(cmd: &[std::ffi::OsString], executable: &std::path::Path) -> Vec<String> {
    let skip_program_name = cmd.first().is_some_and(|first| {
        first == executable.as_os_str()
            || PathBuf::from(first.as_os_str()).file_name() == executable.file_name()
    });
    cmd.iter()
        .skip(if skip_program_name { 1 } else { 0 })
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

pub struct SysFocusController {
    rules: FocusRules,
    snapshot: FocusSnapshot,
    dry_run: bool,
    /// Process fictifs pour dry-run / tests (sinon scan sysinfo).
    fake_processes: Option<Vec<(u32, String)>>,
    focus_started_at: Option<Instant>,
    wallpaper_restarts: Vec<WallpaperRestartCommand>,
}

impl SysFocusController {
    pub fn new(rules: FocusRules) -> Self {
        Self {
            rules,
            snapshot: FocusSnapshot::default(),
            dry_run: false,
            fake_processes: None,
            focus_started_at: None,
            wallpaper_restarts: Vec::new(),
        }
    }

    pub fn with_fake_processes(mut self, procs: Vec<(u32, String)>) -> Self {
        self.fake_processes = Some(procs);
        self
    }

    pub fn dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }

    pub fn rules(&self) -> &FocusRules {
        &self.rules
    }

    pub fn set_rules(&mut self, rules: FocusRules) {
        self.rules = rules;
    }

    pub fn snapshot_path() -> Result<PathBuf, String> {
        Ok(JsonConfigStore::config_dir()?.join("focus-session.json"))
    }

    pub fn load_persisted() -> Result<Option<FocusSnapshot>, String> {
        let path = Self::snapshot_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())
    }

    pub fn restore_persisted(&mut self) -> Result<(), String> {
        if let Some(snapshot) = Self::load_persisted()? {
            self.restore_snapshot(snapshot);
        }
        Ok(())
    }

    fn restore_snapshot(&mut self, snapshot: FocusSnapshot) {
        self.focus_started_at = snapshot.focus_started_at_unix_secs.and_then(|started| {
            let elapsed = unix_time_secs().saturating_sub(started);
            Instant::now().checked_sub(Duration::from_secs(elapsed))
        });
        self.snapshot = snapshot;
        self.wallpaper_restarts = self.snapshot.wallpaper_restart_commands.clone();
        if self.snapshot.active && !self.dry_run && self.fake_processes.is_none() {
            let mut sys = System::new();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let pids = self.snapshot.suspended_pids.clone();
            for pid in pids {
                let expected_start_time = self
                    .snapshot
                    .paused_wallpaper_processes
                    .iter()
                    .find(|identity| identity.pid == pid)
                    .map(|identity| identity.start_time);
                if let Some(process) = sys.process(Pid::from_u32(pid)) {
                    if Some(process.start_time()) == expected_start_time
                        && is_wallpaper_process(process, &normalize_set(&self.rules.keep))
                    {
                        self.track_process_restart(pid, process, false);
                    }
                }
            }
        }
    }

    fn track_process_restart(&mut self, pid: u32, process: &Process, stopped_for_memory: bool) {
        if self.wallpaper_restarts.iter().any(|entry| entry.pid == pid) {
            return;
        }
        let Some(executable) = process.exe() else {
            return;
        };
        let args = restart_args(process.cmd(), executable);
        self.wallpaper_restarts.push(WallpaperRestartCommand {
            pid,
            start_time: process.start_time(),
            executable: executable.to_string_lossy().into_owned(),
            args,
            working_dir: process
                .cwd()
                .map(|path| path.to_string_lossy().into_owned()),
            stopped_for_memory,
        });
    }

    fn persist_snapshot(&self) -> Result<(), String> {
        if self.dry_run {
            return Ok(());
        }
        let path = Self::snapshot_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(
            &path,
            serde_json::to_string_pretty(&self.snapshot).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("écriture {}: {e}", path.display()))
    }

    fn clear_persisted(&self) -> Result<(), String> {
        if self.dry_run {
            return Ok(());
        }
        let path = Self::snapshot_path()?;
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

impl FocusController for SysFocusController {
    fn enter(&mut self) -> Result<FocusSession, String> {
        let already = self.snapshot.active;
        let mut suspended = if already {
            self.snapshot.suspended_pids.clone()
        } else {
            Vec::new()
        };
        let keep = normalize_set(&self.rules.keep);
        let targets = crate::domain::focus::effective_suspend_set(&self.rules);
        let vram_stop = normalize_set(&self.rules.stop_for_vram);
        let ram_stop = normalize_set(&self.rules.stop_for_ram);
        let mut wallpapers = Vec::new();
        let mut skipped_memory_targets = 0usize;
        let mut vram_killed = if already {
            self.snapshot.vram_stopped_pids.clone()
        } else {
            Vec::new()
        };
        let mut ram_stopped = if already {
            self.snapshot.ram_stopped_pids.clone()
        } else {
            Vec::new()
        };
        let mut dropped_cache = already && self.snapshot.dropped_page_cache;

        if let Some(ref fake) = self.fake_processes {
            if !already {
                let refs: Vec<(u32, &str)> = fake.iter().map(|(p, n)| (*p, n.as_str())).collect();
                suspended = select_focus_targets(&refs, &self.rules);
                vram_killed = select_vram_stop_targets(&refs, &self.rules);
                ram_stopped = select_ram_stop_targets(&refs, &self.rules);
            }
            if self.rules.pause_wallpapers {
                for (_pid, name) in fake {
                    let n = name.to_lowercase();
                    for wp in wallpaper_process_names() {
                        if n.contains(wp) {
                            wallpapers.push(n.clone());
                            break;
                        }
                    }
                }
            }
        } else {
            let mut sys = System::new();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let vram_by_pid = list_vram_by_pid();

            let debug = std::env::var("GAMEFOCUS_DEBUG").ok().as_deref() == Some("1");
            let mut debug_samples = 0usize;

            // parent_pid → children (pour geler l’arbre)
            let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
            let session_uid = current_uid();
            for (pid, proc_) in sys.processes() {
                if let Some(pp) = proc_.parent() {
                    children.entry(pp.as_u32()).or_default().push(pid.as_u32());
                }
            }

            if !already {
                for (pid, proc_) in sys.processes() {
                    if matches!(
                        proc_.thread_kind(),
                        Some(ThreadKind::Userland | ThreadKind::Kernel)
                    ) {
                        continue;
                    }

                    let name = proc_.name().to_string_lossy().to_lowercase();
                    let exe = proc_
                        .exe()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let name_base = std::path::Path::new(name.as_str())
                        .file_name()
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_else(|| name.clone());

                    if is_critical_process(&name_base)
                        || is_critical_process(&exe)
                        || matches_any(&name_base, &keep)
                        || matches_any(&exe, &keep)
                    {
                        continue;
                    }
                    if name_base == "gamefocus" || name_base.starts_with('[') {
                        continue;
                    }

                    let pid_u = pid.as_u32();

                    let stop_for_vram =
                        matches_any(&name_base, &vram_stop) || matches_any(&exe, &vram_stop);
                    let stop_for_ram =
                        matches_any(&name_base, &ram_stop) || matches_any(&exe, &ram_stop);
                    if stop_for_vram || stop_for_ram {
                        let has_vram = vram_by_pid.get(&pid_u).copied().unwrap_or_default() > 0;
                        let has_ram = proc_.memory() >= MIN_RAM_RELEASE_BYTES;
                        let release_vram = stop_for_vram && has_vram;
                        let release_ram = stop_for_ram && has_ram;
                        let can_term = release_vram || release_ram;
                        let protected = process_protection_reason(
                            pid_u,
                            &name_base,
                            proc_.exe(),
                            session_uid,
                        )
                        .is_some()
                            || proc_.exe().is_none();

                        if can_term && !protected {
                            if self.dry_run {
                                if release_vram {
                                    vram_killed.push(pid_u);
                                }
                                if release_ram {
                                    ram_stopped.push(pid_u);
                                }
                                continue;
                            }
                            if proc_.kill_with(Signal::Term).unwrap_or(false) {
                                self.track_process_restart(pid_u, proc_, true);
                                if release_vram {
                                    vram_killed.push(pid_u);
                                }
                                if release_ram {
                                    ram_stopped.push(pid_u);
                                }
                                continue;
                            }
                            skipped_memory_targets += 1;
                        } else {
                            skipped_memory_targets += 1;
                        }

                        // Secours : geler (SIGSTOP) si TERM impossible (pas de VRAM
                        // mesurée, RAM sous seuil, protégé, ou kill échoué).
                        if self.dry_run {
                            suspended.push(pid_u);
                            continue;
                        }
                        if proc_.kill_with(Signal::Stop).unwrap_or(false) {
                            suspended.push(pid_u);
                        }
                        continue;
                    }

                    // ------ Gel classique ------
                    if !(matches_any(&name_base, &targets) || matches_any(&exe, &targets)) {
                        continue;
                    }

                    if debug && debug_samples < 40 {
                        eprintln!(
                            "[gamefocus debug] suspend pid={pid_u} name={name_base:?} exe={exe:?}"
                        );
                        debug_samples += 1;
                    }
                    if self.dry_run {
                        suspended.push(pid_u);
                        continue;
                    }
                    if proc_.kill_with(Signal::Stop).unwrap_or(false) {
                        suspended.push(pid_u);
                    }
                }

                // Geler descendants des PIDs déjà gelés (renderers navigateur, etc.)
                if !self.dry_run {
                    let roots: Vec<u32> = suspended.clone();
                    for root in roots {
                        freeze_descendants(
                            &sys,
                            &children,
                            root,
                            &mut suspended,
                            &keep,
                            &vram_stop,
                            &ram_stop,
                        );
                    }
                }
            }

            // Wallpapers : suspendre l’animation et mémoriser comment la relancer.
            if self.rules.pause_wallpapers {
                for (pid, proc_) in sys.processes() {
                    let name = proc_.name().to_string_lossy().to_lowercase();
                    if !is_wallpaper_process(proc_, &keep) {
                        continue;
                    }
                    let pid_u = pid.as_u32();
                    if vram_killed.contains(&pid_u) || ram_stopped.contains(&pid_u) {
                        continue;
                    }
                    if suspended.contains(&pid_u) {
                        if !wallpapers.iter().any(|w| w == &name) {
                            wallpapers.push(name);
                        }
                        self.track_process_restart(pid_u, proc_, false);
                        continue;
                    }
                    if self.dry_run {
                        suspended.push(pid_u);
                        wallpapers.push(name);
                        continue;
                    }
                    if proc_.kill_with(Signal::Stop).unwrap_or(false) {
                        suspended.push(pid_u);
                        wallpapers.push(name);
                        self.track_process_restart(pid_u, proc_, false);
                    }
                }
            }
        }

        let mut stopped_services = if already {
            self.snapshot.stopped_services.clone()
        } else {
            Vec::new()
        };
        if !already {
            for svc in &self.rules.stop_user_services {
                if self.dry_run {
                    stopped_services.push(svc.clone());
                    continue;
                }
                let status = Command::new("systemctl")
                    .args(["--user", "stop", svc])
                    .status();
                if status.map(|s| s.success()).unwrap_or(false) {
                    stopped_services.push(svc.clone());
                }
            }
        }

        // Page cache RAM (pas VRAM) — best effort, sans pkexec bloquant
        if self.rules.drop_page_cache && !already && !self.dry_run {
            dropped_cache = try_drop_page_cache();
        }

        suspended.sort_unstable();
        suspended.dedup();
        vram_killed.sort_unstable();
        vram_killed.dedup();
        ram_stopped.sort_unstable();
        ram_stopped.dedup();

        let focus_started_at_unix_secs = if already {
            self.snapshot
                .focus_started_at_unix_secs
                .or_else(|| Some(unix_time_secs()))
        } else {
            Some(unix_time_secs())
        };
        self.snapshot = FocusSnapshot {
            suspended_pids: suspended.clone(),
            stopped_services: stopped_services.clone(),
            paused_wallpapers: wallpapers.clone(),
            paused_wallpaper_processes: self
                .wallpaper_restarts
                .iter()
                .map(|restart| PausedProcessIdentity {
                    pid: restart.pid,
                    start_time: restart.start_time,
                })
                .collect(),
            wallpaper_restart_commands: self.wallpaper_restarts.clone(),
            focus_started_at_unix_secs,
            vram_stopped_pids: vram_killed.clone(),
            ram_stopped_pids: ram_stopped.clone(),
            active: true,
            dropped_page_cache: dropped_cache,
        };
        if !already {
            self.focus_started_at = Some(Instant::now());
        }
        self.persist_snapshot()?;

        let stopped_processes: HashSet<_> =
            vram_killed.iter().chain(&ram_stopped).copied().collect();
        let mut message = format!(
            "Focus ON — {} gelés · {} arrêt(s) demandé(s) · {} service(s) arrêté(s)",
            suspended.len(),
            stopped_processes.len(),
            stopped_services.len()
        );
        if skipped_memory_targets > 0 {
            message.push_str(&format!(" · {skipped_memory_targets} cible(s) ignorée(s)"));
        }
        if dropped_cache {
            message.push_str(" · cache disque vidé");
        } else if self.rules.drop_page_cache && !already && !self.dry_run {
            message.push_str(" · cache non vidé (droits administrateur requis)");
        }
        if already {
            message.push_str(" · (rafraîchi)");
        }

        Ok(FocusSession {
            action: FocusAction::Enter,
            suspended: suspended.len(),
            services_stopped: stopped_services.len(),
            wallpapers_paused: wallpapers.len(),
            message,
        })
    }

    fn leave(&mut self) -> Result<FocusSession, String> {
        if !self.snapshot.active && self.snapshot.suspended_pids.is_empty() {
            if let Ok(Some(snap)) = Self::load_persisted() {
                self.restore_snapshot(snap);
            }
        }

        let mut resumed = 0usize;
        let mut restarted = 0usize;
        let mut restart_failures = Vec::new();
        if self.dry_run || self.fake_processes.is_some() {
            resumed = self.snapshot.suspended_pids.len();
        } else {
            let mut sys = System::new();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            for pid_u in &self.snapshot.suspended_pids {
                let pid = Pid::from_u32(*pid_u);
                if let Some(proc_) = sys.process(pid) {
                    if proc_.kill_with(Signal::Continue).unwrap_or(false) {
                        resumed += 1;
                    }
                }
            }
            let refresh_paused_wallpapers = self
                .focus_started_at
                .is_some_and(|started| should_restart_wallpapers(started.elapsed()));
            for restart in &self.wallpaper_restarts {
                let mut same_process = sys
                    .process(Pid::from_u32(restart.pid))
                    .is_some_and(|process| process.start_time() == restart.start_time);
                if !should_launch_wallpaper_restart(
                    restart.stopped_for_memory,
                    same_process,
                    refresh_paused_wallpapers,
                    is_video_wallpaper_command(restart),
                ) {
                    continue;
                }
                if restart.stopped_for_memory && same_process {
                    match wait_for_process_exit(&mut sys, restart) {
                        Ok(()) => same_process = false,
                        Err(error) => {
                            restart_failures.push(error);
                            continue;
                        }
                    }
                }
                if same_process {
                    if let Err(error) = stop_process_before_restart(&mut sys, restart) {
                        restart_failures.push(error);
                        continue;
                    }
                }

                let mut command = Command::new(&restart.executable);
                command.args(&restart.args);
                if let Some(cwd) = &restart.working_dir {
                    command.current_dir(cwd);
                }
                match command.spawn() {
                    Ok(_) => restarted += 1,
                    Err(e) => restart_failures.push(format!("{} : {e}", restart.executable)),
                }
            }
        }

        let services = self.snapshot.stopped_services.clone();
        let wp_n = self.snapshot.paused_wallpapers.len();
        if !self.dry_run {
            for svc in &services {
                match Command::new("systemctl")
                    .args(["--user", "start", svc])
                    .status()
                {
                    Ok(status) if status.success() => {}
                    Ok(status) => restart_failures.push(format!(
                        "systemctl --user start {svc} a échoué ({})",
                        status
                    )),
                    Err(error) => {
                        restart_failures.push(format!("systemctl --user start {svc} : {error}"))
                    }
                }
            }
        }

        let mut message = format!(
            "Focus OFF — {resumed} processus repris · {restarted} application(s) relancée(s)."
        );
        if !restart_failures.is_empty() {
            message.push_str(&format!(
                " Relance échouée : {}",
                restart_failures.join(" ; ")
            ));
        }

        let session = FocusSession {
            action: FocusAction::Leave,
            suspended: resumed,
            services_stopped: services.len(),
            wallpapers_paused: wp_n,
            message,
        };

        self.snapshot = FocusSnapshot::default();
        self.focus_started_at = None;
        self.wallpaper_restarts.clear();
        self.clear_persisted()?;
        Ok(session)
    }

    fn is_active(&self) -> bool {
        self.snapshot.active
    }

    fn snapshot(&self) -> &FocusSnapshot {
        &self.snapshot
    }

    fn set_dry_run(&mut self, enabled: bool) {
        self.dry_run = enabled;
    }

    fn set_rules(&mut self, rules: FocusRules) {
        self.rules = rules;
    }
}

fn unix_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_wallpaper_process(process: &Process, keep: &HashSet<String>) -> bool {
    let name = process.name().to_string_lossy().to_lowercase();
    let exe = process
        .exe()
        .map(|path| path.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let cmd = process
        .cmd()
        .iter()
        .map(|arg| arg.to_string_lossy().to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let blob = format!("{name} {exe} {cmd}");
    wallpaper_process_names()
        .iter()
        .any(|wp| !keep.contains(*wp) && blob.contains(wp))
        || blob.contains("org.waywallen")
        || blob.contains("waywallen-video")
        || blob.contains("waywallen-wescene")
        || blob.contains("linux-wallpaperengine")
}

fn freeze_descendants(
    sys: &System,
    children: &HashMap<u32, Vec<u32>>,
    root: u32,
    suspended: &mut Vec<u32>,
    keep: &HashSet<String>,
    vram_stop: &HashSet<String>,
    ram_stop: &HashSet<String>,
) {
    let Some(kids) = children.get(&root) else {
        return;
    };
    for &child in kids {
        if suspended.contains(&child) {
            continue;
        }
        let pid = Pid::from_u32(child);
        let Some(proc_) = sys.process(pid) else {
            continue;
        };
        let name = proc_.name().to_string_lossy().to_lowercase();
        if is_critical_process(&name)
            || matches_any(&name, keep)
            || matches_any(&name, vram_stop)
            || matches_any(&name, ram_stop)
        {
            continue;
        }
        if matches!(
            proc_.thread_kind(),
            Some(ThreadKind::Userland | ThreadKind::Kernel)
        ) {
            // On gèle quand même les threads lourds liés au parent navigateur
        }
        if proc_.kill_with(Signal::Stop).unwrap_or(false) {
            suspended.push(child);
            freeze_descendants(sys, children, child, suspended, keep, vram_stop, ram_stop);
        }
    }
}

/// sync + drop_caches si accessible (sinon no-op après sync).
fn try_drop_page_cache() -> bool {
    let _ = Command::new("sync").status();
    match fs::write("/proc/sys/vm/drop_caches", "3") {
        Ok(()) => true,
        Err(_) => false, // sans root : sync seul (pas de pkexec bloquant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_restart_is_reserved_for_long_focus_sessions() {
        assert!(!should_restart_wallpapers(Duration::from_secs(299)));
        assert!(should_restart_wallpapers(Duration::from_secs(300)));
    }

    #[test]
    fn video_wallpaper_restarts_even_after_a_short_focus_session() {
        assert!(should_launch_wallpaper_restart(false, true, false, true));
        assert!(should_launch_wallpaper_restart(false, false, false, true));
        assert!(!should_launch_wallpaper_restart(false, true, false, false));
        assert!(should_launch_wallpaper_restart(false, true, true, false));
    }

    #[test]
    fn memory_stopped_application_restarts_as_soon_as_it_exits() {
        assert!(should_launch_wallpaper_restart(true, false, false, false));
        assert!(!should_launch_wallpaper_restart(true, true, true, true));
    }

    #[test]
    fn recognizes_supported_video_wallpaper_engines() {
        let command = |executable: &str| WallpaperRestartCommand {
            pid: 1,
            start_time: 1,
            executable: executable.into(),
            args: vec![],
            working_dir: None,
            stopped_for_memory: false,
        };
        assert!(is_video_wallpaper_command(&command("/usr/bin/mpvpaper")));
        assert!(is_video_wallpaper_command(&command(
            "/usr/bin/linux-wallpaperengine"
        )));
        assert!(!is_video_wallpaper_command(&command("/usr/bin/waypaper")));
    }

    #[test]
    fn wallpaper_restart_arguments_exclude_argv_zero() {
        let executable = PathBuf::from("/usr/bin/linux-wallpaperengine");
        let cmd = vec![
            "/usr/bin/linux-wallpaperengine".into(),
            "--screen-root".into(),
            "DP-1".into(),
        ];
        assert_eq!(restart_args(&cmd, &executable), ["--screen-root", "DP-1"]);
    }

    #[test]
    fn dry_run_enter_leave_with_fakes() {
        let mut engine = SysFocusController::new(FocusRules::default())
            .dry_run(true)
            .with_fake_processes(vec![
                (10, "discord".into()),
                (20, "firefox".into()),
                (30, "linux-wallpaperengine".into()),
            ]);
        let enter = engine.enter().unwrap();
        assert_eq!(enter.action, FocusAction::Enter);
        assert!(enter.suspended >= 2);
        assert!(engine.is_active());
        let leave = engine.leave().unwrap();
        assert_eq!(leave.action, FocusAction::Leave);
        assert!(!engine.is_active());
    }

    #[test]
    fn dry_run_records_stop_user_services() {
        let mut rules = FocusRules::default();
        rules.stop_user_services = vec!["example.slice".into()];
        let mut engine = SysFocusController::new(rules)
            .dry_run(true)
            .with_fake_processes(vec![(20, "firefox".into())]);
        let enter = engine.enter().unwrap();
        assert_eq!(enter.services_stopped, 1);
        assert!(engine
            .snapshot()
            .stopped_services
            .contains(&"example.slice".into()));
        let _ = engine.leave().unwrap();
    }

    #[test]
    fn dry_run_does_not_suspend_keep_list() {
        let mut engine = SysFocusController::new(FocusRules::default())
            .dry_run(true)
            .with_fake_processes(vec![
                (10, "discord".into()),
                (40, "steam".into()),
                (50, "pipewire".into()),
            ]);
        let enter = engine.enter().unwrap();
        assert_eq!(enter.suspended, 0);
        assert!(engine.snapshot().suspended_pids.is_empty());
    }

    #[test]
    fn dry_run_vram_stop_ollama() {
        let mut engine = SysFocusController::new(FocusRules::default())
            .dry_run(true)
            .with_fake_processes(vec![(80, "ollama".into()), (20, "firefox".into())]);
        let enter = engine.enter().unwrap();
        assert!(enter.suspended >= 1);
        assert!(engine.snapshot().vram_stopped_pids.contains(&80));
        assert!(!engine.snapshot().suspended_pids.contains(&80));
    }
}
