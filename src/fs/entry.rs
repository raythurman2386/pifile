use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub hidden: bool,
}

impl DirEntry {
    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        let meta = fs::symlink_metadata(&path)?;
        Ok(Self::from_meta(path, &meta))
    }

    pub fn from_meta(path: PathBuf, meta: &Metadata) -> Self {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let hidden = name.starts_with('.');
        let kind = if meta.is_symlink() {
            FileKind::Symlink
        } else if meta.is_dir() {
            FileKind::Directory
        } else if meta.is_file() {
            FileKind::File
        } else {
            FileKind::Other
        };
        let size = if kind == FileKind::File {
            Some(meta.len())
        } else {
            None
        };
        let modified = meta.modified().ok();
        Self {
            path,
            name,
            kind,
            size,
            modified,
            hidden,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == FileKind::Directory
    }

    pub fn size_label(&self) -> String {
        match self.size {
            Some(n) => format_size(n),
            None => "—".into(),
        }
    }

    pub fn modified_label(&self) -> String {
        let Some(ts) = self.modified else {
            return "—".into();
        };
        let dt = chrono::DateTime::<chrono::Local>::from(ts);
        dt.format("%Y-%m-%d %H:%M").to_string()
    }
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut n = bytes as f64;
    let mut unit = 0;
    while n >= 1024.0 && unit < UNITS.len() - 1 {
        n /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{n:.1} {}", UNITS[unit])
    }
}

pub fn parent_of(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}
