use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

pub fn validate_name(name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("name is empty");
    }
    if name.contains('/') || name.contains('\0') {
        bail!("name must be a single path component");
    }
    if name == "." || name == ".." {
        bail!("invalid name");
    }
    Ok(())
}

pub fn mkdir(parent: &Path, name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    let dest = parent.join(name.trim());
    fs::create_dir(&dest).with_context(|| format!("mkdir {}", dest.display()))?;
    Ok(dest)
}

pub fn rename(from: &Path, new_name: &str) -> Result<PathBuf> {
    validate_name(new_name)?;
    let dest = from
        .parent()
        .map(|p| p.join(new_name.trim()))
        .ok_or_else(|| anyhow::anyhow!("no parent for {}", from.display()))?;
    if dest == from {
        return Ok(dest);
    }
    if dest.exists() {
        bail!("{} already exists", dest.display());
    }
    fs::rename(from, &dest)
        .with_context(|| format!("rename {} -> {}", from.display(), dest.display()))?;
    Ok(dest)
}

pub fn trash_path(path: &Path) -> Result<()> {
    trash::delete(path).with_context(|| format!("trash {}", path.display()))
}

pub fn delete_permanent(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("rm -r {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("rm {}", path.display()))
    }
}

pub fn copy_file(from: &Path, to_dir: &Path) -> Result<PathBuf> {
    let name = from
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("no file name"))?;
    let dest = unique_dest(to_dir, name);
    if from.is_dir() {
        copy_dir_recursive(from, &dest)?;
    } else {
        fs::copy(from, &dest)
            .with_context(|| format!("copy {} -> {}", from.display(), dest.display()))?;
    }
    Ok(dest)
}

pub fn move_path(from: &Path, to_dir: &Path) -> Result<PathBuf> {
    let name = from
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("no file name"))?;
    let dest = unique_dest(to_dir, name);
    match fs::rename(from, &dest) {
        Ok(()) => Ok(dest),
        Err(_) => {
            let dest = copy_file(from, to_dir)?;
            delete_permanent(from)?;
            Ok(dest)
        }
    }
}

fn unique_dest(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }
    let stem = Path::new(name)
        .file_stem()
        .unwrap_or(name)
        .to_string_lossy();
    let ext = Path::new(name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for i in 1..1000 {
        let candidate = dir.join(format!("{stem} ({i}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dest
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for dent in fs::read_dir(from)? {
        let dent = dent?;
        let src = dent.path();
        let dest = to.join(dent.file_name());
        if src.is_dir() {
            copy_dir_recursive(&src, &dest)?;
        } else {
            fs::copy(&src, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("").is_err());
        assert!(validate_name("..").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("ok").is_ok());
    }
}
