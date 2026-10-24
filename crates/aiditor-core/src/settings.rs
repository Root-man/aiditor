//! TOML settings: bundled defaults with the user's file merged over them.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use toml::Table;

pub const BUNDLED: &str = include_str!("../settings.toml");

/// Parses `bundled`, merges `user` over it key by key and deserializes the result.
pub fn load<T: DeserializeOwned>(bundled: &str, user: Option<&str>) -> Result<T, String> {
    let mut table: Table = toml::from_str(bundled).map_err(|e| e.to_string())?;
    if let Some(user) = user {
        merge(&mut table, toml::from_str(user).map_err(|e| e.to_string())?);
    }
    table.try_into().map_err(|e: toml::de::Error| e.to_string())
}

/// Tables merge recursively; any other value replaces the default.
fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// `None` means the platform's monospace font.
    pub font_family: Option<String>,
    pub font_size: f32,
    /// Option is Meta on macOS; false leaves it for characters and dead keys. Linux's Alt is always Meta.
    pub option_as_meta: bool,
    pub max_watched_files: usize,
    pub theme: Theme,
}

/// Colors as 0xRRGGBB.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub background: u32,
    pub foreground: u32,
    pub cursor: u32,
    /// The region's background.
    pub region: u32,
    /// The selected window's mode line background.
    pub mode_line: u32,
    /// Every other window's mode line background.
    pub mode_line_inactive: u32,
    /// Terminals' ANSI colours 0 to 15.
    pub ansi: [u32; 16],
}

impl Settings {
    /// The bundled defaults with the user's `settings.toml` text, if any, over them.
    pub fn parse(user: Option<&str>) -> Result<Self, String> {
        let settings: Settings = load(BUNDLED, user)?;
        if !(6.0..=72.0).contains(&settings.font_size) {
            return Err(format!(
                "font_size {} is outside 6 to 72",
                settings.font_size
            ));
        }
        Ok(settings)
    }

    /// The height of one line of text in px.
    pub fn line_height(&self) -> f32 {
        self.font_size * 1.4
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings::parse(None).expect("bundled settings are valid")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_defaults_parse() {
        let s = Settings::default();
        assert_eq!(s.font_family, None);
        assert_eq!(s.font_size, 14.0);
        assert_eq!(s.theme.cursor, 0xffcc00);
        assert!(s.option_as_meta);
        assert_eq!(s.max_watched_files, 10000);
        assert_eq!(
            Settings::parse(Some("max_watched_files = 0"))
                .unwrap()
                .max_watched_files,
            0
        );
    }

    #[test]
    fn user_keys_override_and_the_rest_stay_default() {
        let s = Settings::parse(Some(
            "font_size = 18\n[theme]\ncursor = 0x00ff00\nregion = 0x123456\nmode_line = 0x111111",
        ))
        .unwrap();
        assert_eq!(s.font_size, 18.0);
        assert_eq!(s.theme.cursor, 0x00ff00);
        assert_eq!(s.theme.region, 0x123456);
        assert_eq!(s.theme.mode_line, 0x111111);
        let inactive = Settings::default().theme.mode_line_inactive;
        assert_eq!(s.theme.mode_line_inactive, inactive);
        assert_eq!(s.theme.background, Settings::default().theme.background);
        let ansi = (0..16).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let s = Settings::parse(Some(&format!("[theme]\nansi = [{ansi}]"))).unwrap();
        assert_eq!(s.theme.ansi[1], 1);
    }

    #[test]
    fn bad_files_are_rejected() {
        for (user, needle) in [
            ("font_size = \"big\"", "font_size"),
            ("font_sise = 16", "font_sise"),
            ("[theme]\nbackgrund = 1", "backgrund"),
            ("font_size = 0", "outside"),
            ("font_size = ", ""),
            ("max_watched_files = -1", "max_watched_files"),
            ("max_watched_files = \"many\"", "max_watched_files"),
        ] {
            let err = Settings::parse(Some(user)).unwrap_err();
            assert!(err.contains(needle), "{user:?}: {err}");
        }
    }
}
