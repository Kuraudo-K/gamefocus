//! Smoke tests du binaire `gamefocus` (CLI, sans UI).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gamefocus"))
}

#[test]
fn help_exits_zero_and_lists_flags() {
    let out = bin().arg("--help").output().expect("run --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--scan"));
    assert!(stdout.contains("--focus-on"));
    assert!(stdout.contains("--focus-off"));
    assert!(stdout.contains("--demo"));
    assert!(stdout.contains("GAMEFOCUS_DRY_RUN") || stdout.contains("dry-run"));
}

#[test]
fn demo_cycle_prints_focus_messages() {
    let out = bin()
        .env("GAMEFOCUS_DEMO", "1")
        .arg("--demo")
        .output()
        .expect("run --demo");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Démo") || stdout.contains("Focus") || stdout.contains("Fin démo"));
    assert!(
        stdout.to_lowercase().contains("discord")
            || stdout.contains("non ciblés")
            || stdout.contains("dry-run")
            || stdout.contains("Focus")
    );
}

#[test]
fn unknown_arg_exits_nonzero() {
    let out = bin()
        .arg("--not-a-real-flag")
        .output()
        .expect("run unknown");
    assert!(!out.status.success());
}

#[test]
fn scan_exits_zero_even_if_empty() {
    // HOME vide → 0 jeux, mais pas d'erreur
    let dir = tempfile::tempdir().unwrap();
    let out = bin()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .arg("--scan")
        .output()
        .expect("run --scan");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("jeu"));
}

#[test]
fn focus_on_off_dry_run_via_env() {
    let dir = tempfile::tempdir().unwrap();
    let on = bin()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("GAMEFOCUS_DRY_RUN", "1")
        .arg("--focus-on")
        .output()
        .expect("focus-on");
    assert!(
        on.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&on.stderr)
    );
    let off = bin()
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join(".config"))
        .env("GAMEFOCUS_DRY_RUN", "1")
        .arg("--focus-off")
        .output()
        .expect("focus-off");
    assert!(
        off.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&off.stderr)
    );
}
