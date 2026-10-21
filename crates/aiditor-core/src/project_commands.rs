//! The project commands: making, editing, visiting and saving projects.

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use crate::buffer::{file_name, unique};
use crate::command::Args;
use crate::project::{Loaded, Project, Root, sync};
use crate::{Core, Ctx, Effect, FileOp};

/// A project's file index: the files under each root, relative to it, from the walk numbered
/// `walk` once it answers.
#[derive(Default)]
pub(crate) struct FileIndex {
    walk: u64,
    roots: Vec<(PathBuf, Vec<PathBuf>)>,
}

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
    pub(crate) fn project(&self, name: &str) -> Result<usize, String> {
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

    /// The name of the project for `path`: the first saved project with a root containing it,
    /// else the ad-hoc project whose primary is `root`, else a new ad-hoc project at `root`.
    pub(crate) fn project_for(&mut self, path: &Path, root: &Path) -> String {
        let saved = (self.projects.iter())
            .find(|p| p.file.is_some() && p.project.all_roots().any(|r| path.starts_with(&r.path)));
        // Ad-hoc projects match only exactly, or the `$HOME` one would take every folder.
        let ad_hoc =
            || (self.projects.iter()).find(|p| p.file.is_none() && p.project.primary.path == root);
        if let Some(found) = saved.or_else(ad_hoc) {
            return found.project.name.clone();
        }
        let taken = |name: &str| self.projects.iter().any(|p| p.project.name == name);
        let name = unique(&file_name(root), taken);
        self.projects.push(Loaded::ad_hoc(&name, root));
        name
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

    /// Takes the file service's answer to `FileOp::Index`, unless a newer walk was asked for.
    pub(crate) fn indexed(
        &mut self,
        project: String,
        walk: u64,
        roots: Vec<(PathBuf, Result<Vec<PathBuf>, String>)>,
    ) {
        if self.project(&project).is_err() {
            return;
        }
        let Some(index) = self.index.get_mut(&project).filter(|i| i.walk == walk) else {
            return;
        };
        let mut errors = String::new();
        index.roots = (roots.into_iter())
            .map(|(root, files)| {
                let files = files.unwrap_or_else(|err| {
                    errors.push_str(&format!("; {err}"));
                    Vec::new()
                });
                (root, files)
            })
            .collect();
        let n: usize = index.roots.iter().map(|(_, files)| files.len()).sum();
        let s = if n == 1 { "" } else { "s" };
        self.echo(format!("Indexed {n} file{s} in {project}{errors}"));
        // An open finder is always the current workspace's, as switching closes the minibuffer.
        if self.current_workspace().project == project {
            let files = self.project_files(&project);
            self.add_candidates("project-find-file", files);
        }
    }

    /// Every indexed file of `project` as `<root name>/<relative path>`, the primary's first.
    fn project_files(&self, project: &str) -> Vec<String> {
        let roots = self.index.get(project).map_or(&[][..], |i| &i.roots);
        let names = root_names(roots);
        (names.iter().zip(roots))
            .flat_map(|(name, (_, files))| {
                (files.iter()).map(move |f| format!("{name}/{}", f.display()))
            })
            .collect()
    }

    /// The file `text` names in `project`: under the root its first component names, else under
    /// the primary.
    pub(crate) fn project_file(&self, project: &str, text: &str) -> Result<PathBuf, String> {
        let roots = self.index.get(project).map_or(&[][..], |i| &i.roots);
        if let Some((head, rest)) = text.split_once('/')
            && let Some(i) = root_names(roots).iter().position(|n| n == head)
        {
            return Ok(roots[i].0.join(rest));
        }
        let primary = &self.projects[self.project(project)?].project.primary.path;
        Ok(primary.join(text))
    }
}

/// Each indexed root's name: its last component, with `<2>` on for one an earlier root has.
fn root_names(roots: &[(PathBuf, Vec<PathBuf>)]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (root, _) in roots {
        let name = unique(&file_name(root), |n| names.iter().any(|taken| taken == n));
        names.push(name);
    }
    names
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

    /// Walks every root of the project `name` again, off the core thread.
    pub(crate) fn index_project(&mut self, name: &str) -> Result<(), String> {
        let project = &self.core.projects[self.core.project(name)?].project;
        let roots = project.all_roots().map(|r| r.path.clone()).collect();
        self.core.index_walks += 1;
        let walk = self.core.index_walks;
        self.core.index.entry(name.into()).or_default().walk = walk;
        self.core.echo(format!("Indexing {name}..."));
        self.effects.push(Effect::File(FileOp::Index {
            project: name.into(),
            walk,
            roots,
        }));
        Ok(())
    }

    /// Asks for a file in any root of the current workspace's project and visits it, indexing
    /// the project first when it has no index yet.
    pub(crate) fn project_find_file(&mut self, args: Args) -> Result<(), String> {
        let project = self.core.current_workspace().project.clone();
        if let Some(text) = args.strings.first() {
            let path = self.core.project_file(&project, text)?;
            return self.find_file(&path.to_string_lossy());
        }
        if !self.core.index.contains_key(&project) {
            self.index_project(&project)?;
        }
        let files = self.core.project_files(&project);
        let choices: Vec<&str> = files.iter().map(String::as_str).collect();
        let command = self.core.commands["project-find-file"];
        let prompt = format!("Find file in {project}: ");
        self.read_string(command, args, prompt, Some(&choices))
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
        // Without the launch `$HOME` project, so the lists hold only the test's.
        h.core.projects.clear();
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
        let tmp = h.dir.path().display().to_string();
        // Only directories are listed, the typed one first, and RET takes the selected one.
        h.keys(&format!("C-x p c app RET C-a C-k {tmp}/"));
        assert_eq!(
            crate::harness::listed(&mut h),
            (vec!["./".into(), "a/".into(), "b/".into()], Some(0))
        );
        h.keys("C-n RET");
        let file = h.path("projects/app.toml");
        assert_eq!(
            h.core.message(),
            Some(&*format!("Wrote {}", file.display()))
        );
        let text = format!("name = \"app\"\n\n[primary]\npath = \"{a}\"\n");
        assert_eq!(h.disk("projects/app.toml"), text, "no empty roots or role");
        let p = Project::parse(&text).unwrap();
        assert_eq!(h.core.projects(), [&p]);
        // Typing filters the list; `./` takes the directory typed.
        h.keys(&format!("C-x p a app RET C-a C-k {tmp}/b RET Bee RET"));
        h.keys(&format!("C-x p c two RET C-a C-k {tmp}/b/ RET"));
        let (app, two) = (project(&h, "app"), project(&h, "two"));
        assert_eq!(
            app.roots,
            [Root {
                path: h.path("b"),
                role: "Bee".into()
            }]
        );
        assert_eq!(two.primary.path, h.path("b"));
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
            "project-switch",
            "project-index",
            "project-find-file",
        ] {
            assert!(hint.contains(command), "{command} in {hint}");
        }
    }

    /// The `tmp` project from `ad_hoc` with `b` and `roots` as additional roots.
    fn indexable(roots: &[&str]) -> Harness {
        let mut h = with(Vec::new());
        let mut loaded = ad_hoc(&h);
        for root in ["b"].iter().chain(roots) {
            loaded.project.roots.push(Root {
                path: h.path(root),
                role: String::new(),
            });
        }
        h.core.projects.push(loaded);
        h
    }

    /// Calls `project-index` for `tmp`, returning the walk it asks for without carrying it out.
    fn start_index(h: &mut Harness) -> crate::Effects {
        let mut ctx = h.ctx();
        let strings = vec!["tmp".into()];
        ctx.call(
            "project-index",
            Args {
                strings,
                ..Args::default()
            },
        );
        ctx.effects
    }

    fn indexed(h: &Harness) -> Vec<(std::path::PathBuf, Vec<std::path::PathBuf>)> {
        h.core.index["tmp"].roots.clone()
    }

    #[test]
    fn index_holds_each_roots_files_and_counts_them() {
        let mut h = indexable(&[]);
        std::fs::create_dir_all(h.path("a/.git")).unwrap();
        std::fs::write(h.path("a/.git/config"), "").unwrap();
        std::fs::write(h.path("a/.env"), "").unwrap();
        h.keys("C-x p i tmp RET");
        assert_eq!(h.core.message(), Some("Indexed 3 files in tmp"));
        assert_eq!(
            indexed(&h),
            [
                (h.path("a"), vec![".env".into(), "x".into()]),
                (h.path("b"), vec!["x".into()])
            ]
        );
    }

    #[test]
    fn index_names_a_root_it_could_not_walk() {
        let mut h = indexable(&["gone"]);
        call(&mut h, "project-index", &["tmp"]);
        let gone = h.path("gone").display().to_string();
        let msg = h.core.message().unwrap();
        assert!(
            msg.starts_with(&format!("Indexed 2 files in tmp; {gone}: ")),
            "{msg}"
        );
        assert_eq!(indexed(&h)[2], (h.path("gone"), Vec::new()));
    }

    #[test]
    fn typing_goes_on_while_a_walk_runs() {
        let mut h = indexable(&[]);
        let pending = start_index(&mut h);
        h.keys("hi");
        assert_eq!(h.core.text(), "hi");
        h.run(pending);
        assert_eq!(h.core.message(), Some("Indexed 2 files in tmp"));
    }

    #[test]
    fn index_drops_an_older_walks_answer() {
        let mut h = indexable(&[]);
        let first = start_index(&mut h);
        assert_eq!(h.core.message(), Some("Indexing tmp..."));
        call(&mut h, "project-index", &["tmp"]);
        std::fs::write(h.path("a/y"), "").unwrap();
        h.run(first);
        assert_eq!(h.core.message(), Some("Indexed 2 files in tmp"));
        assert_eq!(indexed(&h)[0], (h.path("a"), vec!["x".into()]));
        // Nor is an answer for a project no longer loaded taken.
        let pending = start_index(&mut h);
        h.core.projects.clear();
        h.run(pending);
        assert_eq!(h.core.message(), Some("Indexing tmp..."));
    }

    /// `indexable` with the current workspace bound to `tmp`.
    fn findable(roots: &[&str]) -> Harness {
        let mut h = indexable(roots);
        h.core.workspaces[0].project = "tmp".into();
        h
    }

    fn candidates(h: &mut Harness) -> Vec<String> {
        h.core.minibuffer().unwrap().candidates
    }

    fn visited(h: &Harness) -> Option<std::path::PathBuf> {
        h.core.current().file.clone()
    }

    #[test]
    fn finds_files_in_the_primary_and_an_additional_root() {
        let mut h = findable(&[]);
        h.keys("C-x p f");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Find file in tmp: ");
        assert_eq!(candidates(&mut h), ["a/x", "b/x"]);
        h.keys("b RET");
        assert_eq!(visited(&h), Some(h.path("b/x")));
        h.keys("C-x p f a RET");
        assert_eq!(visited(&h), Some(h.path("a/x")));
    }

    #[test]
    fn a_repeated_root_name_gets_a_number() {
        let mut h = findable(&["q/a"]);
        std::fs::create_dir_all(h.path("q/a")).unwrap();
        std::fs::write(h.path("q/a/y"), "").unwrap();
        h.keys("C-x p f");
        assert_eq!(candidates(&mut h), ["a/x", "b/x", "a<2>/y"]);
        h.keys("C-g");
        call(&mut h, "project-find-file", &["a<2>/y"]);
        assert_eq!(visited(&h), Some(h.path("q/a/y")));
    }

    #[test]
    fn a_typed_name_is_visited_under_its_root_or_the_primary() {
        let mut h = findable(&[]);
        h.keys("C-x p f b/new.rs M-RET");
        assert_eq!(visited(&h), Some(h.path("b/new.rs")));
        assert_eq!(h.text(), "");
        h.keys("C-x p f notes.md RET");
        assert_eq!(visited(&h), Some(h.path("a/notes.md")));
    }

    #[test]
    fn the_first_use_indexes_and_fills_the_open_list() {
        let mut h = findable(&[]);
        let mut ctx = h.ctx();
        ctx.call("project-find-file", Args::default());
        let pending = ctx.effects;
        assert_eq!(h.core.message(), Some("Indexing tmp..."));
        assert_eq!(candidates(&mut h), Vec::<String>::new());
        h.run(pending);
        assert_eq!(candidates(&mut h), ["a/x", "b/x"]);
        h.keys("C-g C-x p f");
        assert_eq!(h.core.index_walks, 1, "no second walk");
        assert_eq!(candidates(&mut h), ["a/x", "b/x"]);
    }

    /// The name of the buffer the first window shows while the minibuffer is open.
    fn shown(h: &Harness) -> String {
        let id = h.core.others[&h.core.tree.leaves()[0]].buffer();
        h.core.buffers.get(id).unwrap().name.clone()
    }

    #[test]
    fn the_finder_previews_files_in_a_hidden_buffer() {
        let mut h = findable(&[]);
        std::fs::write(h.path("b/x"), "bee").unwrap();
        h.keys("C-x p f C-n");
        assert_eq!(shown(&h), " x");
        let peek = h.core.buffers.visiting(&h.path("b/x")).unwrap();
        let buffer = h.core.buffers.get(peek).unwrap();
        assert_eq!(
            (buffer.text().to_string().as_str(), buffer.read_only),
            ("bee", true)
        );
        assert!(
            h.core.buffers.visiting(&h.path("a/x")).is_none(),
            "one peek"
        );
        h.keys("C-g");
        assert_eq!(name(&h), "*scratch*<main>");
        assert!(h.core.buffers.visiting(&h.path("b/x")).is_none(), "killed");
        h.keys("C-x p f C-n RET");
        assert_eq!(visited(&h), Some(h.path("b/x")));
        assert_eq!((name(&h), h.core.current().read_only), ("x", false));
    }

    #[test]
    fn the_finder_previews_a_visited_file_s_own_buffer() {
        let mut h = findable(&[]);
        h.open("b/x").open("a/x");
        h.keys("C-x p f C-n");
        assert_eq!(shown(&h), "x");
    }

    #[test]
    fn a_read_no_longer_awaited_is_dropped() {
        let mut h = findable(&[]);
        h.keys("C-x p f");
        let result = Ok(Some((b"late".to_vec(), 0)));
        h.send(Msg::Previewed {
            path: h.path("b/x"),
            result,
        });
        assert_eq!(shown(&h), " x");
        assert!(h.core.buffers.visiting(&h.path("b/x")).is_none());
        assert!(h.core.buffers.visiting(&h.path("a/x")).is_some());
    }
}
