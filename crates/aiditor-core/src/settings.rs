//! TOML settings: bundled defaults with the user's file merged over them.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use toml::Table;

use crate::buffer::{Face, FaceRef};

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
    /// Files project grep searches at once.
    pub grep_workers: NonZeroUsize,
    /// The file's `theme`: which of `themes` is drawn.
    #[serde(rename = "theme")]
    pub theme_name: String,
    /// Themes by name, as written; each lists only what it changes from `default`.
    pub themes: BTreeMap<String, Table>,
    /// `themes.default` with the selected theme over it.
    #[serde(skip)]
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

/// The faces the design names, spelled in kebab-case in `settings.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FaceName {
    Default,
    Cursor,
    Echo,
    Separator,
    ModeLine,
    ModeLineInactive,
    ModeLineDim,
    WorkspaceBar,
    WorkspaceCurrent,
    Divider,
    LineNumber,
    LineNumberCurrent,
    HlLine,
    Region,
    Match,
    MatchCurrent,
    CompletionMatch,
    CompletionSelected,
    Annotation,
    Prompt,
    Directory,
    GrepFile,
    GrepLineNumber,
    VcsBranch,
    StatusModified,
    StatusWaiting,
    StatusRunning,
    StatusFailed,
    StatusInfo,
    SessionUser,
    SessionThought,
    SessionBlock,
    SessionCode,
    DiffAdded,
    DiffRemoved,
    DiffHunk,
    KeyHint,
    Permission,
    Link,
    Attachment,
    MessageAtPoint,
}

/// A resolved theme: `themes.default` with the selected one merged over it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Terminals' ANSI colours 0 to 15.
    pub ansi: [u32; 16],
    pub faces: BTreeMap<FaceName, Face>,
    /// Code's faces by tree-sitter capture name; `function.method` falls back to `function`.
    pub syntax: BTreeMap<String, Face>,
}

impl Theme {
    /// `name`'s face; the bundled theme defines all of them.
    pub fn face(&self, name: FaceName) -> Face {
        self.faces.get(&name).copied().unwrap_or_default()
    }

    /// `refs` over each other, later ones on top.
    pub(crate) fn resolve(&self, refs: &[FaceRef]) -> Face {
        (refs.iter()).fold(Face::default(), |under, r| {
            let face = match *r {
                FaceRef::Face(name) => self.face(name),
                FaceRef::Syntax(name) => {
                    crate::syntax::face(&self.syntax, name).unwrap_or_default()
                }
                FaceRef::Fg(i) => Face {
                    fg: Some(self.ansi[i as usize]),
                    ..Face::default()
                },
                FaceRef::Bg(i) => Face {
                    bg: Some(self.ansi[i as usize]),
                    ..Face::default()
                },
                FaceRef::Bold => Face {
                    bold: true,
                    ..Face::default()
                },
                FaceRef::Italic => Face {
                    italic: true,
                    ..Face::default()
                },
                FaceRef::Underline => Face {
                    underline: true,
                    ..Face::default()
                },
            };
            face.over(under)
        })
    }

    /// The `default` face's text colour.
    pub fn foreground(&self) -> u32 {
        self.face(FaceName::Default).fg.unwrap_or_default()
    }

    /// The `default` face's background.
    pub fn background(&self) -> u32 {
        self.face(FaceName::Default).bg.unwrap_or_default()
    }

    /// The `cursor` face's background.
    pub fn cursor(&self) -> u32 {
        self.face(FaceName::Cursor).bg.unwrap_or_default()
    }
}

impl Settings {
    /// The bundled defaults with the user's `settings.toml` text, if any, over them.
    pub fn parse(user: Option<&str>) -> Result<Self, String> {
        let mut settings: Settings = load(BUNDLED, user)?;
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
        let default = settings.themes.get("default").cloned().unwrap_or_default();
        for (name, theme) in &settings.themes {
            let mut merged = default.clone();
            merge(&mut merged, theme.clone());
            let merged: Theme =
                (merged.try_into()).map_err(|e: toml::de::Error| format!("themes.{name}: {e}"))?;
            if *name == settings.theme_name {
                settings.theme = merged;
            }
        }
        if !settings.themes.contains_key(&settings.theme_name) {
            let name = &settings.theme_name;
            return Err(format!("theme {name:?}: no [themes.{name}] table"));
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
        assert_eq!(s.theme.cursor(), 0xffcc00);
        assert!(s.option_as_meta);
        assert_eq!(s.max_watched_files, 10000);
        assert_eq!(s.grep_workers.get(), 1);
        let s = Settings::parse(Some("grep_workers = 8")).unwrap();
        assert_eq!(s.grep_workers.get(), 8);
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
            "font_size = 18\n[themes.default.faces]\nregion = { bg = 0x123456 }\nmode-line = { bg = 0x111111 }",
        ))
        .unwrap();
        let bundled = Settings::default().theme;
        assert_eq!(s.font_size, 18.0);
        assert_eq!(s.theme.face(FaceName::Region).bg, Some(0x123456));
        let line = s.theme.face(FaceName::ModeLine);
        assert_eq!(line.bg, Some(0x111111));
        assert_eq!(line.fg, bundled.face(FaceName::ModeLine).fg);
        let inactive = FaceName::ModeLineInactive;
        assert_eq!(s.theme.face(inactive), bundled.face(inactive));
        assert_eq!(s.theme.background(), bundled.background());
        let ansi = (0..16).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let s = Settings::parse(Some(&format!("[themes.default]\nansi = [{ansi}]"))).unwrap();
        assert_eq!(s.theme.ansi[1], 1);
        let s = Settings::parse(Some(
            "[themes.default.syntax]\nkeyword = { fg = 0xff0000 }\nfoo = { fg = 1, italic = true }",
        ))
        .unwrap();
        assert_eq!(s.theme.syntax["keyword"].fg, Some(0xff0000));
        assert_eq!(s.theme.syntax["string"], bundled.syntax["string"]);
        assert!(s.theme.syntax["foo"].italic);
    }

    #[test]
    fn the_theme_key_picks_a_table_merged_over_default() {
        let s = Settings::parse(Some(
            "theme = \"mine\"\n[themes.mine.faces]\ndefault = { bg = 1 }",
        ))
        .unwrap();
        let bundled = Settings::default().theme;
        assert_eq!(s.theme.background(), 1);
        assert_eq!(s.theme.foreground(), bundled.foreground());
        assert_eq!(s.theme.ansi, bundled.ansi);
        let name = FaceName::SessionBlock;
        assert_eq!(s.theme.face(name), bundled.face(name));
    }

    #[test]
    fn every_face_is_bundled_and_read_by_name() {
        let names = [
            "default",
            "cursor",
            "echo",
            "separator",
            "mode-line",
            "mode-line-inactive",
            "mode-line-dim",
            "workspace-bar",
            "workspace-current",
            "divider",
            "line-number",
            "line-number-current",
            "hl-line",
            "region",
            "match",
            "match-current",
            "completion-match",
            "completion-selected",
            "annotation",
            "prompt",
            "directory",
            "grep-file",
            "grep-line-number",
            "vcs-branch",
            "status-modified",
            "status-waiting",
            "status-running",
            "status-failed",
            "status-info",
            "session-user",
            "session-thought",
            "session-block",
            "session-code",
            "diff-added",
            "diff-removed",
            "diff-hunk",
            "key-hint",
            "permission",
            "link",
            "attachment",
            "message-at-point",
        ];
        let theme = Settings::default().theme;
        assert_eq!(theme.faces.len(), names.len());
        // A theme can't unset what it inherits, and inline code sits on whatever is under it.
        assert_eq!(theme.face(FaceName::SessionCode).bg, None);
        for name in names {
            let parsed = toml::Value::String(name.into()).try_into().unwrap();
            assert!(theme.faces.contains_key(&parsed), "{name}");
        }
    }

    #[test]
    fn faces_stack_with_unset_attributes_taking_what_is_under() {
        let s = Settings::parse(Some(
            "[themes.default.faces]\nstatus-failed = { fg = 7 }\nmode-line = { fg = 8, bg = 9 }",
        ))
        .unwrap();
        let (bar, failed) = (
            FaceRef::Face(FaceName::ModeLine),
            FaceRef::Face(FaceName::StatusFailed),
        );
        let face = s.theme.resolve(&[bar, failed, FaceRef::Bold]);
        assert_eq!((face.fg, face.bg, face.bold), (Some(7), Some(9), true));
    }

    #[test]
    fn bundled_claude_agent_and_user_overrides_or_additions() {
        let claude = &Settings::default().agents["claude"];
        assert_eq!(claude.command, "claude-agent-acp");
        assert!(claude.args.is_empty());
        let env: Vec<_> = (claude.env.iter())
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        assert_eq!(
            env,
            [
                "CLAUDE_CODE_ENABLE_TASKS=1",
                "CLAUDE_CODE_ENABLE_TODO_TOOLS=1"
            ]
        );
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
            ("theme = \"nope\"", "nope"),
            ("[theme]\nbackground = 1", "theme"),
            ("[themes.other.faces]\nmode-lin = { fg = 1 }", "mode-lin"),
            (
                "[themes.other.faces]\nregion = { background = 1 }",
                "background",
            ),
            ("[themes.other.syntax]\nkeyword = 1", "themes.other"),
            ("[themes.other]\nbackground = 1", "background"),
            ("font_size = 0", "outside"),
            ("font_size = ", ""),
            ("max_watched_files = -1", "max_watched_files"),
            ("max_watched_files = \"many\"", "max_watched_files"),
            ("grep_workers = 0", "grep_workers"),
            ("grep_workers = -1", "grep_workers"),
            ("grep_workers = \"many\"", "grep_workers"),
            ("grep_workers = 1.5", "grep_workers"),
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
