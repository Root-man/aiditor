//! Project files under the config dir's `projects/`.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use aiditor_core::project::Project;

/// Every `*.toml` in `dir`, by file name, parsed and with its roots checked to be directories.
/// Each error starts with the file's name; a missing `dir` means no projects.
pub fn read(dir: &Path) -> Vec<Result<Project, String>> {
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
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            check(file).map_err(|err| format!("{name}: {err}"))
        })
        .collect()
}

fn check(file: &Path) -> Result<Project, String> {
    let project = Project::parse(&fs::read_to_string(file).map_err(|e| e.to_string())?)?;
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
        assert_eq!(got[0].as_ref().unwrap().name, "a");
        assert_eq!(
            got[1],
            Err(format!("b.toml: {root}/gone is not a directory"))
        );
        assert_eq!(got[2], Err("c.toml: rel is not absolute".into()));
    }
}
