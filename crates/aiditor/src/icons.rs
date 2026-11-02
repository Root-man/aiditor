//! The bundled line icons, drawn from Lucide (ISC, see `resources/icons/LICENSE`).

use std::borrow::Cow;

use aiditor_core::buffer::Icon;
use gpui::{AssetSource, Result, SharedString};

/// `icon`'s asset path.
pub(crate) fn path(icon: Icon) -> &'static str {
    match icon {
        Icon::Book => "icons/book.svg",
        Icon::Magnifier => "icons/magnifier.svg",
        Icon::Pencil => "icons/pencil.svg",
        Icon::Terminal => "icons/terminal.svg",
        Icon::Globe => "icons/globe.svg",
        Icon::Wrench => "icons/wrench.svg",
    }
}

/// The app's assets, in the binary so that it runs from anywhere.
pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let svg: &[u8] = match path {
            "icons/book.svg" => include_bytes!("../resources/icons/book.svg"),
            "icons/magnifier.svg" => include_bytes!("../resources/icons/magnifier.svg"),
            "icons/pencil.svg" => include_bytes!("../resources/icons/pencil.svg"),
            "icons/terminal.svg" => include_bytes!("../resources/icons/terminal.svg"),
            "icons/globe.svg" => include_bytes!("../resources/icons/globe.svg"),
            "icons/wrench.svg" => include_bytes!("../resources/icons/wrench.svg"),
            _ => return Ok(None),
        };
        Ok(Some(svg.into()))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_bundled() {
        let icons = [
            Icon::Book,
            Icon::Magnifier,
            Icon::Pencil,
            Icon::Terminal,
            Icon::Globe,
            Icon::Wrench,
        ];
        for icon in icons {
            let svg = Assets.load(path(icon)).unwrap().expect("bundled");
            let svg = String::from_utf8_lossy(&svg);
            assert!(svg.starts_with("<svg"), "{icon:?}");
            assert!(
                !svg.contains("<script") && !svg.contains("href"),
                "{icon:?}"
            );
        }
    }
}
