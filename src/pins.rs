//! User preferences for overriding the built-in application pins.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const LIMIT: usize = 64 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Pin overrides and their optional user configuration location.
#[derive(Debug, Clone, Default)]
pub struct Preferences {
    overrides: BTreeMap<String, bool>,
    path: Option<PathBuf>,
}

/// A pin preference could not be read or saved.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No user configuration location is available.
    #[error("No user configuration directory is available")]
    NoLocation,
    /// The preference file is invalid.
    #[error("Invalid pins.conf: expected +desktop-id or -desktop-id on each line")]
    Format,
    /// Limit preference files to a reasonable size.
    #[error("Pin preferences exceed the 64 KiB limit")]
    TooLarge,
    /// The operating system rejected a file operation.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl Preferences {
    /// Loads preferences from the user's XDG configuration directory.
    pub fn load_default() -> Result<Self, Error> {
        let directory = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|path| path.join(".config"))
            })
            .ok_or(Error::NoLocation)?;
        Self::load(directory.join("baka-launcher/pins.conf"))
    }

    /// Loads explicit pin/unpin overrides; a missing file uses built-in defaults.
    pub fn load(path: PathBuf) -> Result<Self, Error> {
        let mut preferences = Self {
            path: Some(path.clone()),
            ..Self::default()
        };
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(preferences),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take((LIMIT + 1) as u64).read_to_end(&mut bytes)?;
        if bytes.len() > LIMIT {
            return Err(Error::TooLarge);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| Error::Format)?;
        for line in text.lines().filter(|line| !line.is_empty()) {
            let (pinned, id) = if let Some(id) = line.strip_prefix('+') {
                (true, id)
            } else if let Some(id) = line.strip_prefix('-') {
                (false, id)
            } else {
                return Err(Error::Format);
            };
            if id.is_empty() || id.chars().any(char::is_control) {
                return Err(Error::Format);
            }
            preferences.overrides.insert(id.to_owned(), pinned);
        }
        Ok(preferences)
    }

    /// Returns the explicit preference, or the built-in pin default.
    pub fn is_pinned(&self, entry: &model::Entry) -> bool {
        self.overrides
            .get(&entry.id)
            .copied()
            .unwrap_or_else(|| model::search::is_pinned(entry))
    }

    /// Changes an override without altering built-in defaults for other apps.
    pub fn set(&mut self, id: String, pinned: bool) {
        self.overrides.insert(id, pinned);
    }

    /// Atomically saves preferences so an interrupted write keeps the old file.
    pub fn save(&self) -> Result<(), Error> {
        let path = self.path.as_ref().ok_or(Error::NoLocation)?;
        let directory = path.parent().ok_or(Error::NoLocation)?;
        let mut content = String::new();
        for (id, pinned) in &self.overrides {
            if id.is_empty() || id.chars().any(char::is_control) {
                return Err(Error::Format);
            }
            content.push(if *pinned { '+' } else { '-' });
            content.push_str(id);
            content.push('\n');
        }
        if content.len() > LIMIT {
            return Err(Error::TooLarge);
        }
        fs::create_dir_all(directory)?;
        let temporary = directory.join(format!(
            ".pins-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| {
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.map_err(Error::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, NEXT, Preferences};
    use std::fs;
    use std::sync::atomic::Ordering;

    #[test]
    fn overrides_survive_restart_including_unpinning_a_default() {
        let directory = std::env::temp_dir().join(format!(
            "baka-pins-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join("pins.conf");
        let default = model::Entry::parse(
            "code.desktop".into(),
            "code.desktop".into(),
            "[Desktop Entry]\nType=Application\nName=Code\nExec=code",
            "C",
        )
        .unwrap()
        .unwrap();
        let mut custom = default.clone();
        custom.id = "custom.desktop".into();
        let mut preferences = Preferences::load(path.clone()).unwrap();
        assert!(preferences.is_pinned(&default));
        assert!(!preferences.is_pinned(&custom));
        preferences.set(default.id.clone(), false);
        preferences.set(custom.id.clone(), true);
        preferences.save().unwrap();
        let restarted = Preferences::load(path.clone()).unwrap();
        assert!(!restarted.is_pinned(&default));
        assert!(restarted.is_pinned(&custom));
        let mut unpinned = restarted;
        unpinned.set(custom.id.clone(), false);
        unpinned.save().unwrap();
        assert!(!Preferences::load(path).unwrap().is_pinned(&custom));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn malformed_or_unwritable_preferences_are_reported_without_overwriting() {
        let directory = std::env::temp_dir().join(format!(
            "baka-pins-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("pins.conf");
        fs::write(&path, "invalid preferences").unwrap();
        assert!(matches!(
            Preferences::load(path.clone()),
            Err(Error::Format)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid preferences");
        let preferences = Preferences::load(path.join("not-a-directory")).unwrap_err();
        assert!(matches!(preferences, Error::Io(_)));
        let mut preferences = Preferences::default();
        preferences.set("example.desktop".into(), true);
        assert!(matches!(preferences.save(), Err(Error::NoLocation)));
        fs::remove_dir_all(directory).unwrap();
    }
}
