//! Custom icon and asset plumbing.
//!
//! The icon set bundled with gpui-kit-assets 0.6.0 has no home/desktop/
//! media-folder icons, so the sidebar used loose stand-ins. We ship the
//! official Lucide icons ourselves and serve them alongside the bundled
//! set: `PlaceIcon` implements `IconNamed` (the documented drop-in
//! extension point for `IconName`) and `PifileAssets` delegates to
//! gpui-kit's asset source, answering `icons/<name>.svg` from the
//! embedded copies.

use gpui_kit::component::IconNamed;
use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;

/// Lucide icons used by the places sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceIcon {
    House,
    Monitor,
    FileText,
    ArrowDown,
    Music,
    Image,
    MonitorPlay,
    HardDrive,
}

impl IconNamed for PlaceIcon {
    fn path(self) -> SharedString {
        match self {
            Self::House => "icons/house.svg",
            Self::Monitor => "icons/monitor.svg",
            Self::FileText => "icons/file-text.svg",
            Self::ArrowDown => "icons/arrow-down.svg",
            Self::Music => "icons/music.svg",
            Self::Image => "icons/image.svg",
            Self::MonitorPlay => "icons/monitor-play.svg",
            Self::HardDrive => "icons/hard-drive.svg",
        }
        .into()
    }
}

/// Asset source that serves the bundled gpui-kit icons plus `icons/*.svg`.
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*.svg"]
pub struct PifileAssets;

impl AssetSource for PifileAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("icons/") && path.ends_with(".svg") {
            if let Some(data) = Self::get(path) {
                return Ok(Some(data.data));
            }
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
