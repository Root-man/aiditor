//! Project files under the config dir's `projects/`.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use aiditor_core::buffer::hash;
use aiditor_core::project::{Loaded, Project};

use crate::files;

/// Every `*.toml` in `dir`, by file name, parsed and with its roots checked to be directories.
/// Each error starts with the file's name; a missing `dir` means no projects.
pub fn read(dir: &Path) -> Vec<Result<Loaded, String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Vec::new(),
        Err(err) => return vec![Err(format!("{}: {err}", dir.display()))],
    };
    let mut files: Vec<_> = (entries.flatten().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|file| {
            let err = |err: String| format!("{}: {err}", name(file));
            let text = fs::read_to_string(file).map_err(|e| err(e.to_string()))?;
            let project = check(&text).map_err(err)?;
            Ok(Loaded {
                project,
                file: Some((file.clone(), text)),
            })
        })
        .collect()
}

/// Writes the project file `text` to `file` if it is a valid project with existing roots and
/// the file is still `old` (`None`: no file, making its directory), and returns it as loaded.
/// Each error names the file.
pub fn write(file: &Path, old: Option<&str>, text: String) -> Result<Loaded, String> {
    let project = check(&text).map_err(|err| format!("{}: {err}", name(file)))?;
    if old.is_none() {
        if file.exists() {
            return Err(format!("{} already exists", file.display()));
        }
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
        }
    }
    let changed = |err: String| err.replace("not saved", "reload with C-x C-r");
    files::write(file, text.as_bytes(), old.map(|old| hash(old.as_bytes()))).map_err(changed)?;
    Ok(Loaded {
        project,
        file: Some((file.to_owned(), text)),
    })
}

fn name(file: &Path) -> std::borrow::Cow<'_, str> {
    file.file_name().unwrap_or_default().to_string_lossy()
}

/// Parses a project file's text and checks its roots are directories and its ignore patterns
/// compile.
fn check(text: &str) -> Result<Project, String> {
    let project = Project::parse(text)?;
    files::Patterns::new(&project.ignore)?;
    if let Some(root) = project.all_roots().find(|root| !root.path.is_dir()) {
        return Err(format!("{} is not a directory", root.path.display()));
    }
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_valid_files_and_names_the_bad_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projects");
        assert!(read(&dir).is_empty(), "a missing dir is no projects");
        fs::create_dir(&dir).unwrap();
        let root = tmp.path().display();
        let file = |name: &str, primary: &str| {
            let text = format!("name = \"{name}\"\n[primary]\npath = \"{primary}\"");
            fs::write(dir.join(format!("{name}.toml")), text).unwrap();
        };
        file("a", &root.to_string());
        file("b", &format!("{root}/gone"));
        file("c", "rel");
        fs::write(dir.join("notes.txt"), "not a project").unwrap();
        let got = read(&dir);
        assert_eq!(got.len(), 3, "{got:?}");
        let a = got[0].as_ref().unwrap();
        assert_eq!(a.project.name, "a");
        assert_eq!(
            a.file,
            Some((
                dir.join("a.toml"),
                fs::read_to_string(dir.join("a.toml")).unwrap()
            ))
        );
        assert_eq!(
            got[1],
            Err(format!("b.toml: {root}/gone is not a directory"))
        );
        assert_eq!(got[2], Err("c.toml: rel is not absolute".into()));
    }

    /// Feeds `script` to `core`, carrying out file effects at once as the headless binary does.
    fn keys(core: &mut aiditor_core::Core, script: &str) {
        fn send(core: &mut aiditor_core::Core, msg: aiditor_core::Msg) {
            for effect in core.handle(std::time::Instant::now(), msg) {
                if let aiditor_core::Effect::File(op) = effect {
                    send(core, files::run(op));
                }
            }
        }
        for key in aiditor_core::key::parse_keys(script).unwrap() {
            send(core, aiditor_core::Msg::Key(key));
            // A tick waits for the matcher again, as a user waits for the list to fill.
            send(core, aiditor_core::Msg::Tick);
        }
    }

    #[test]
    fn a_project_made_and_edited_from_the_editor_loads_back() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("projects");
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let mut core = aiditor_core::Core::default();
        let projects = read(&dir);
        core.handle(
            std::time::Instant::now(),
            aiditor_core::Msg::ProjectsLoaded {
                dir: Some(dir.clone()),
                projects,
            },
        );
        // As a user does: from a/, DEL up to the parent, then pick a listed directory.
        let start = format!("C-a C-k {}/ DEL", a.display());
        keys(&mut core, &format!("C-x p c app RET {start} C-n RET"));
        keys(
            &mut core,
            &format!("C-x p a app RET {start} b RET Docs RET"),
        );
        keys(&mut core, "C-x p d app RET claude RET");
        assert_eq!(
            core.message(),
            Some(&*format!("Wrote {}", dir.join("app.toml").display()))
        );
        let loaded: Vec<_> = read(&dir).into_iter().map(|p| p.unwrap().project).collect();
        // After the launch `$HOME` project.
        assert_eq!(core.projects()[1..], loaded.iter().collect::<Vec<_>>());
        let p = &loaded[0];
        assert_eq!((&p.primary.path, &p.roots[0].path), (&a, &b));
        assert_eq!(
            (p.roots[0].role.as_str(), p.default_agent.as_deref()),
            ("Docs", Some("claude"))
        );
    }

    #[test]
    fn write_checks_the_project_and_the_disk_first() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("projects/p.toml");
        let root = tmp.path().display();
        let text = format!("name = \"p\"\n[primary]\npath = \"{root}\"\n");
        // A new file makes the directory, and reads back as written.
        let wrote = write(&file, None, text.clone()).unwrap();
        assert_eq!(read(file.parent().unwrap()), [Ok(wrote.clone())]);
        let exists = write(&file, None, text.clone()).unwrap_err();
        assert!(exists.ends_with("p.toml already exists"), "{exists}");
        let bad = format!("ignore = [\"[z-a]\"]\n{text}");
        let err = write(&file, Some(&text), bad).unwrap_err();
        assert!(
            err.starts_with("p.toml: ignore pattern \"[z-a]\": "),
            "{err}"
        );
        let missing = format!("{text}[[roots]]\npath = \"{root}2\"\n");
        assert_eq!(
            write(&file, Some(&text), missing),
            Err(format!("p.toml: {root}2 is not a directory"))
        );
        let edited = format!("# mine\n{text}");
        let changed = write(&file, Some("stale"), edited.clone()).unwrap_err();
        assert!(
            changed.ends_with("changed on disk; reload with C-x C-r"),
            "{changed}"
        );
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            text,
            "refusals leave the file"
        );
        let wrote = write(&file, Some(&text), edited.clone()).unwrap();
        assert_eq!(wrote.file, Some((file.clone(), edited.clone())));
        assert_eq!(fs::read_to_string(&file).unwrap(), edited);
    }
}
