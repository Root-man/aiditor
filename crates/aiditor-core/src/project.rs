//! Project files: a primary repo and additional roots, each with a role line.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub name: String,
    /// shortcut: any string is accepted; check it against the agent registry once there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    pub primary: Root,
    #[serde(default)]
    pub roots: Vec<Root>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub path: PathBuf,
    #[serde(default)]
    pub role: String,
}

impl Project {
    /// Parses a project file's text and checks what can be checked without the filesystem.
    pub fn parse(text: &str) -> Result<Self, String> {
        let project: Project = crate::settings::load("", Some(text))?;
        if project.name.trim().is_empty() {
            return Err("name is empty".into());
        }
        for root in project.all_roots() {
            if !root.path.is_absolute() {
                return Err(format!("{} is not absolute", root.path.display()));
            }
            if root.role.contains('\n') {
                return Err(format!(
                    "the role of {} is not one line",
                    root.path.display()
                ));
            }
        }
        // shortcut: paths are compared as written, so symlinks and `..` can hide an overlap; canonicalize in the services check if that bites.
        let roots: Vec<_> = project.all_roots().map(|r| &r.path).collect();
        for (i, a) in roots.iter().enumerate() {
            for b in &roots[..i] {
                let (inner, outer) = if a.starts_with(b) { (a, b) } else { (b, a) };
                if inner.starts_with(outer) {
                    return Err(format!("{} is inside {}", inner.display(), outer.display()));
                }
            }
        }
        Ok(project)
    }

    /// The primary root, then the additional ones.
    pub fn all_roots(&self) -> impl Iterator<Item = &Root> {
        std::iter::once(&self.primary).chain(&self.roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
name = "aiditor"
default_agent = "claude"

[primary]
path = "/r/aiditor"
role = "The editor itself"

[[roots]]
path = "/r/zed"
role = "GPUI source, read only"

[[roots]]
path = "/r/notes"
"#;

    #[test]
    fn full_file_parses_and_round_trips() {
        let p = Project::parse(FULL).unwrap();
        assert_eq!(p.name, "aiditor");
        assert_eq!(p.default_agent.as_deref(), Some("claude"));
        assert_eq!(p.primary.role, "The editor itself");
        let paths: Vec<_> = p.roots.iter().map(|r| r.path.to_str().unwrap()).collect();
        assert_eq!(paths, ["/r/zed", "/r/notes"]);
        assert_eq!(p.roots[1].role, "");
        let text = toml::to_string(&p).unwrap();
        assert_eq!(Project::parse(&text).unwrap(), p, "{text}");
    }

    #[test]
    fn minimal_file_round_trips() {
        let p = Project::parse("name = \"x\"\n[primary]\npath = \"/r/x\"").unwrap();
        assert_eq!((p.default_agent.as_ref(), p.roots.len()), (None, 0));
        assert_eq!(p.primary.role, "");
        assert_eq!(Project::parse(&toml::to_string(&p).unwrap()).unwrap(), p);
    }

    #[test]
    fn bad_files_are_rejected() {
        let primary = "[primary]\npath = \"/r/x\"";
        for (text, needle) in [
            ("name = ", "parse error"),
            (&format!("name = 1\n{primary}"), "name"),
            (
                &format!("name = \"x\"\ndefualt_agent = \"a\"\n{primary}"),
                "defualt_agent",
            ),
            (&format!("name = \" \"\n{primary}"), "name is empty"),
            ("name = \"x\"", "primary"),
            (primary, "name"),
            (
                "name = \"x\"\n[primary]\npath = \"src\"",
                "src is not absolute",
            ),
            (
                &format!("name = \"x\"\n{primary}\n[[roots]]\npath = \"../y\""),
                "../y is not absolute",
            ),
            (
                &format!("name = \"x\"\n{primary}\nrole = \"a\\nb\""),
                "not one line",
            ),
            (&format!("name = \"x\"\n{primary}\nrol = \"a\""), "rol"),
            (
                &format!("name = \"x\"\n{primary}\n[[roots]]\npath = \"/r/x/v/lib\""),
                "/r/x/v/lib is inside /r/x",
            ),
            (
                "name = \"x\"\n[primary]\npath = \"/r/x/v\"\n[[roots]]\npath = \"/r\"",
                "/r/x/v is inside /r",
            ),
            (
                &format!("name = \"x\"\n{primary}\n[[roots]]\npath = \"/r/x/\""),
                "is inside /r/x",
            ),
            (
                &format!("name = \"x\"\n{primary}\n[[roots]]\npath = \"/r/x\""),
                "/r/x is inside /r/x",
            ),
        ] {
            let err = Project::parse(text).unwrap_err();
            assert!(err.contains(needle), "{text:?}: {err}");
        }
    }

    #[test]
    fn sibling_with_shared_prefix_loads() {
        let text = "name = \"x\"\n[primary]\npath = \"/r/app\"\n[[roots]]\npath = \"/r/app2\"";
        assert!(Project::parse(text).is_ok());
    }
}
