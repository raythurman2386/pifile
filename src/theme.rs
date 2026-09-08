use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RgbaColor {
    pub fn luminance(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }
}

pub fn parse_hex_color(value: &str) -> Option<RgbaColor> {
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    let hex = value.strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(RgbaColor {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct OmarchyPalette {
    pub dark: bool,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub selection: String,
    /// Raised panel color (sidebar, toolbar, status bar): the theme's
    /// `lighter_background`, derived from background/foreground when absent.
    pub surface: String,
    /// Secondary text color: the theme's `muted`, derived when absent.
    pub muted: String,
}

impl OmarchyPalette {
    pub fn fallback(dark: bool) -> Self {
        if dark {
            Self {
                dark: true,
                background: "#101010".into(),
                foreground: "#eeeeee".into(),
                accent: "#5584aa".into(),
                selection: "#186a9a".into(),
                surface: "#181818".into(),
                muted: "#909191".into(),
            }
        } else {
            Self {
                dark: false,
                background: "#ffffff".into(),
                foreground: "#222324".into(),
                accent: "#2077b2".into(),
                selection: "#2077b2".into(),
                surface: "#f2f2f2".into(),
                muted: "#6b6f75".into(),
            }
        }
    }

    pub fn load(dark_hint: bool) -> Self {
        for path in colors_candidates() {
            if path.is_file() {
                return Self::from_colors_file(&path, dark_hint);
            }
        }
        Self::fallback(dark_hint)
    }

    pub fn from_colors_file(path: &Path, dark_hint: bool) -> Self {
        let mut palette = Self::fallback(dark_hint);
        let Ok(raw) = fs::read_to_string(path) else {
            return palette;
        };
        let mut mode = String::new();
        let mut saw_surface = false;
        let mut saw_muted = false;
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unquote(value.trim());
            match key {
                "mode" => mode = value,
                "background" | "COLOR_BASE" => palette.background = value,
                "foreground" | "COLOR_TEXT" => palette.foreground = value,
                "accent" | "COLOR_PRIMARY" => palette.accent = value,
                "selection" | "COLOR_SURFACE" => palette.selection = value,
                "lighter_background" => {
                    palette.surface = value;
                    saw_surface = true;
                }
                "muted" | "dark_foreground" | "COLOR_SUBTEXT" => {
                    palette.muted = value;
                    saw_muted = true;
                }
                _ => {}
            }
        }
        if mode == "dark" {
            palette.dark = true;
        } else if mode == "light" {
            palette.dark = false;
        } else if let Some(bg) = parse_hex_color(&palette.background) {
            palette.dark = bg.luminance() < 0.5;
        }
        palette.derive_missing(saw_surface, saw_muted);
        palette
    }

    /// Fill surface and muted from the base colors when the theme file does
    /// not provide them, so panels stay in the theme's own tonal family.
    fn derive_missing(&mut self, saw_surface: bool, saw_muted: bool) {
        let (Some(bg), Some(fg)) = (
            parse_hex_color(&self.background),
            parse_hex_color(&self.foreground),
        ) else {
            return;
        };
        if !saw_surface {
            // Surface: nudge the background ~12% toward the foreground.
            self.surface = mix_hex(&bg, &fg, 0.12);
        }
        if !saw_muted {
            // Muted text: foreground pulled ~40% back toward the background.
            self.muted = mix_hex(&fg, &bg, 0.4);
        }
    }
}

/// Mix two colors: `t` is the weight of the second color.
fn mix_hex(from: &RgbaColor, to: &RgbaColor, t: f32) -> String {
    let mix = |a: f32, b: f32| ((a * (1.0 - t) + b * t) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        mix(from.r, to.r),
        mix(from.g, to.g),
        mix(from.b, to.b)
    )
}

pub fn omarchy_colors_path() -> PathBuf {
    home_dir().join(".local/state/omarchy/current/theme/colors.toml")
}

pub fn colors_candidates() -> Vec<PathBuf> {
    let home = home_dir();
    let mut paths = Vec::new();
    if let Ok(dir) = std::env::var("PIFILE_THEME_DIR") {
        let dir = PathBuf::from(dir);
        paths.push(dir.join("colors.toml"));
        paths.push(dir.join("theme.conf"));
    }
    paths.push(omarchy_colors_path());
    paths.push(home.join(".local/state/omarchy/current/theme/theme.conf"));
    paths.push(home.join(".config/omarchy/current/theme/colors.toml"));
    paths.push(home.join(".config/omarchy/current/theme/theme.conf"));
    paths.push(home.join(".local/state/pimarchy/current/theme/colors.toml"));
    paths.push(home.join(".local/state/pimarchy/current/theme/theme.conf"));
    paths.push(home.join(".config/pimarchy/current/theme/colors.toml"));
    paths.push(home.join(".config/pimarchy/current/theme/theme.conf"));
    paths
}

pub fn omarchy_watch_paths() -> Vec<PathBuf> {
    let current = home_dir().join(".local/state/omarchy/current");
    vec![
        current.clone(),
        current.join("theme"),
        current.join("theme/colors.toml"),
    ]
}

pub fn detect_system_dark() -> bool {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            if raw.contains("prefer-dark") {
                return true;
            }
            if raw.contains("prefer-light") {
                return false;
            }
        }
    }
    true
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn loads_omarchy_theme() {
        let dir = std::env::temp_dir().join(format!("pifile-theme-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("colors.toml");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(
            b"mode = \"light\"\naccent = \"#112233\"\nselection = \"#445566\"\nbackground = \"#fefefe\"\nforeground = \"#101010\"\n",
        )
        .unwrap();
        let palette = OmarchyPalette::from_colors_file(&path, true);
        assert_eq!(palette.background, "#fefefe");
        assert_eq!(palette.foreground, "#101010");
        assert_eq!(palette.accent, "#112233");
        assert_eq!(palette.selection, "#445566");
        // No surface/muted keys in the file: derived from background/foreground.
        assert_eq!(palette.surface, "#e1e1e1");
        assert_eq!(palette.muted, "#6f6f6f");
        assert!(!palette.dark);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_surface_and_muted_keys() {
        let dir = std::env::temp_dir().join(format!("pifile-surface-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("colors.toml");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(
            b"mode = \"dark\"\nbackground = \"#222822\"\nforeground = \"#e8d5b7\"\naccent = \"#4ade80\"\nselection = \"#e8d5b7\"\nlighter_background = \"#2d3830\"\nmuted = \"#7f897d\"\n",
        )
        .unwrap();
        let palette = OmarchyPalette::from_colors_file(&path, false);
        assert_eq!(palette.surface, "#2d3830");
        assert_eq!(palette.muted, "#7f897d");
        assert!(palette.dark);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_theme_conf() {
        let dir = std::env::temp_dir().join(format!("pifile-conf-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("theme.conf");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"mode=dark\nCOLOR_BASE=#101010\nCOLOR_TEXT=#eeeeee\nCOLOR_PRIMARY=#5584aa\nCOLOR_SURFACE=#186a9a\n")
            .unwrap();
        let palette = OmarchyPalette::from_colors_file(&path, false);
        assert!(palette.dark);
        assert_eq!(palette.background, "#101010");
        assert_eq!(palette.foreground, "#eeeeee");
        assert_eq!(palette.accent, "#5584aa");
        assert_eq!(palette.selection, "#186a9a");
        let _ = fs::remove_dir_all(&dir);
    }
}
