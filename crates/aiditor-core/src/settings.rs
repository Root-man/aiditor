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
    pub theme: Theme,
}

/// Colors as 0xRRGGBB.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub background: u32,
    pub foreground: u32,
    pub cursor: u32,
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
    }

    #[test]
    fn user_keys_override_and_the_rest_stay_default() {
        let s = Settings::parse(Some("font_size = 18\n[theme]\ncursor = 0x00ff00")).unwrap();
        assert_eq!(s.font_size, 18.0);
        assert_eq!(s.theme.cursor, 0x00ff00);
        assert_eq!(s.theme.background, Settings::default().theme.background);
    }

    #[test]
    fn bad_files_are_rejected() {
        for (user, needle) in [
            ("font_size = \"big\"", "font_size"),
            ("font_sise = 16", "font_sise"),
            ("[theme]\nbackgrund = 1", "backgrund"),
            ("font_size = 0", "outside"),
            ("font_size = ", ""),
        ] {
            let err = Settings::parse(Some(user)).unwrap_err();
            assert!(err.contains(needle), "{user:?}: {err}");
        }
    }
}
