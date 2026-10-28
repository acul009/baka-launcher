//! Direct desktop command launching, without a shell or launcher service.

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use model::entry::Entry;

use crate::catalog;

/// A recoverable application launch error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The desktop entry has no executable command.
    #[error("This application has no launch command")]
    MissingCommand,
    /// A terminal application needs an installed terminal emulator.
    #[error("No supported terminal emulator found; install a terminal to open this application")]
    MissingTerminal,
    /// The OS rejected process creation or the working directory.
    #[error("Could not start {program}: {source}")]
    Spawn {
        /// Program sent to the operating system.
        program: String,
        /// Original process creation error.
        #[source]
        source: std::io::Error,
    },
}

/// Starts an application; success means the OS accepted process creation.
///
/// The launcher exits after this succeeds, so the target is not waited on.
pub fn spawn(entry: &Entry) -> Result<(), Error> {
    let Some(program) = entry.command.first() else {
        return Err(Error::MissingCommand);
    };
    let mut command = if entry.terminal {
        let paths: Vec<PathBuf> =
            env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect();
        let (terminal, prefix) = terminal(&paths).ok_or(Error::MissingTerminal)?;
        let mut command = Command::new(terminal);
        command.args(prefix).args(&entry.command);
        command
    } else {
        let mut command = Command::new(program);
        command.args(&entry.command[1..]);
        command
    };
    if let Some(directory) = &entry.directory {
        command.current_dir(directory);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn().map(|_| ()).map_err(|source| Error::Spawn {
        program: program.clone(),
        source,
    })
}

fn terminal(paths: &[PathBuf]) -> Option<(&'static str, &'static [&'static str])> {
    let candidates: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["-e"]),
        ("cosmic-term", &["-e"]),
        ("kitty", &["--"]),
        ("alacritty", &["-e"]),
        ("foot", &["-e"]),
        ("wezterm", &["start", "--"]),
        ("gnome-terminal", &["--"]),
        ("kgx", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["--execute"]),
        ("xterm", &["-e"]),
    ];
    candidates
        .iter()
        .copied()
        .find(|(program, _)| catalog::executable(program, paths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn entry(exec: &str) -> Entry {
        Entry::parse(
            "fixture.desktop".into(),
            "/tmp/fixture.desktop".into(),
            &format!("[Desktop Entry]\nType=Application\nName=Fixture\nExec={exec}\n"),
            "",
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn missing_program_and_directory_are_recoverable() {
        assert!(matches!(
            spawn(&entry("/definitely/not/a/program")),
            Err(Error::Spawn { .. })
        ));
        let mut entry = entry("true");
        entry.directory = Some("/definitely/not/a/directory".into());
        assert!(matches!(spawn(&entry), Err(Error::Spawn { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn launch_preserves_argument_boundaries_working_directory_and_literal_shell_text() {
        let directory = env::temp_dir().join(format!(
            "baka-launch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let names = ["argument with spaces", "$(touch injected)", "semi;colon"];
        let mut entry = entry("touch");
        entry.command = vec!["touch".into(), "--".into()];
        entry
            .command
            .extend(names.iter().map(|name| (*name).into()));
        entry.directory = Some(directory.clone());
        spawn(&entry).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !names.iter().all(|name| directory.join(name).exists()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        for name in names {
            assert!(
                directory.join(name).exists(),
                "literal argument missing: {name}"
            );
        }
        assert!(!directory.join("injected").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn terminal_lookup_requires_an_executable_and_known_argument_convention() {
        assert!(terminal(&[]).is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let directory = env::temp_dir().join(format!(
                "baka-terminal-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join("xterm");
            fs::write(&path, b"fixture").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(terminal(std::slice::from_ref(&directory)).is_none());
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            assert_eq!(
                terminal(std::slice::from_ref(&directory)),
                Some(("xterm", ["-e"].as_slice()))
            );
            fs::remove_dir_all(directory).unwrap();
        }
    }
}
