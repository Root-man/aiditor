//! Project files: a primary repo and additional roots, each with a role line.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, TableLike, Value};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub name: String,
    #[serde(default = "watch_default", skip_serializing_if = "is_true")]
    pub watch: bool,
    /// shortcut: any string is accepted; check it against the agent registry once there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    /// Gitignore-syntax patterns, relative to each root, leaving files out of the index, grep
    /// and the watcher; `aiditor-services` checks them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
    pub primary: Root,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<Root>,
}

fn watch_default() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub role: String,
}

/// A project and the file it lives in, with the text last read from or written to it; no file
/// while it is ad-hoc.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub project: Project,
    pub file: Option<(PathBuf, String)>,
}

impl Loaded {
    /// An ad-hoc project named `name` with `root` as its primary.
    pub(crate) fn ad_hoc(name: &str, root: &Path) -> Self {
        let primary = Root {
            path: root.to_path_buf(),
            role: String::new(),
        };
        let project = Project {
            name: name.into(),
            watch: std::env::home_dir().as_deref() != Some(root),
            default_agent: None,
            ignore: Vec::new(),
            primary,
            roots: Vec::new(),
        };
        Loaded {
            project,
            file: None,
        }
    }
}

impl Project {
    /// Parses a project file's text and checks what can be checked without the filesystem.
    pub fn parse(text: &str) -> Result<Self, String> {
        let project: Project = crate::settings::load("", Some(text))?;
        project.check()?;
        Ok(project)
    }

    /// The checks `parse` runs after reading the text.
    pub fn check(&self) -> Result<(), String> {
        let project = self;
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
        Ok(())
    }

    /// The primary root, then the additional ones.
    pub fn all_roots(&self) -> impl Iterator<Item = &Root> {
        std::iter::once(&self.primary).chain(&self.roots)
    }
}

/// Changes `doc`, a project file, to hold `project`, setting only what differs so the file's
/// comments and layout stay.
pub fn sync(doc: &mut DocumentMut, project: &Project) {
    set(doc.as_table_mut(), "name", &project.name);
    match doc.get_mut("watch") {
        Some(Item::Value(old)) if old.as_bool() != Some(project.watch) => {
            let decor = old.decor().clone();
            *old = project.watch.into();
            *old.decor_mut() = decor;
        }
        None if !project.watch => doc["watch"] = toml_edit::value(false),
        _ => {}
    }
    let agent = project.default_agent.as_deref().unwrap_or_default();
    set(doc.as_table_mut(), "default_agent", agent);
    let primary = (doc.entry("primary").or_insert(toml_edit::table()))
        .as_table_like_mut()
        .expect("a loaded project's primary is a table");
    set(primary, "path", &project.primary.path.to_string_lossy());
    set(primary, "role", &project.primary.role);
    let mut old: Vec<Table> = match doc.remove("roots") {
        Some(Item::ArrayOfTables(roots)) => roots.into_iter().collect(),
        Some(Item::Value(Value::Array(roots))) => (roots.into_iter())
            .filter_map(|root| match root {
                Value::InlineTable(root) => Some(root.into_table()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    // Tables print sorted by position, stably, so one shared position keeps the roots' order.
    let start = old.iter().filter_map(Table::position).min();
    let mut roots = ArrayOfTables::new();
    for root in &project.roots {
        let path = root.path.to_string_lossy();
        let same = |t: &Table| t.get("path").and_then(Item::as_str) == Some(&*path);
        let mut table = old
            .iter()
            .position(same)
            .map_or_else(Table::new, |i| old.remove(i));
        set(&mut table, "path", &path);
        set(&mut table, "role", &root.role);
        table.set_position(start);
        roots.push(table);
    }
    if !roots.is_empty() {
        doc.insert("roots", Item::ArrayOfTables(roots));
    }
}

/// Sets `key` to `value`, keeping the old value's comments, or removes it when `value` is empty.
fn set(table: &mut dyn TableLike, key: &str, value: &str) {
    match table.get_mut(key) {
        _ if value.is_empty() => _ = table.remove(key),
        Some(Item::Value(old)) if old.as_str() == Some(value) => {}
        Some(Item::Value(old)) => {
            let decor = old.decor().clone();
            *old = value.into();
            *old.decor_mut() = decor;
        }
        _ => _ = table.insert(key, toml_edit::value(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
name = "aiditor"
default_agent = "claude"
ignore = ["*", "!notes/"]

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
        assert_eq!(p.ignore, ["*", "!notes/"]);
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
        assert!(p.watch);
        assert_eq!((p.default_agent.as_ref(), p.roots.len()), (None, 0));
        assert!(p.ignore.is_empty());
        assert_eq!(p.primary.role, "");
        assert_eq!(Project::parse(&toml::to_string(&p).unwrap()).unwrap(), p);
    }

    #[test]
    fn watch_round_trips_and_sync_preserves_comments() {
        for watch in [true, false] {
            let text =
                format!("name = \"x\"\nwatch = {watch} # quiet\n[primary]\npath = \"/r/x\"\n");
            let mut p = Project::parse(&text).unwrap();
            assert_eq!(p.watch, watch);
            assert_eq!(Project::parse(&toml::to_string(&p).unwrap()).unwrap(), p);
            let mut doc: DocumentMut = text.parse().unwrap();
            sync(&mut doc, &p);
            assert_eq!(doc.to_string(), text);
            p.watch = !watch;
            sync(&mut doc, &p);
            assert_eq!(
                doc.to_string(),
                text.replace(&watch.to_string(), &(!watch).to_string())
            );
            assert_eq!(Project::parse(&doc.to_string()).unwrap(), p);
        }
        let text = "name = \"x\"\n[primary]\npath = \"/r/x\"\n";
        let mut p = Project::parse(text).unwrap();
        let mut doc: DocumentMut = text.parse().unwrap();
        p.watch = false;
        sync(&mut doc, &p);
        assert_eq!(Project::parse(&doc.to_string()).unwrap(), p);
    }

    #[test]
    fn home_ad_hoc_is_unwatched_and_other_roots_are_watched() {
        let home = std::env::home_dir().unwrap();
        assert!(!Loaded::ad_hoc("home", &home).project.watch);
        assert!(Loaded::ad_hoc("other", &home.join("other")).project.watch);
    }

    #[test]
    fn bad_files_are_rejected() {
        let primary = "[primary]\npath = \"/r/x\"";
        for (text, needle) in [
            ("name = ", "parse error"),
            (&format!("name = 1\n{primary}"), "name"),
            (
                &format!("name = \"x\"\nwatch = \"false\"\n{primary}"),
                "watch",
            ),
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
    fn sync_changes_only_what_differs() {
        let text = r#"# My project
name = "aiditor"
default_agent = "claude" # for now
ignore = ["target/"] # project commands leave this alone

[primary]
path = "/r/aiditor"
role = "The editor itself"

# GPUI
[[roots]]
path = "/r/zed"
role = "GPUI source"

[[roots]]
path = "/r/notes"
"#;
        let mut p = Project::parse(text).unwrap();
        let mut doc: DocumentMut = text.parse().unwrap();
        sync(&mut doc, &p);
        assert_eq!(
            doc.to_string(),
            text,
            "an unchanged project leaves the text alone"
        );
        p.default_agent = None;
        p.primary.role = "Editor".into();
        p.roots.swap(0, 1);
        p.roots[0].role = "Notes".into();
        p.roots[1].role.clear();
        p.roots.push(Root {
            path: "/r/new".into(),
            role: "New".into(),
        });
        sync(&mut doc, &p);
        let got = doc.to_string();
        assert_eq!(Project::parse(&got).unwrap(), p, "{got}");
        for kept in ["# My project", "# GPUI\n[[roots]]\npath = \"/r/zed\""] {
            assert!(got.contains(kept), "{kept:?} in {got}");
        }
        p.roots.clear();
        sync(&mut doc, &p);
        let got = doc.to_string();
        assert_eq!(Project::parse(&got).unwrap(), p, "{got}");
        // An inline primary and roots array are edited too.
        let inline = "name = \"x\"\nprimary = { path = \"/r/x\" }\nroots = [{ path = \"/r/y\" }]\n";
        let mut p = Project::parse(inline).unwrap();
        let mut doc: DocumentMut = inline.parse().unwrap();
        p.primary.role = "X".into();
        p.roots[0].role = "Y".into();
        sync(&mut doc, &p);
        assert_eq!(Project::parse(&doc.to_string()).unwrap(), p);
    }

    #[test]
    fn sibling_with_shared_prefix_loads() {
        let text = "name = \"x\"\n[primary]\npath = \"/r/app\"\n[[roots]]\npath = \"/r/app2\"";
        assert!(Project::parse(text).is_ok());
    }
}
