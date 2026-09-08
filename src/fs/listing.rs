use std::fs;
use std::path::{Path, PathBuf};

use super::entry::{DirEntry, FileKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
}

pub fn list_dir(path: &Path, show_hidden: bool, sort: SortKey) -> std::io::Result<Vec<DirEntry>> {
    let mut entries = Vec::new();
    for dent in fs::read_dir(path)? {
        let dent = match dent {
            Ok(d) => d,
            Err(_) => continue,
        };
        let path = dent.path();
        let meta = match dent.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let entry = DirEntry::from_meta(path, &meta);
        if !show_hidden && entry.hidden {
            continue;
        }
        entries.push(entry);
    }
    entries.sort_by(
        |a, b| match (a.kind == FileKind::Directory, b.kind == FileKind::Directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => match sort {
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
                SortKey::Modified => a.modified.cmp(&b.modified),
            },
        },
    );
    Ok(entries)
}

pub fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// A sidebar location: label, icon name, and target path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub label: String,
    pub icon: String,
    pub path: PathBuf,
}

/// Sidebar entries: `(label, icon, path)`. Standard XDG user dirs come from
/// `xdg-user-dir` when available (respects localization), falling back to the
/// conventional names; every entry is skipped when the directory does not
/// exist, except Root.
pub fn places() -> Vec<Place> {
    let home = home_dir();
    let mut items = Vec::new();

    let mut push = |label: &str, icon: &str, path: PathBuf| {
        if path.is_dir() {
            items.push(Place {
                label: label.into(),
                icon: icon.into(),
                path,
            });
        }
    };

    push("Home", "user", home.clone());
    push("Desktop", "folder-closed", user_dir("DESKTOP", "Desktop"));
    push("Documents", "file-text", user_dir("DOCUMENTS", "Documents"));
    push("Downloads", "arrow-down", user_dir("DOWNLOAD", "Downloads"));
    push("Music", "star", user_dir("MUSIC", "Music"));
    push("Pictures", "palette", user_dir("PICTURES", "Pictures"));
    push("Videos", "play", user_dir("VIDEOS", "Videos"));
    items.push(Place {
        label: "Root".into(),
        icon: "hard-drive".into(),
        path: PathBuf::from("/"),
    });
    items
}

fn user_dir(name: &str, fallback: &str) -> PathBuf {
    let dir = home_dir();
    match std::process::Command::new("xdg-user-dir")
        .arg(name)
        .output()
    {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let raw = raw.trim();
            let path = PathBuf::from(raw);
            // Unset entries echo the home directory itself.
            if raw.is_empty() || path == dir {
                dir.join(fallback)
            } else {
                path
            }
        }
        _ => dir.join(fallback),
    }
}

pub fn initial_cwd() -> PathBuf {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        let path = PathBuf::from(arg);
        if path.is_dir() {
            return path;
        }
        if let Some(parent) = path.parent() {
            if parent.is_dir() {
                return parent.to_path_buf();
            }
        }
    }
    std::env::current_dir()
        .ok()
        .filter(|p| p.is_dir())
        .unwrap_or_else(home_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_unique_dirs() {
        let items = places();
        assert!(
            items.iter().any(|p| p.path.as_path() == Path::new("/")),
            "Root must always be present"
        );
        let mut seen = std::collections::HashSet::new();
        for place in &items {
            assert!(place.path.is_dir(), "{} must exist", place.path.display());
            assert!(
                seen.insert(place.path.clone()),
                "duplicate place {}",
                place.path.display()
            );
            assert!(!place.icon.is_empty());
            assert!(!place.label.is_empty());
        }
    }

    #[test]
    fn user_dir_falls_back_to_conventional_name() {
        // A non-existent xdg-user-dir binary must not panic.
        std::env::set_var("PATH", "/nonexistent");
        let dir = home_dir();
        assert_eq!(user_dir("MUSIC", "Music"), dir.join("Music"));
    }
}
