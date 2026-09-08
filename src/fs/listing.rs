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

pub fn places() -> Vec<(String, PathBuf)> {
    let mut items = Vec::new();
    let home = home_dir();
    push_if_dir(&mut items, "Home", home.clone());
    push_if_dir(&mut items, "Downloads", home.join("Downloads"));
    push_if_dir(&mut items, "Documents", home.join("Documents"));
    push_if_dir(&mut items, "Desktop", home.join("Desktop"));
    items.push(("Root".into(), PathBuf::from("/")));
    items
}

fn push_if_dir(items: &mut Vec<(String, PathBuf)>, label: &str, path: PathBuf) {
    if path.is_dir() {
        items.push((label.into(), path));
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
