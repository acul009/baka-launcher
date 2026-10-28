//! Deterministic launch ordering shared by the grid and Enter activation.

use crate::Entry;

/// Returns the exact entry indexes to display and activate, without duplicates.
pub fn order(entries: &[Entry], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut ranked = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let name = entry.name.to_lowercase();
        let score = if query.is_empty() {
            pin(entry).unwrap_or(5)
        } else if name == query {
            0
        } else if name.starts_with(&query) {
            1
        } else if name.contains(&query) {
            2
        } else {
            let metadata = format!(
                "{} {} {}",
                entry.generic_name,
                entry.keywords.join(" "),
                entry.id
            )
            .to_lowercase();
            if query
                .split_whitespace()
                .all(|word| name.contains(word) || metadata.contains(word))
            {
                3
            } else {
                continue;
            }
        };
        ranked.push((score, name, &entry.id, index));
    }
    ranked.sort_unstable();
    ranked.into_iter().map(|(_, _, _, index)| index).collect()
}

/// Whether this installed application belongs in the default pinned section.
pub fn is_pinned(entry: &Entry) -> bool {
    pin(entry).is_some()
}

fn pin(entry: &Entry) -> Option<u8> {
    match entry.id.to_lowercase().as_str() {
        "google-chrome.desktop" | "google-chrome-stable.desktop" | "com.google.chrome.desktop" => {
            Some(0)
        }
        "brave-browser.desktop" | "brave-browser-stable.desktop" | "com.brave.browser.desktop" => {
            Some(1)
        }
        "code.desktop" | "visual-studio-code.desktop" | "com.visualstudio.code.desktop" => Some(2),
        "org.gnome.nautilus.desktop"
        | "nautilus.desktop"
        | "org.kde.dolphin.desktop"
        | "thunar.desktop"
        | "cosmic-files.desktop"
        | "com.system76.cosmicfiles.desktop" => Some(3),
        "obsidian.desktop" | "md.obsidian.obsidian.desktop" => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_pinned, order};
    use crate::Entry;
    use std::path::PathBuf;

    fn entry(id: &str, name: &str, extra: &str) -> Entry {
        Entry::parse(
            id.into(),
            PathBuf::from(id),
            &format!("[Desktop Entry]\nType=Application\nName={name}\nExec=app\n{extra}"),
            "C",
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn empty_query_orders_pins_then_alphabetical_apps_without_duplicates() {
        let entries = [
            entry("z.desktop", "Zulu", ""),
            entry("obsidian.desktop", "Obsidian", ""),
            entry("a.desktop", "Alpha", ""),
            entry("google-chrome.desktop", "Chrome", ""),
            entry("org.gnome.Nautilus.desktop", "Files", ""),
        ];
        assert_eq!(order(&entries, ""), [3, 4, 1, 2, 0]);
        assert_eq!(order(&entries, "  "), [3, 4, 1, 2, 0]);
        assert!(is_pinned(&entries[4]));
        assert!(!is_pinned(&entries[0]));
    }

    #[test]
    fn strong_unpinned_match_precedes_weak_pinned_match() {
        let entries = [
            entry("google-chrome.desktop", "Chrome Browser", ""),
            entry("other.desktop", "Browser", ""),
        ];
        assert_eq!(order(&entries, "browser"), [1, 0]);
    }

    #[test]
    fn query_rank_is_case_insensitive_and_searches_metadata_and_id() {
        let entries = [
            entry(
                "calculator.desktop",
                "Calculate",
                "GenericName=Calculator\n",
            ),
            entry("org.text.desktop", "Editor", "Keywords=Writing;Notes;\n"),
            entry("org.browser.desktop", "Web", ""),
        ];
        assert_eq!(order(&entries, "cALc"), [0]);
        assert_eq!(order(&entries, "notes"), [1]);
        assert_eq!(order(&entries, "browser"), [2]);
        assert!(order(&entries, "unmatched").is_empty());
    }

    #[test]
    fn rank_ties_use_name_then_desktop_id_instead_of_scan_order() {
        let entries = [
            entry("z.desktop", "Same", ""),
            entry("a.desktop", "Same", ""),
        ];
        assert_eq!(order(&entries, "same"), [1, 0]);
    }
}
