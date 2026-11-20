//! Liste des applications en cours (sysinfo) — rapide, sans threads.

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::Instant;

use sysinfo::{ProcessesToUpdate, System, ThreadKind};

use crate::domain::{
    is_critical_process, process_name_matches, select_focus_targets, FocusRules, RunningApp,
};

pub const MIN_RAM_RELEASE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningResourceApp {
    pub name: String,
    pub pid_count: usize,
    pub ram_bytes: u64,
    pub vram_bytes: u64,
    pub protection_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryStatus {
    pub ram_total: u64,
    pub ram_available: u64,
    pub vram_total: Option<u64>,
    pub vram_used: Option<u64>,
}

/// Scanne les process principaux et agrège par nom d'exécutable.
pub fn list_running_apps() -> (Vec<RunningApp>, u128) {
    let t0 = Instant::now();
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut map: HashMap<String, RunningApp> = HashMap::new();
    for (_pid, proc_) in sys.processes() {
        if matches!(
            proc_.thread_kind(),
            Some(ThreadKind::Userland | ThreadKind::Kernel)
        ) {
            continue;
        }
        let name = proc_.name().to_string_lossy().to_lowercase();
        if name.is_empty() || name.starts_with('[') {
            continue;
        }
        let name_base = std::path::Path::new(name.as_str())
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or(name);
        let exe = proc_
            .exe()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| name_base.clone());

        let key = if !exe.is_empty() {
            exe.clone()
        } else {
            name_base.clone()
        };
        let entry = map.entry(key.clone()).or_insert_with(|| RunningApp {
            name: key,
            exe: exe.clone(),
            pid_count: 0,
        });
        entry.pid_count += 1;
    }

    let mut apps: Vec<RunningApp> = map.into_values().collect();
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    (apps, t0.elapsed().as_millis())
}

/// Scanne les processus et ne marque comme libérables que les processus utilisateur vérifiés.
pub fn list_running_resource_apps() -> (Vec<RunningResourceApp>, u128) {
    let t0 = Instant::now();
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let vram_by_pid = list_vram_by_pid();
    let uid = current_uid();

    let mut apps: HashMap<String, RunningResourceApp> = HashMap::new();
    for (pid, process) in sys.processes() {
        if matches!(
            process.thread_kind(),
            Some(ThreadKind::Userland | ThreadKind::Kernel)
        ) {
            continue;
        }
        let pid = pid.as_u32();
        let ram_bytes = process.memory();
        let vram_bytes = vram_by_pid.get(&pid).copied().unwrap_or_default();
        if ram_bytes == 0 && vram_bytes == 0 {
            continue;
        }

        let name = process.name().to_string_lossy().to_lowercase();
        if name.is_empty() || name.starts_with('[') {
            continue;
        }
        let name = std::path::Path::new(&name)
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or(name);
        let exe = process.exe().map(|path| path.to_path_buf());
        let protection_reason = process_protection_reason(pid, &name, exe.as_deref(), uid);
        let app = apps
            .entry(name.clone())
            .or_insert_with(|| RunningResourceApp {
                name,
                pid_count: 0,
                ram_bytes: 0,
                vram_bytes: 0,
                protection_reason: None,
            });
        app.pid_count += 1;
        app.ram_bytes = app.ram_bytes.saturating_add(ram_bytes);
        app.vram_bytes = app.vram_bytes.saturating_add(vram_bytes);
        if protection_reason.is_some() {
            app.protection_reason = protection_reason;
        }
    }

    let mut apps: Vec<_> = apps.into_values().collect();
    apps.sort_by(|a, b| {
        b.vram_bytes
            .max(b.ram_bytes)
            .cmp(&a.vram_bytes.max(a.ram_bytes))
            .then_with(|| a.name.cmp(&b.name))
    });
    (apps, t0.elapsed().as_millis())
}

pub fn memory_status() -> MemoryStatus {
    let mut sys = System::new();
    sys.refresh_memory();
    let (vram_total, vram_used) = gpu_memory_status();
    MemoryStatus {
        ram_total: sys.total_memory(),
        ram_available: sys.available_memory(),
        vram_total,
        vram_used,
    }
}

fn gpu_memory_status() -> (Option<u64>, Option<u64>) {
    if let Some(status) = nvidia_gpu_memory_status() {
        return status;
    }
    amd_gpu_memory_status()
}

fn nvidia_gpu_memory_status() -> Option<(Option<u64>, Option<u64>)> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    parse_nvidia_gpu_memory_csv(&raw)
}

fn parse_nvidia_gpu_memory_csv(raw: &str) -> Option<(Option<u64>, Option<u64>)> {
    let mut total = 0_u64;
    let mut used = 0_u64;
    let mut found = false;
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let (gpu_total, gpu_used) = line.split_once(',')?;
        let (Ok(gpu_total), Ok(gpu_used)) = (
            gpu_total.trim().parse::<u64>(),
            gpu_used.trim().parse::<u64>(),
        ) else {
            return None;
        };
        total = total.saturating_add(gpu_total.saturating_mul(1024 * 1024));
        used = used.saturating_add(gpu_used.saturating_mul(1024 * 1024));
        found = true;
    }
    found.then_some((Some(total), Some(used)))
}

fn amd_gpu_memory_status() -> (Option<u64>, Option<u64>) {
    let Ok(cards) = fs::read_dir("/sys/class/drm") else {
        return (None, None);
    };
    let mut total = 0_u64;
    let mut used = 0_u64;
    let mut found = false;
    for card in cards.flatten() {
        let name = card.file_name();
        let name = name.to_string_lossy();
        if !name
            .strip_prefix("card")
            .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let device = card.path().join("device");
        let Ok(card_total) =
            fs::read_to_string(device.join("mem_info_vram_total")).map(|v| v.trim().parse::<u64>())
        else {
            continue;
        };
        let Ok(card_used) =
            fs::read_to_string(device.join("mem_info_vram_used")).map(|v| v.trim().parse::<u64>())
        else {
            continue;
        };
        let (Ok(card_total), Ok(card_used)) = (card_total, card_used) else {
            continue;
        };
        total = total.saturating_add(card_total);
        used = used.saturating_add(card_used);
        found = true;
    }
    if found {
        (Some(total), Some(used))
    } else {
        (None, None)
    }
}

pub(crate) fn current_uid() -> Option<u32> {
    process_uid("/proc/self/status")
}

fn process_uid(status_path: impl AsRef<std::path::Path>) -> Option<u32> {
    let status = fs::read_to_string(status_path).ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

pub(crate) fn process_protection_reason(
    pid: u32,
    name: &str,
    exe: Option<&std::path::Path>,
    uid: Option<u32>,
) -> Option<String> {
    if pid <= 1 || pid == std::process::id() {
        return Some("processus système ou GameFocus".into());
    }
    if is_critical_process(name)
        || exe
            .and_then(std::path::Path::file_name)
            .is_some_and(|file| is_critical_process(&file.to_string_lossy()))
    {
        return Some("composant système, graphique, bureau ou jeu protégé".into());
    }
    let Some(uid) = uid else {
        return Some("identité utilisateur non vérifiable".into());
    };
    let Some(target_uid) = process_uid(format!("/proc/{pid}/status")) else {
        return Some("propriétaire du processus non vérifiable".into());
    };
    if target_uid != uid {
        return Some("appartient à un autre utilisateur".into());
    }

    let Ok(cgroup) = fs::read_to_string(format!("/proc/{pid}/cgroup")) else {
        return Some("rôle systemd non vérifiable".into());
    };
    let Some(leaf) = cgroup
        .lines()
        .filter_map(|line| line.rsplit(':').next())
        .filter_map(|path| path.rsplit('/').next())
        .filter(|part| !part.is_empty())
        .last()
    else {
        return Some("rôle systemd non vérifiable".into());
    };
    if cgroup.lines().any(|line| {
        let path = line.rsplit(':').next().unwrap_or_default();
        path == "/init.scope" || path.contains("/system.slice/")
    }) {
        return Some("service système (systemd)".into());
    }
    if leaf.ends_with(".service") {
        return Some("service utilisateur géré par systemd".into());
    }

    None
}

/// Usage VRAM par PID via nvidia-smi, puis via le comptage DRM de fdinfo.
pub fn list_vram_by_pid() -> HashMap<u32, u64> {
    let mut vram_by_pid = list_nvidia_vram_by_pid().unwrap_or_default();
    let Ok(processes) = fs::read_dir("/proc") else {
        return vram_by_pid;
    };
    for entry in processes.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let bytes = process_vram_bytes(pid);
        if bytes > 0 {
            vram_by_pid
                .entry(pid)
                .and_modify(|current: &mut u64| *current = (*current).max(bytes))
                .or_insert(bytes);
        }
    }
    vram_by_pid
}

fn list_nvidia_vram_by_pid() -> Option<HashMap<u32, u64>> {
    let output = Command::new("nvidia-smi")
        .args(["-q", "-x"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let xml = String::from_utf8(output.stdout).ok()?;
    Some(parse_nvidia_vram_xml(&xml))
}

fn parse_nvidia_vram_xml(xml: &str) -> HashMap<u32, u64> {
    let mut processes = HashMap::new();

    for block in xml_tag_blocks(&xml, "process_info") {
        let Some(pid) = xml_tag_value(block, "pid").and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(vram_bytes) = xml_tag_value(block, "used_memory").and_then(parse_memory_bytes)
        else {
            continue;
        };
        if vram_bytes == 0 {
            continue;
        }
        processes
            .entry(pid)
            .and_modify(|current: &mut u64| *current = current.saturating_add(vram_bytes))
            .or_insert(vram_bytes);
    }
    processes
}

fn xml_tag_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut blocks = Vec::new();
    let mut remaining = xml;
    while let Some(start) = remaining.find(&open) {
        let after_open = &remaining[start + open.len()..];
        let Some(end) = after_open.find(&close) else {
            break;
        };
        blocks.push(&after_open[..end]);
        remaining = &after_open[end + close.len()..];
    }
    blocks
}

fn xml_tag_value<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim())
}

/// VRAM par processus, telle que publiée par le pilote DRM dans fdinfo.
pub fn process_vram_bytes(pid: u32) -> u64 {
    let fdinfo_dir = format!("/proc/{pid}/fdinfo");
    let Ok(entries) = fs::read_dir(fdinfo_dir) else {
        return 0;
    };

    let mut clients: HashMap<(String, String), u64> = HashMap::new();
    let mut fallback_devices: HashMap<String, u64> = HashMap::new();
    for entry in entries.flatten() {
        let Ok(contents) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let Some((device, client_id, bytes)) = parse_fdinfo_vram(&contents) else {
            continue;
        };
        if let Some(client_id) = client_id {
            clients
                .entry((device, client_id))
                .and_modify(|current| *current = (*current).max(bytes))
                .or_insert(bytes);
        } else {
            fallback_devices
                .entry(device)
                .and_modify(|current| *current = (*current).max(bytes))
                .or_insert(bytes);
        }
    }

    clients
        .values()
        .chain(fallback_devices.values())
        .fold(0_u64, |total, bytes| total.saturating_add(*bytes))
}

fn parse_fdinfo_vram(contents: &str) -> Option<(String, Option<String>, u64)> {
    let mut device = String::new();
    let mut client_id = None;
    let mut vram_bytes = None;

    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("drm-pdev:") {
            device = value.trim().to_owned();
        } else if let Some(value) = line.strip_prefix("drm-client-id:") {
            client_id = Some(value.trim().to_owned());
        } else if let Some(value) = line.strip_prefix("drm-memory-vram:") {
            vram_bytes = parse_memory_bytes(value.trim());
        } else if vram_bytes.is_none() {
            if let Some(value) = line.strip_prefix("drm-memory-local:") {
                vram_bytes = parse_memory_bytes(value.trim());
            }
        }
    }

    vram_bytes
        .filter(|bytes| *bytes > 0)
        .map(|bytes| (device, client_id, bytes))
}

fn parse_memory_bytes(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let raw_amount = parts.next()?;
    let amount = raw_amount
        .trim_end_matches("MiB")
        .trim_end_matches("MB")
        .trim_end_matches("KiB")
        .trim_end_matches("kB")
        .trim_end_matches("KB")
        .trim_end_matches("GiB")
        .trim_end_matches("GB")
        .trim_end_matches('B')
        .parse::<u64>()
        .ok()?;
    let unit = parts
        .next()
        .or_else(|| {
            ["GiB", "GB", "MiB", "MB", "KiB", "kB", "KB", "B"]
                .into_iter()
                .find(|suffix| raw_amount.ends_with(suffix))
        })
        .unwrap_or("B");
    let multiplier = match unit {
        "B" | "bytes" => 1,
        "kB" | "KB" | "KiB" => 1024,
        "MB" | "MiB" => 1024 * 1024,
        "GB" | "GiB" => 1024 * 1024 * 1024,
        _ => return None,
    };
    Some(amount.saturating_mul(multiplier))
}

/// Applis actuellement lancées qui seraient gelées par les règles Focus.
pub fn preview_focus_targets(rules: &FocusRules) -> (Vec<RunningApp>, u128) {
    let (apps, ms) = list_running_apps();
    let procs: Vec<(u32, &str)> = apps
        .iter()
        .enumerate()
        .map(|(i, a)| (i as u32 + 1, a.name.as_str()))
        .collect();
    let hit_ids = select_focus_targets(&procs, rules);
    let hit: Vec<RunningApp> = apps
        .into_iter()
        .enumerate()
        .filter(|(i, _)| hit_ids.contains(&(*i as u32 + 1)))
        .map(|(_, a)| a)
        .collect();
    (hit, ms)
}

/// Vrai si une appli tournante est déjà dans keep / suspend.
pub fn is_listed(name: &str, list: &[String]) -> bool {
    list.iter().any(|k| process_name_matches(name, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fdinfo_vram_parser_uses_only_vram_and_dedupes_by_drm_client() {
        let first = "drm-driver: amdgpu\ndrm-pdev: 0000:03:00.0\ndrm-client-id: 7\ndrm-memory-vram: 1024 KiB\ndrm-memory-gtt: 4096 KiB";
        let duplicate = "drm-driver: amdgpu\ndrm-pdev: 0000:03:00.0\ndrm-client-id: 7\ndrm-memory-vram: 1024 KiB";
        let other_client =
            "drm-driver: amdgpu\ndrm-pdev: 0000:03:00.0\ndrm-client-id: 8\ndrm-memory-vram: 2 MiB";

        let parsed = [
            parse_fdinfo_vram(first).unwrap(),
            parse_fdinfo_vram(duplicate).unwrap(),
            parse_fdinfo_vram(other_client).unwrap(),
        ];
        let mut clients = HashMap::new();
        for (device, client, bytes) in parsed {
            let client = client.unwrap();
            clients
                .entry((device, client))
                .and_modify(|current: &mut u64| *current = (*current).max(bytes))
                .or_insert(bytes);
        }

        assert_eq!(clients.values().sum::<u64>(), 3 * 1024 * 1024);
    }

    #[test]
    fn fdinfo_without_vram_is_not_reported() {
        assert!(parse_fdinfo_vram("drm-memory-gtt: 1024 KiB").is_none());
        assert!(parse_fdinfo_vram("drm-memory-vram: 0 KiB").is_none());
    }

    #[test]
    fn nvidia_xml_process_memory_is_parsed_in_mib() {
        let xml = "<gpu><process_info><pid>42</pid><used_memory>128 MiB</used_memory></process_info><process_info><pid>42</pid><used_memory>64 MiB</used_memory></process_info></gpu>";
        assert_eq!(
            parse_nvidia_vram_xml(xml).get(&42),
            Some(&(192 * 1024 * 1024))
        );
    }

    #[test]
    fn memory_parser_accepts_units_with_or_without_whitespace() {
        assert_eq!(parse_memory_bytes("64MiB"), Some(64 * 1024 * 1024));
        assert_eq!(parse_memory_bytes("64 MiB"), Some(64 * 1024 * 1024));
        assert_eq!(parse_memory_bytes("Not available"), None);
    }

    #[test]
    fn nvidia_gpu_csv_requires_complete_measurements() {
        assert_eq!(
            parse_nvidia_gpu_memory_csv("8192, 1024\n4096, 2048\n"),
            Some((Some(12 * 1024 * 1024 * 1024), Some(3 * 1024 * 1024 * 1024)))
        );
        assert_eq!(parse_nvidia_gpu_memory_csv("8192, 1024\nN/A, N/A\n"), None);
        assert_eq!(parse_nvidia_gpu_memory_csv("N/A, N/A\n"), None);
    }

    #[test]
    fn graphics_and_session_processes_are_marked_critical() {
        for name in [
            ".gnome-shell-wr",
            "gnome-shell",
            "kwin_wayland",
            "plasmashell",
            "Xwayland",
            "nvidia-persistenced",
        ] {
            assert!(is_critical_process(name), "{name} must be protected");
        }
    }

    #[test]
    fn list_running_is_fast_and_nonempty() {
        let (apps, ms) = list_running_apps();
        assert!(!apps.is_empty());
        assert!(ms < 2000, "scan process trop lent : {ms} ms (seuil 2s)");
    }

    #[test]
    fn preview_respects_keep_discord() {
        let mut rules = FocusRules::default();
        // Forcer un nom improbable dans suspend pour test déterministe
        rules.suspend = vec!["___no_such_process_gamefocus___".into()];
        let (hit, _) = preview_focus_targets(&rules);
        assert!(hit.is_empty() || hit.iter().all(|a| a.name != "discord"));
    }
}
