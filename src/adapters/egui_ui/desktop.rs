use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DesktopCommand {
    /// Une 2ᵉ ouverture demande d’afficher la fenêtre déjà active.
    Show,
}

#[derive(Clone)]
pub(super) struct DesktopCommandSender {
    sender: Sender<DesktopCommand>,
    context: Arc<Mutex<Option<egui::Context>>>,
}

impl DesktopCommandSender {
    pub(super) fn channel() -> (Self, Receiver<DesktopCommand>) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                sender,
                context: Arc::new(Mutex::new(None)),
            },
            receiver,
        )
    }

    pub(super) fn set_context(&self, context: egui::Context) {
        *self
            .context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(context);
    }

    fn send(&self, command: DesktopCommand) {
        if self.sender.send(command).is_ok() {
            if let Some(context) = self
                .context
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
            {
                context.request_repaint();
            }
        }
    }
}

pub(super) enum InstanceStatus {
    Primary(SingleInstanceGuard),
    AlreadyRunning,
}

pub(super) struct SingleInstanceGuard {
    socket_path: PathBuf,
    shutdown: Sender<()>,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        if let Err(error) = std::fs::remove_file(&self.socket_path) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("Suppression du socket GameFocus : {error}");
            }
        }
    }
}

pub(super) fn acquire_single_instance(
    commands: DesktopCommandSender,
) -> io::Result<InstanceStatus> {
    let runtime_dir = runtime_directory()?;
    acquire_single_instance_at(&runtime_dir, commands)
}

fn acquire_single_instance_at(
    runtime_dir: &std::path::Path,
    commands: DesktopCommandSender,
) -> io::Result<InstanceStatus> {
    let socket_path = runtime_dir.join("gamefocus.sock");

    for _ in 0..2 {
        match UnixListener::bind(&socket_path) {
            Ok(listener) => {
                std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
                listener.set_nonblocking(true)?;
                let (shutdown, shutdown_rx) = mpsc::channel();
                thread::Builder::new()
                    .name("gamefocus-activation".into())
                    .spawn(move || loop {
                        if shutdown_rx.try_recv().is_ok() {
                            break;
                        }
                        match listener.accept() {
                            Ok((mut stream, _)) => {
                                let mut request = [0; 16];
                                if stream.read(&mut request).is_ok() && request.starts_with(b"show")
                                {
                                    commands.send(DesktopCommand::Show);
                                    let _ = stream.write_all(b"ok");
                                }
                            }
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(100));
                            }
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                            Err(error) => {
                                eprintln!("Socket d’activation GameFocus : {error}");
                                break;
                            }
                        }
                    })?;
                return Ok(InstanceStatus::Primary(SingleInstanceGuard {
                    socket_path,
                    shutdown,
                }));
            }
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                if let Ok(mut stream) = UnixStream::connect(&socket_path) {
                    stream.write_all(b"show")?;
                    return Ok(InstanceStatus::AlreadyRunning);
                }

                let metadata = std::fs::symlink_metadata(&socket_path)?;
                if !metadata.file_type().is_socket() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "Le chemin d’activation GameFocus existe mais n’est pas un socket.",
                    ));
                }
                std::fs::remove_file(&socket_path)?;
            }
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AddrInUse,
        "Une autre instance GameFocus démarre déjà.",
    ))
}

fn runtime_directory() -> io::Result<PathBuf> {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(runtime_dir));
    }

    let uid = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("Uid:")
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|value| value.parse::<u32>().ok())
            })
        })
        .or_else(|| std::env::var("UID").ok()?.parse().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Impossible de déterminer l’utilisateur pour le socket d’activation.",
            )
        })?;
    let runtime_dir = std::env::temp_dir().join(format!("gamefocus-{uid}"));
    if !runtime_dir.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&runtime_dir)?;
    }
    let metadata = std::fs::metadata(&runtime_dir)?;
    if !metadata.is_dir() || metadata.uid() != uid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Le répertoire temporaire GameFocus n’appartient pas à cet utilisateur.",
        ));
    }
    std::fs::set_permissions(&runtime_dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(runtime_dir)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_graphical_launch_activates_the_first_instance() {
        let runtime_dir = tempfile::tempdir().unwrap();
        let (commands, receiver) = DesktopCommandSender::channel();
        let first = acquire_single_instance_at(runtime_dir.path(), commands.clone()).unwrap();
        assert!(matches!(first, InstanceStatus::Primary(_)));

        let second = acquire_single_instance_at(runtime_dir.path(), commands).unwrap();
        assert!(matches!(second, InstanceStatus::AlreadyRunning));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            DesktopCommand::Show
        ));
    }
}
