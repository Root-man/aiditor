//! TOML settings: bundled defaults with the user's file merged over them.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
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

/// Checks the variables of an `env` table can be set on a process.
pub(crate) fn check_env(env: &BTreeMap<String, String>) -> Result<(), String> {
    for (name, value) in env {
        if name.is_empty() || name.contains(['=', '\0']) {
            return Err(format!("env {name:?}: not a variable name"));
        }
        if value.contains('\0') {
            return Err(format!("env {name}: the value holds a NUL"));
        }
    }
    Ok(())
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
    /// The agents sessions can start, by name.
    pub agents: BTreeMap<String, Agent>,
}

/// How to start an agent: its ACP adapter's command line and environment.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Variables set on top of the captured environment, under the project file's.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// The session mode to switch to once the agent advertises its modes.
    pub default_mode: Option<String>,
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
    /// Code's colours by tree-sitter capture name; `function.method` falls back to `function`.
    pub syntax: BTreeMap<String, u32>,
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
        for (name, agent) in &settings.agents {
            if agent.command.trim().is_empty() {
                return Err(format!("agents.{name}: command is empty"));
            }
            check_env(&agent.env).map_err(|err| format!("agents.{name}: {err}"))?;
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
        let s = Settings::parse(Some("[theme.syntax]\nkeyword = 0xff0000\nfoo = 1")).unwrap();
        let bundled = Settings::default().theme.syntax;
        assert_eq!(s.theme.syntax["keyword"], 0xff0000);
        assert_eq!(
            (s.theme.syntax["string"], s.theme.syntax["foo"]),
            (bundled["string"], 1)
        );
    }

    #[test]
    fn bundled_claude_agent_and_user_overrides_or_additions() {
        let claude = &Settings::default().agents["claude"];
        assert_eq!(claude.command, "claude-agent-acp");
        assert!(claude.args.is_empty() && claude.env.is_empty());
        assert_eq!(claude.default_mode.as_deref(), Some("auto"));
        let s = Settings::parse(Some(
            "[agents.claude]\ncommand = \"/opt/claude\"\n[agents.codex]\ncommand = \"codex-acp\"\nargs = [\"-v\"]",
        ))
        .unwrap();
        assert_eq!(s.agents["claude"].command, "/opt/claude");
        assert_eq!(s.agents["claude"].default_mode.as_deref(), Some("auto"));
        assert_eq!(s.agents["codex"].args, ["-v"]);
        assert_eq!(s.agents["codex"].default_mode, None);
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
            ("[agents.claude]\ncomand = \"x\"", "agents.claude"),
            ("[agents.mine]\nargs = []", "agents.mine"),
            (
                "[agents.mine]\ncommand = \"\"",
                "agents.mine: command is empty",
            ),
            (
                "[agents.mine]\ncommand = \"  \"",
                "agents.mine: command is empty",
            ),
            ("[agents.claude]\nargs = \"-v\"", "args"),
            (
                "[agents.claude.env]\n\"A=B\" = \"c\"",
                "agents.claude: env \"A=B\"",
            ),
            (
                "[agents.claude.env]\nA = \"\\u0000\"",
                "agents.claude: env A",
            ),
        ] {
            let err = Settings::parse(Some(user)).unwrap_err();
            assert!(err.contains(needle), "{user:?}: {err}");
        }
    }
}
