//! Linux desktop application discovery and immutable icon loading.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use model::entry::Entry;

/// A launchable application and its optional, already loaded icon.
#[derive(Debug, Clone)]
pub struct App {
    /// Parsed desktop entry.
    pub entry: Entry,
    /// Icon bytes remain usable if the source file disappears.
    pub icon: Option<Icon>,
}

/// Supported installed icon formats.
#[derive(Debug, Clone)]
pub enum Icon {
    /// Scalable vector icon.
    Svg(Vec<u8>),
    /// PNG image.
    Raster(Vec<u8>),
}

/// An application discovery failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Native discovery is currently implemented for Linux.
    #[error("Application discovery is currently supported on Linux only")]
    UnsupportedPlatform,
}

/// Discovers installed applications without creating a cache or resident worker.
pub fn scan() -> Result<Vec<App>, Error> {
    if !cfg!(target_os = "linux") {
        return Err(Error::UnsupportedPlatform);
    }

    let home = env::var_os("HOME").map(PathBuf::from);
    let user_data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.as_ref().map(|path| path.join(".local/share")));
    let mut roots = Vec::new();
    if let Some(path) = user_data {
        roots.push(path);
    }
    let system_data = env::var_os("XDG_DATA_DIRS").filter(|value| !value.is_empty());
    roots.extend(
        env::split_paths(&system_data.unwrap_or_else(|| "/usr/local/share:/usr/share".into()))
            .filter(|path| path.is_absolute()),
    );

    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| env::var(key).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default();
    let desktops = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let search_path = env::var_os("PATH").unwrap_or_default();
    let paths: Vec<_> = env::split_paths(&search_path).collect();
    let mut icon_roots: Vec<_> = roots.iter().map(|path| path.join("icons")).collect();
    if let Some(home) = home {
        icon_roots.insert(0, home.join(".icons"));
    }
    let mut locations = icon_locations(&icon_roots);
    locations.extend(roots.iter().map(|path| path.join("pixmaps")));

    Ok(scan_roots(&roots, &locale, &desktops, &paths, &locations))
}

fn scan_roots(
    roots: &[PathBuf],
    locale: &str,
    desktops: &str,
    paths: &[PathBuf],
    icon_locations: &[PathBuf],
) -> Vec<App> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    let desktops: Vec<String> = desktops
        .split(':')
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();

    for root in roots {
        let applications = root.join("applications");
        let mut pending = vec![(applications.clone(), 0)];
        let mut visited = 0;
        while let Some((directory, depth)) = pending.pop() {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            let mut entries: Vec<_> = entries.flatten().take(100_001).collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                visited += 1;
                if visited > 100_000 {
                    break;
                }
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                // Directory symlinks may point back into this tree or leave it.
                if kind.is_dir() {
                    if depth < 32 {
                        pending.push((path, depth + 1));
                    }
                    continue;
                }
                if path
                    .extension()
                    .is_none_or(|extension| extension != "desktop")
                {
                    continue;
                }
                let Ok(relative) = path.strip_prefix(&applications) else {
                    continue;
                };
                let id = relative
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "-");
                // Even a hidden or invalid user override masks the system entry.
                if !seen.insert(id.clone()) {
                    continue;
                }
                let Ok(text) = read_bounded(&path, 1024 * 1024).and_then(|bytes| {
                    String::from_utf8(bytes).map_err(|error| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, error)
                    })
                }) else {
                    continue;
                };
                let Ok(Some(entry)) = Entry::parse(id, path, &text, locale) else {
                    continue;
                };
                if !entry.visible_on(&desktops)
                    || entry
                        .try_exec
                        .as_ref()
                        .is_some_and(|program| !executable(program, paths))
                {
                    continue;
                }
                let icon = entry
                    .icon
                    .as_deref()
                    .and_then(|name| load_icon(name, icon_locations));
                apps.push(App { entry, icon });
            }
            if visited > 100_000 {
                break;
            }
        }
    }
    apps.sort_by(|a, b| {
        a.entry
            .name
            .to_lowercase()
            .cmp(&b.entry.name.to_lowercase())
            .then_with(|| a.entry.id.cmp(&b.entry.id))
    });
    apps
}

pub(crate) fn executable(program: &str, paths: &[PathBuf]) -> bool {
    fn runnable(path: &Path) -> bool {
        let Ok(metadata) = path.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
    if Path::new(program).is_absolute() {
        runnable(Path::new(program))
    } else if program.contains(std::path::MAIN_SEPARATOR) {
        false
    } else {
        paths
            .iter()
            .any(|directory| runnable(&directory.join(program)))
    }
}

fn icon_locations(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut locations = Vec::new();
    for root in roots {
        let mut themes = vec![root.join("hicolor")];
        if let Ok(entries) = fs::read_dir(root) {
            let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
            entries.sort();
            themes.extend(
                entries
                    .into_iter()
                    .filter(|path| path.is_dir() && !path.ends_with("hicolor"))
                    .take(64),
            );
        }
        for theme in themes {
            for size in [
                "64x64", "128x128", "scalable", "48x48", "256x256", "96x96", "32x32", "symbolic",
            ] {
                locations.push(theme.join(size).join("apps"));
            }
        }
        locations.push(root.clone());
    }
    locations
}

fn read_bounded(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    if !path.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a regular file",
        ));
    }
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "File exceeds size limit",
        ));
    }
    Ok(bytes)
}

fn load_icon(name: &str, locations: &[PathBuf]) -> Option<Icon> {
    fn load(path: &Path) -> Option<Icon> {
        let extension = path.extension()?.to_str()?;
        let bytes = read_bounded(path, 4 * 1024 * 1024).ok()?;
        match extension {
            "svg" if std::str::from_utf8(&bytes).ok()?.contains("<svg") => Some(Icon::Svg(bytes)),
            "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Some(Icon::Raster(bytes)),
            _ => None,
        }
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return load(path);
    }
    if path.file_name()?.to_str()? != name {
        return None;
    }
    for location in locations {
        // Reverse-DNS icon names contain dots without specifying an image format.
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("svg" | "png")
        ) {
            if let Some(icon) = load(&location.join(name)) {
                return Some(icon);
            }
        } else {
            for extension in ["svg", "png"] {
                if let Some(icon) = load(&location.join(format!("{name}.{extension}"))) {
                    return Some(icon);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "baka-catalog-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn entry(&self, root: &str, name: &str, content: &str) {
            let path = self.0.join(root).join("applications").join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        fn scan(&self) -> Vec<App> {
            scan_roots(
                &[self.0.join("user"), self.0.join("system")],
                "en_US.UTF-8",
                "GNOME:Unity",
                &[PathBuf::from("/usr/bin")],
                &[],
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn hidden_user_override_masks_system_entry_and_nested_ids_are_stable() {
        let fixture = Fixture::new();
        fixture.entry(
            "system",
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Visible\nExec=true\n",
        );
        fixture.entry("user", "a.desktop", "[Desktop Entry]\nHidden=true\n");
        fixture.entry(
            "system",
            "nested/b.desktop",
            "[Desktop Entry]\nType=Application\nName=Nested\nExec=true\n",
        );
        let apps = fixture.scan();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].entry.id, "nested-b.desktop");
    }

    #[test]
    fn desktop_visibility_and_try_exec_are_respected() {
        let fixture = Fixture::new();
        for (id, flags) in [
            ("yes", "OnlyShowIn=GNOME;"),
            ("no", "NotShowIn=Unity;"),
            ("other", "OnlyShowIn=KDE;"),
            ("missing", "TryExec=/definitely/missing/program"),
            ("hidden", "NoDisplay=true"),
        ] {
            fixture.entry(
                "system",
                &format!("{id}.desktop"),
                &format!("[Desktop Entry]\nType=Application\nName={id}\nExec=true\n{flags}\n"),
            );
        }
        let apps = fixture.scan();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].entry.name, "yes");
    }

    #[test]
    fn icons_are_owned_and_missing_or_broken_files_fall_back() {
        let fixture = Fixture::new();
        let icon_path = fixture.0.join("app.svg");
        fs::write(&icon_path, b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        let icon = load_icon(icon_path.to_str().unwrap(), &[]).unwrap();
        fs::remove_file(&icon_path).unwrap();
        assert!(matches!(icon, Icon::Svg(bytes) if !bytes.is_empty()));
        assert!(load_icon(icon_path.to_str().unwrap(), &[]).is_none());
        fs::write(&icon_path, b"not an icon").unwrap();
        assert!(load_icon(icon_path.to_str().unwrap(), &[]).is_none());
        assert!(load_icon(fixture.0.to_str().unwrap(), &[]).is_none());
        assert!(read_bounded(&icon_path, 3).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(&icon_path, b"<svg/>").unwrap();
            fs::set_permissions(&icon_path, fs::Permissions::from_mode(0o000)).unwrap();
            assert!(load_icon(icon_path.to_str().unwrap(), &[]).is_none());
            fs::set_permissions(&icon_path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    #[test]
    fn dotted_icon_names_resolve_exported_images_and_explicit_extensions() {
        let fixture = Fixture::new();
        let icons = fixture.0.join("icons/hicolor/64x64/apps");
        fs::create_dir_all(&icons).unwrap();
        let bytes = b"\x89PNG\r\n\x1a\nfixture";
        fs::write(icons.join("com.bitwarden.desktop.png"), bytes).unwrap();
        let locations = icon_locations(&[fixture.0.join("icons")]);
        for name in ["com.bitwarden.desktop", "com.bitwarden.desktop.png"] {
            assert!(
                matches!(load_icon(name, &locations), Some(Icon::Raster(loaded)) if loaded == bytes)
            );
        }
        fs::write(icons.join("org.example.App.svg"), b"<svg/>").unwrap();
        for name in ["org.example.App", "org.example.App.svg"] {
            assert!(matches!(load_icon(name, &locations), Some(Icon::Svg(_))));
        }
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlink_cycles_are_not_followed() {
        let fixture = Fixture::new();
        fixture.entry(
            "system",
            "one.desktop",
            "[Desktop Entry]\nType=Application\nName=One\nExec=true\n",
        );
        std::os::unix::fs::symlink(
            fixture.0.join("system/applications"),
            fixture.0.join("system/applications/loop"),
        )
        .unwrap();
        assert_eq!(fixture.scan().len(), 1);
    }
}
