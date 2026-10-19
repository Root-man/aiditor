//! The project commands: making, editing, visiting and saving projects.

use std::path::PathBuf;

use toml_edit::DocumentMut;

use crate::command::Args;
use crate::project::{Loaded, Project, Root, sync};
use crate::{Core, Ctx, Effect, FileOp};

impl Core {
    /// The loaded projects' names, only the ad-hoc ones' when `ad_hoc`.
    pub(crate) fn project_names(&self, ad_hoc: bool) -> Vec<String> {
        (self.projects.iter())
            .filter(|p| !ad_hoc || p.file.is_none())
            .map(|p| p.project.name.clone())
            .collect()
    }

    /// The project named `name`, as an index into `projects`.
    // shortcut: duplicate names across files are not caught (#56), so the first wins; check names at load if that bites.
    fn project(&self, name: &str) -> Result<usize, String> {
        (self.projects.iter())
            .position(|p| p.project.name == name)
            .ok_or_else(|| format!("No project named {name}"))
    }

    /// The root paths of the project named by the first string, without the primary unless
    /// `primary`.
    pub(crate) fn root_paths(&self, args: &Args, primary: bool) -> Vec<String> {
        let Ok(i) = self.project(&args.strings[0]) else {
            return Vec::new();
        };
        (self.projects[i].project.all_roots())
            .skip(usize::from(!primary))
            .map(|root| root.path.display().to_string())
            .collect()
    }

    /// A directory typed at a prompt, taken from the current buffer's directory, without a
    /// trailing `/`.
    fn root_path(&self, typed: &str) -> PathBuf {
        self.directory().join(typed).components().collect()
    }

    /// Takes the file service's answer to `FileOp::WriteProject`: the project replaces the one
    /// with its file, or else the ad-hoc one with its name.
    pub(crate) fn project_written(&mut self, result: Result<Loaded, String>) {
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(err) => return self.echo(format!("project {err}")),
        };
        let path = loaded
            .file
            .as_ref()
            .expect("a written project has a file")
            .0
            .clone();
        let file =
            (self.projects.iter()).position(|p| p.file.as_ref().is_some_and(|f| f.0 == path));
        let ad_hoc = || {
            (self.projects.iter())
                .position(|p| p.file.is_none() && p.project.name == loaded.project.name)
        };
        match file.or_else(ad_hoc) {
            Some(i) => self.projects[i] = loaded,
            None => self.projects.push(loaded),
        }
        self.echo(format!("Wrote {}", path.display()));
    }
}

impl Ctx<'_> {
    pub(crate) fn create_project(&mut self, name: &str, primary: &str) -> Result<(), String> {
        if self.core.project(name).is_ok() {
            return Err(format!("Project {name} already exists"));
        }
        let project = Project {
            name: name.into(),
            default_agent: None,
            primary: Root {
                path: self.core.root_path(primary),
                role: String::new(),
            },
            roots: Vec::new(),
        };
        self.write_new_project(&project)
    }

    pub(crate) fn save_project(&mut self, name: &str) -> Result<(), String> {
        let loaded = &self.core.projects[self.core.project(name)?];
        if let Some((path, _)) = &loaded.file {
            return Err(format!("{name} is already saved in {}", path.display()));
        }
        let project = loaded.project.clone();
        self.write_new_project(&project)
    }

    /// Writes `project` to a new `<name>.toml` in the projects directory.
    fn write_new_project(&mut self, project: &Project) -> Result<(), String> {
        project.check()?;
        let name = &project.name;
        if name.contains('/') || name.starts_with('.') {
            return Err(format!("{name} can't be a file name"));
        }
        let taken = |p: &Loaded| p.file.is_some() && p.project.name == *name;
        if self.core.projects.iter().any(taken) {
            return Err(format!("Project {name} already exists"));
        }
        let dir = self
            .core
            .projects_dir
            .as_ref()
            .ok_or("No config directory")?;
        let path = dir.join(format!("{name}.toml"));
        let text = toml::to_string(project).map_err(|err| err.to_string())?;
        self.effects.push(Effect::File(FileOp::WriteProject {
            path,
            old: None,
            text,
        }));
        Ok(())
    }

    /// Changes a copy of the project named `name` with `edit` and keeps it once checked: in
    /// memory while it is ad-hoc, else by rewriting its file in place.
    fn edit_project(
        &mut self,
        name: &str,
        edit: impl FnOnce(&mut Project) -> Result<(), String>,
    ) -> Result<(), String> {
        let i = self.core.project(name)?;
        let loaded = &mut self.core.projects[i];
        let mut project = loaded.project.clone();
        edit(&mut project)?;
        project.check()?;
        let Some((path, old)) = &loaded.file else {
            loaded.project = project;
            return Ok(());
        };
        let mut doc: DocumentMut = old.parse().map_err(|err| format!("{err}"))?;
        sync(&mut doc, &project);
        self.effects.push(Effect::File(FileOp::WriteProject {
            path: path.clone(),
            old: Some(old.clone()),
            text: doc.to_string(),
        }));
        Ok(())
    }

    pub(crate) fn add_root(&mut self, args: &Args) -> Result<(), String> {
        let path = self.core.root_path(&args.strings[1]);
        let role = args.strings[2].clone();
        self.edit_project(&args.strings[0], |p| {
            p.roots.push(Root { path, role });
            Ok(())
        })
    }

    pub(crate) fn remove_root(&mut self, args: &Args) -> Result<(), String> {
        let (name, path) = (&args.strings[0], self.core.root_path(&args.strings[1]));
        self.edit_project(name, |p| {
            let i = (p.roots.iter().position(|r| r.path == path))
                .ok_or_else(|| format!("{} is not an additional root of {name}", path.display()))?;
            p.roots.remove(i);
            Ok(())
        })
    }

    pub(crate) fn set_role(&mut self, args: &Args) -> Result<(), String> {
        let (name, path) = (&args.strings[0], self.core.root_path(&args.strings[1]));
        self.edit_project(name, |p| {
            let root = (std::iter::once(&mut p.primary).chain(&mut p.roots))
                .find(|r| r.path == path)
                .ok_or_else(|| format!("{} is not a root of {name}", path.display()))?;
            root.role = args.strings[2].clone();
            Ok(())
        })
    }

    pub(crate) fn set_default_agent(&mut self, args: &Args) -> Result<(), String> {
        let agent = &args.strings[1];
        self.edit_project(&args.strings[0], |p| {
            p.default_agent = (!agent.is_empty()).then(|| agent.clone());
            Ok(())
        })
    }

    pub(crate) fn open_project_file(&mut self, name: &str) -> Result<(), String> {
        match self.core.projects[self.core.project(name)?].file.clone() {
            Some((path, _)) => self.find_file(&path.to_string_lossy()),
            None => Err(format!("{name} has no file; save it with project-save")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::command::Args;
    use crate::harness::{Harness, name};
    use crate::project::{Loaded, Project, Root};

    /// A harness with directories `a` and `b`, projects going to `projects/` and `loaded` loaded.
    fn with(loaded: Vec<Loaded>) -> Harness {
        let mut h = Harness::new(&[("a/x", ""), ("b/x", "")]);
        let dir = Some(h.path("projects"));
        let projects = loaded.into_iter().map(Ok).collect();
        h.send(Msg::ProjectsLoaded { dir, projects });
        h
    }

    fn call(h: &mut Harness, command: &str, strings: &[&str]) -> Option<String> {
        let strings = strings.iter().map(|s| s.to_string()).collect();
        h.call(
            command,
            Args {
                strings,
                ..Args::default()
            },
        );
        h.core.message().map(str::to_string)
    }

    fn project(h: &Harness, name: &str) -> Project {
        (h.core.projects().into_iter())
            .find(|p| p.name == name)
            .unwrap()
            .clone()
    }

    fn ad_hoc(h: &Harness) -> Loaded {
        let project = Project {
            name: "tmp".into(),
            default_agent: None,
            primary: Root {
                path: h.path("a"),
                role: String::new(),
            },
            roots: Vec::new(),
        };
        Loaded {
            project,
            file: None,
        }
    }

    #[test]
    fn create_writes_a_file_that_loads_back() {
        let mut h = with(Vec::new());
        let a = h.path("a").display().to_string();
        // RET would pick the first entry listed under it; M-RET takes the directory as typed.
        h.keys(&format!("C-x p c app RET C-a C-k {a}/ M-RET"));
        let file = h.path("projects/app.toml");
        assert_eq!(
            h.core.message(),
            Some(&*format!("Wrote {}", file.display()))
        );
        let p = Project::parse(&h.disk("projects/app.toml")).unwrap();
        assert_eq!((p.name.as_str(), &p.primary.path), ("app", &h.path("a")));
        assert_eq!(h.core.projects(), [&p]);
        for (name, err) in [
            ("app", "Project app already exists"),
            (".app", ".app can't be a file name"),
            ("a/b", "a/b can't be a file name"),
            (" ", "name is empty"),
        ] {
            assert_eq!(
                call(&mut h, "project-create", &[name, &a]).as_deref(),
                Some(err)
            );
        }
        // Without a config dir there is nowhere to write.
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        });
        let err = call(&mut h, "project-create", &["new", &a]);
        assert_eq!(err.as_deref(), Some("No config directory"));
    }

    #[test]
    fn edits_rewrite_the_file_and_keep_its_comments() {
        let mut h = with(Vec::new());
        let (a, b) = (
            h.path("a").display().to_string(),
            h.path("b").display().to_string(),
        );
        let text = format!("# Mine\nname = \"app\"\n\n[primary]\npath = \"{a}\"\n");
        std::fs::create_dir(h.path("projects")).unwrap();
        std::fs::write(h.path("projects/app.toml"), &text).unwrap();
        let loaded = Loaded {
            project: Project::parse(&text).unwrap(),
            file: Some((h.path("projects/app.toml"), text)),
        };
        h.send(Msg::ProjectsLoaded {
            dir: Some(h.path("projects")),
            projects: vec![Ok(loaded)],
        });
        call(&mut h, "project-add-root", &["app", &format!("{b}/"), "B"]);
        call(&mut h, "project-set-role", &["app", &a, "Main"]);
        call(&mut h, "project-set-default-agent", &["app", "claude"]);
        let disk = h.disk("projects/app.toml");
        assert!(disk.starts_with("# Mine\n"), "{disk}");
        let p = Project::parse(&disk).unwrap();
        assert_eq!(p, project(&h, "app"));
        assert_eq!(
            (p.primary.role.as_str(), p.default_agent.as_deref()),
            ("Main", Some("claude"))
        );
        assert_eq!(
            p.roots,
            [Root {
                path: h.path("b"),
                role: "B".into()
            }]
        );
        let err = call(&mut h, "project-add-root", &["app", &format!("{a}/x"), ""]);
        assert_eq!(err, Some(format!("{a}/x is inside {a}")));
        let err = call(&mut h, "project-remove-root", &["app", &a]);
        assert_eq!(err, Some(format!("{a} is not an additional root of app")));
        assert_eq!(h.disk("projects/app.toml"), disk, "refusals write nothing");
        // A file changed on disk since it was read is left alone, and so is the project.
        let before = project(&h, "app");
        std::fs::write(h.path("projects/app.toml"), format!("{disk}# theirs\n")).unwrap();
        let err = call(&mut h, "project-set-role", &["app", &a, "Ours"]).unwrap();
        assert!(
            err.ends_with("changed on disk; reload with C-x C-r"),
            "{err}"
        );
        assert!(err.starts_with("project "), "{err}");
        assert_eq!(h.disk("projects/app.toml"), format!("{disk}# theirs\n"));
        assert_eq!(project(&h, "app"), before);
        std::fs::write(h.path("projects/app.toml"), &disk).unwrap();
        call(&mut h, "project-remove-root", &["app", &b]);
        call(&mut h, "project-set-default-agent", &["app", ""]);
        let p = project(&h, "app");
        assert_eq!((p.roots.len(), p.default_agent.as_deref()), (0, None));
        assert_eq!(Project::parse(&h.disk("projects/app.toml")).unwrap(), p);
        call(&mut h, "project-open-file", &["app"]);
        assert_eq!(name(&h), "app.toml");
        let err = call(&mut h, "project-open-file", &["zz"]);
        assert_eq!(err.as_deref(), Some("No project named zz"));
    }

    #[test]
    fn ad_hoc_projects_are_edited_in_memory_then_saved() {
        let mut h = with(Vec::new());
        let loaded = ad_hoc(&h);
        h.send(Msg::ProjectsLoaded {
            dir: Some(h.path("projects")),
            projects: vec![Ok(loaded)],
        });
        let b = h.path("b").display().to_string();
        call(&mut h, "project-add-root", &["tmp", &b, "B"]);
        call(&mut h, "project-set-role", &["tmp", &b, "Bee"]);
        let err = call(&mut h, "project-open-file", &["tmp"]);
        assert_eq!(
            err.as_deref(),
            Some("tmp has no file; save it with project-save")
        );
        assert!(
            !h.path("projects").exists(),
            "nothing written before a save"
        );
        let edited = project(&h, "tmp");
        assert_eq!(edited.roots[0].role, "Bee");
        call(&mut h, "project-save", &["tmp"]);
        assert_eq!(
            Project::parse(&h.disk("projects/tmp.toml")).unwrap(),
            edited
        );
        assert_eq!(
            h.core.projects(),
            [&edited],
            "the saved project replaces the ad-hoc one"
        );
        let err = call(&mut h, "project-save", &["tmp"]).unwrap();
        assert!(err.starts_with("tmp is already saved in "), "{err}");
    }

    #[test]
    fn c_x_p_lists_the_project_commands() {
        let mut h = with(Vec::new());
        h.keys("C-x p").advance(crate::bindings::HINT_DELAY);
        let hint = format!("{:?}", h.core.hint());
        for command in [
            "project-create",
            "project-add-root",
            "project-remove-root",
            "project-set-role",
            "project-set-default-agent",
            "project-open-file",
            "project-save",
        ] {
            assert!(hint.contains(command), "{command} in {hint}");
        }
    }
}
