//! The project commands: making, editing, visiting and saving projects.

use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use crate::buffer::{file_name, unique};
use crate::command::Args;
use crate::project::{Entry, Loaded, Project, ProjectId, Root, WriteIntent, sync};
use crate::{Core, Ctx, Effect, FileOp};

impl Core {
    pub fn project_id(&self, name: &str) -> Result<ProjectId, String> {
        self.project(name).map(|i| self.projects[i].id)
    }

    pub fn project_name(&self, id: ProjectId) -> &str {
        &self.projects[self.project_at(id).expect("workspace project is live")]
            .project
            .name
    }

    pub(crate) fn project_at(&self, id: ProjectId) -> Result<usize, String> {
        self.projects
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| "Project is no longer loaded".into())
    }

    pub(crate) fn add_project(&mut self, loaded: Loaded) -> ProjectId {
        let id = ProjectId(self.next_project);
        self.next_project += 1;
        self.projects.push(Entry::new(id, loaded));
        id
    }

    pub(crate) fn reconcile_projects(&mut self, loaded: Vec<Result<Loaded, String>>) {
        let mut old = std::mem::take(&mut self.projects);
        let mut valid = Vec::new();
        for result in loaded {
            match result {
                Ok(loaded) => {
                    if let (Some(agent), Some((path, _))) =
                        (&loaded.project.default_agent, &loaded.file)
                        && !self.settings.agents.contains_key(agent)
                    {
                        let file = file_name(path);
                        self.report(format!(
                            "project {file}: {agent:?} is not a registered agent"
                        ));
                    }
                    valid.push(loaded);
                }
                Err(err) => self.report(format!("project {err}")),
            }
        }
        valid.sort_by(|a, b| {
            a.file
                .as_ref()
                .map(|f| &f.0)
                .cmp(&b.file.as_ref().map(|f| &f.0))
        });
        let mut saved = Vec::new();
        for entry in old.drain(..) {
            if entry.file.is_none() {
                self.projects.push(entry);
            } else {
                saved.push(entry);
            }
        }
        for loaded in valid {
            let at = saved
                .iter()
                .position(|p| p.file.as_ref().map(|f| &f.0) == loaded.file.as_ref().map(|f| &f.0));
            if let Some(at) = at {
                let mut entry = saved.remove(at);
                entry.replace(loaded);
                self.projects.push(entry);
            } else {
                self.add_project(loaded);
            }
        }
        for mut entry in saved {
            entry.retained = true;
            self.projects.push(entry);
        }
        self.release_projects();
    }

    pub(crate) fn release_projects(&mut self) {
        self.projects
            .retain(|p| !p.retained || self.workspaces.iter().any(|w| w.project == p.id));
        self.release_indexes();
    }

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

    pub(crate) fn project_arg(&self, args: &Args) -> Result<usize, String> {
        match args.project {
            Some(id) => self.project_at(id),
            None => self.project(args.strings.first().ok_or("No project selected")?),
        }
    }

    /// The root paths of the project named by the first string, without the primary unless
    /// `primary`.
    pub(crate) fn root_paths(&self, args: &Args, primary: bool) -> Vec<String> {
        let Ok(i) = self.project_arg(args) else {
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
    pub(crate) fn project_for(&mut self, path: &Path, root: &Path) -> ProjectId {
        let saved = (self.projects.iter())
            .find(|p| p.file.is_some() && p.project.all_roots().any(|r| path.starts_with(&r.path)));
        // Ad-hoc projects match only exactly, or the `$HOME` one would take every folder.
        let ad_hoc =
            || (self.projects.iter()).find(|p| p.file.is_none() && p.project.primary.path == root);
        if let Some(found) = saved.or_else(ad_hoc) {
            return found.id;
        }
        let taken = |name: &str| self.projects.iter().any(|p| p.project.name == name);
        let name = unique(&file_name(root), taken);
        self.add_project(Loaded::ad_hoc(&name, root))
    }

    /// Every indexed file of `project` as `<root name>/<relative path>`, the primary's first.
    pub(crate) fn project_files(&self, project: ProjectId) -> Vec<String> {
        let roots = self.index.get(&project).map_or(&[][..], |i| i.roots());
        let names = root_names(roots.iter().map(|(root, _)| root.as_path()));
        (names.iter().zip(roots))
            .flat_map(|(name, (_, files))| {
                (files.iter()).map(move |f| format!("{name}/{}", f.display()))
            })
            .collect()
    }

    pub(crate) fn finder(&self, project: ProjectId) -> Result<Finder, String> {
        let roots: Vec<_> = self.projects[self.project_at(project)?]
            .project
            .all_roots()
            .map(|r| r.path.clone())
            .collect();
        let waiting = self
            .index
            .get(&project)
            .is_none_or(|index| !index.roots().iter().map(|(p, _)| p).eq(roots.iter()));
        let names = root_names(roots.iter().map(PathBuf::as_path));
        Ok(Finder {
            project,
            roots,
            names,
            waiting,
        })
    }

    pub(crate) fn fill_project_finder(&mut self, project: ProjectId) {
        let Some(finder) = self
            .prompts
            .minibuffer
            .as_mut()
            .and_then(|mb| mb.finder.as_mut())
        else {
            return;
        };
        if finder.project != project || !finder.waiting {
            return;
        }
        let roots = self.index[&project].roots();
        if !roots.iter().map(|(p, _)| p).eq(finder.roots.iter()) {
            return;
        }
        finder.waiting = false;
        let files = self.project_files(project);
        self.add_candidates("project-find-file", files);
    }
}

pub(crate) struct Finder {
    project: ProjectId,
    roots: Vec<PathBuf>,
    names: Vec<String>,
    waiting: bool,
}

impl Finder {
    pub(crate) fn path(&self, text: &str) -> PathBuf {
        if let Some((head, rest)) = text.split_once('/')
            && let Some(i) = self.names.iter().position(|n| n == head)
        {
            return self.roots[i].join(rest);
        }
        self.roots[0].join(text)
    }
}

/// Each indexed root's name: its last component, with `<2>` on for one an earlier root has.
pub(crate) fn root_names<'a>(roots: impl IntoIterator<Item = &'a Path>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for root in roots {
        let name = unique(&file_name(root), |n| names.iter().any(|taken| taken == n));
        names.push(name);
    }
    names
}

impl Ctx<'_> {
    fn project_write_intent(&mut self, project: Option<ProjectId>) -> WriteIntent {
        let core = &mut self.core;
        let project = project.unwrap_or_else(|| {
            let id = ProjectId(core.next_project);
            core.next_project += 1;
            id
        });
        let revision = core
            .project_at(project)
            .map_or(0, |i| core.projects[i].revision);
        core.project_writes += 1;
        let intent = WriteIntent {
            project,
            revision,
            request: core.project_writes,
        };
        core.pending_project_writes.insert(project, intent);
        intent
    }

    pub(crate) fn project_written(&mut self, intent: WriteIntent, result: Result<Loaded, String>) {
        let core = &mut self.core;
        let at = core.project_at(intent.project).ok();
        let current = core.pending_project_writes.get(&intent.project) == Some(&intent);
        if current {
            core.pending_project_writes.remove(&intent.project);
        }
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(err) => {
                if current {
                    core.echo(format!("project {err}"));
                }
                return;
            }
        };
        core.reload.project_revision += 1;
        if !current
            || at.map_or(intent.revision != 0, |i| {
                core.projects[i].revision != intent.revision
            })
        {
            self.reload_configuration();
            return;
        }
        let path = loaded
            .file
            .as_ref()
            .expect("written project has a file")
            .0
            .clone();
        match at {
            Some(i) => core.projects[i].replace(loaded),
            None => core.projects.push(Entry::new(intent.project, loaded)),
        }
        core.echo(format!("Wrote {}", path.display()));
    }

    pub(crate) fn create_project(&mut self, name: &str, primary: &str) -> Result<(), String> {
        if self.core.project(name).is_ok() {
            return Err(format!("Project {name} already exists"));
        }
        let project = Project {
            name: name.into(),
            watch: true,
            max_watched_files: None,
            default_agent: None,
            ignore: Vec::new(),
            env: Default::default(),
            primary: Root {
                path: self.core.root_path(primary),
                role: String::new(),
            },
            roots: Vec::new(),
        };
        self.write_new_project(&project, None)
    }

    pub(crate) fn save_project(&mut self, args: &Args) -> Result<(), String> {
        let loaded = &self.core.projects[self.core.project_arg(args)?];
        let name = &loaded.project.name;
        if let Some((path, _)) = &loaded.file {
            return Err(format!("{name} is already saved in {}", path.display()));
        }
        let project = loaded.project.clone();
        self.write_new_project(&project, Some(loaded.id))
    }

    /// Writes `project` to a new `<name>.toml` in the projects directory.
    fn write_new_project(
        &mut self,
        project: &Project,
        id: Option<ProjectId>,
    ) -> Result<(), String> {
        project.check()?;
        let name = &project.name;
        if name.contains('/') || name.starts_with('.') {
            return Err(format!("{name} can't be a file name"));
        }
        let taken = |p: &Entry| p.file.is_some() && p.project.name == *name;
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
        let intent = self.project_write_intent(id);
        self.effects.push(Effect::File(FileOp::WriteProject {
            intent,
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
        args: &Args,
        edit: impl FnOnce(&mut Project) -> Result<(), String>,
    ) -> Result<(), String> {
        let i = self.core.project_arg(args)?;
        let loaded = &mut self.core.projects[i];
        let mut project = loaded.project.clone();
        edit(&mut project)?;
        project.check()?;
        let Some((path, old)) = &loaded.file else {
            loaded.project = project;
            loaded.revision += 1;
            return Ok(());
        };
        let mut doc: DocumentMut = old.parse().map_err(|err| format!("{err}"))?;
        sync(&mut doc, &project);
        let (id, path, old) = (loaded.id, path.clone(), old.clone());
        let intent = self.project_write_intent(Some(id));
        self.effects.push(Effect::File(FileOp::WriteProject {
            intent,
            path,
            old: Some(old),
            text: doc.to_string(),
        }));
        Ok(())
    }

    pub(crate) fn add_root(&mut self, args: &Args) -> Result<(), String> {
        let path = self.core.root_path(&args.strings[1]);
        let role = args.strings[2].clone();
        self.edit_project(args, |p| {
            p.roots.push(Root { path, role });
            Ok(())
        })
    }

    pub(crate) fn remove_root(&mut self, args: &Args) -> Result<(), String> {
        let (name, path) = (&args.strings[0], self.core.root_path(&args.strings[1]));
        self.edit_project(args, |p| {
            let i = (p.roots.iter().position(|r| r.path == path))
                .ok_or_else(|| format!("{} is not an additional root of {name}", path.display()))?;
            p.roots.remove(i);
            Ok(())
        })
    }

    pub(crate) fn set_role(&mut self, args: &Args) -> Result<(), String> {
        let (name, path) = (&args.strings[0], self.core.root_path(&args.strings[1]));
        self.edit_project(args, |p| {
            let root = (std::iter::once(&mut p.primary).chain(&mut p.roots))
                .find(|r| r.path == path)
                .ok_or_else(|| format!("{} is not a root of {name}", path.display()))?;
            root.role = args.strings[2].clone();
            Ok(())
        })
    }

    pub(crate) fn set_default_agent(&mut self, args: &Args) -> Result<(), String> {
        let agent = &args.strings[1];
        if !agent.is_empty() && !self.core.settings.agents.contains_key(agent) {
            return Err(format!("{agent:?} is not a registered agent"));
        }
        self.edit_project(args, |p| {
            p.default_agent = (!agent.is_empty()).then(|| agent.clone());
            Ok(())
        })
    }

    pub(crate) fn set_watch(&mut self, args: &Args) -> Result<(), String> {
        let watch = args.strings[1]
            .parse::<bool>()
            .map_err(|_| "Watch must be true or false".to_string())?;
        self.edit_project(args, |p| {
            p.watch = watch;
            Ok(())
        })
    }

    /// Asks for a file in any root of the current workspace's project and visits it, indexing
    /// the project first when it has no index yet.
    pub(crate) fn project_find_file(&mut self, args: Args) -> Result<(), String> {
        let project = self.core.current_workspace().project;
        if let Some(text) = args.strings.first() {
            let path = self.core.finder(project)?.path(text);
            return self.find_file(&path.to_string_lossy());
        }
        if let Some(index) = self.core.index.get_mut(&project) {
            // No answer yet, as every answer lists the primary: the pending walk fills the list.
            index.announce_initial();
        } else {
            self.index_project(project, false)?;
        }
        let finder = self.core.finder(project)?;
        let files = if finder.waiting {
            Vec::new()
        } else {
            self.core.project_files(project)
        };
        let choices: Vec<&str> = files.iter().map(String::as_str).collect();
        let command = self.core.commands["project-find-file"];
        let prompt = format!("Find file in {}: ", self.core.project_name(project));
        self.read_string(command, args, prompt, Some(&choices))?;
        self.core
            .prompts
            .minibuffer
            .as_mut()
            .expect("opened finder")
            .finder = Some(finder);
        Ok(())
    }

    pub(crate) fn open_project_file(&mut self, args: &Args) -> Result<(), String> {
        let loaded = &self.core.projects[self.core.project_arg(args)?];
        let name = &loaded.project.name;
        match loaded.file.clone() {
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

    fn saved(h: &Harness, file: &str, name: &str) -> Loaded {
        let mut loaded = Loaded::ad_hoc(name, &h.path(name));
        loaded.project.watch = false;
        loaded.file = Some((h.path(file), toml::to_string(&loaded.project).unwrap()));
        loaded
    }

    #[test]
    fn reload_reconciles_paths_and_retains_bound_definitions_until_the_last_workspace_dies() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        let old = saved(&h, "b.toml", "old");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(old.clone())],
        });
        let project = h.core.project_id("old").unwrap();
        let workspace = h.new_workspace("task", "old");
        h.keys("text C-a C-SPC C-e C-x 2");
        let point = h.point();
        let renamed = saved(&h, "b.toml", "new");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(renamed.clone())],
        });
        assert_eq!(h.core.project_id("new").unwrap(), project);
        assert_eq!(h.core.current_workspace().name, "task");
        assert_eq!(h.core.current_workspace().project, project);
        assert_eq!((h.text(), h.point()), ("text".into(), point));
        assert_eq!(h.core.current().name, "*scratch*<task>");
        assert_eq!(h.core.frame().windows.len(), 2);
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Err("b.toml: invalid".into())],
        });
        let entry = &h.core.projects[h.core.project_at(project).unwrap()];
        assert_eq!(entry.file, renamed.file);
        assert_eq!(entry.project, renamed.project);
        assert_eq!(h.core.message(), Some("project b.toml: invalid"));
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![],
        });
        assert_eq!(h.core.project_id("new").unwrap(), project);
        let second = h.new_workspace("second", "new");
        h.core.kill_workspace(workspace).unwrap();
        assert_eq!(h.core.project_id("new").unwrap(), project);
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(old)],
        });
        assert_eq!(h.core.project_id("old").unwrap(), project);
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![],
        });
        h.core.index.entry(project).or_default();
        h.core.kill_workspace(second).unwrap();
        assert_eq!(h.core.project_names(false), ["home"]);
        assert_eq!(h.core.index.len(), 0);
    }

    #[test]
    fn duplicate_names_and_file_renames_do_not_redirect_existing_owners() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        let b = saved(&h, "b.toml", "same");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(b.clone())],
        });
        let b_id = h.core.project_id("same").unwrap();
        h.new_workspace("bound", "same");
        h.core.index.entry(b_id).or_default();
        let a = saved(&h, "a.toml", "same");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(b), Ok(a.clone())],
        });
        let a_id = h.core.project_id("same").unwrap();
        assert_eq!(
            h.core.projects[h.core.project_at(a_id).unwrap()].file,
            a.file
        );
        assert_eq!(h.core.current_workspace().project, b_id);
        assert!(h.core.index.contains_key(&b_id));
        let moved = saved(&h, "moved.toml", "same");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(moved.clone())],
        });
        assert_eq!(h.core.current_workspace().project, b_id);
        assert_eq!(
            h.core.projects[h
                .core
                .project_at(h.core.project_id("same").unwrap())
                .unwrap()]
            .file,
            moved.file
        );
        assert!(h.core.projects[h.core.project_at(b_id).unwrap()].retained);
    }

    #[test]
    fn a_project_selected_before_reload_keeps_its_identity_for_the_next_prompt() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        let b = saved(&h, "b.toml", "same");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(b)],
        });
        let selected = h.core.project_id("same").unwrap();
        h.keys("C-x p d same RET");
        let renamed = saved(&h, "b.toml", "renamed");
        let duplicate = saved(&h, "a.toml", "same");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(duplicate), Ok(renamed)],
        });
        h.keys("claude RET");
        let (intent, text) = std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::WriteProject { intent, text, .. }) => {
                    Some((intent, text))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(intent.project, selected);
        let parsed = Project::parse(&text).unwrap();
        assert_eq!(parsed.name, "renamed");
        assert_eq!(parsed.default_agent.as_deref(), Some("claude"));
    }

    #[test]
    fn saving_an_ad_hoc_project_keeps_its_workspaces_and_index() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        h.core.projects_dir = Some(h.path("projects"));
        let id = h.core.project_id("home").unwrap();
        h.new_workspace("second", "home");
        h.core.index.entry(id).or_default();
        h.keys("C-x p s home RET");
        let (intent, path, text) = std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::WriteProject {
                    intent, path, text, ..
                }) => Some((intent, path, text)),
                _ => None,
            })
            .unwrap();
        let project = Project::parse(&text).unwrap();
        h.send(Msg::ProjectWritten {
            intent,
            result: Ok(Loaded {
                project,
                file: Some((path.clone(), text)),
            }),
        });
        assert!(h.core.workspaces.iter().all(|w| w.project == id));
        assert!(h.core.index.contains_key(&id));
        assert_eq!(
            h.core.projects[h.core.project_at(id).unwrap()]
                .file
                .as_ref()
                .unwrap()
                .0,
            path
        );
    }

    #[test]
    fn a_current_write_error_is_reported_after_reload_changes_the_revision() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        let old = saved(&h, "p.toml", "p");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(old)],
        });
        let id = h.core.project_id("p").unwrap();
        h.keys("C-x p d p RET claude RET");
        let intent = std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::WriteProject { intent, .. }) => Some(intent),
                _ => None,
            })
            .unwrap();
        let latest = saved(&h, "p.toml", "latest");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(latest.clone())],
        });
        h.send(Msg::ProjectWritten {
            intent,
            result: Err("p.toml changed on disk; reload projects and retry".into()),
        });
        assert_eq!(
            h.core.message(),
            Some("project p.toml changed on disk; reload projects and retry")
        );
        assert_eq!(
            h.core.projects[h.core.project_at(id).unwrap()].project,
            latest.project
        );
    }

    #[test]
    fn a_superseded_write_requests_a_reload_without_replacing_the_new_definition() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        let old = saved(&h, "p.toml", "p");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(old.clone())],
        });
        let id = h.core.project_id("p").unwrap();
        h.keys("C-x p d p RET claude RET");
        let intent = std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::WriteProject { intent, .. }) => Some(intent),
                _ => None,
            })
            .unwrap();
        let latest = saved(&h, "p.toml", "latest");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(latest.clone())],
        });
        h.send(Msg::ProjectWritten {
            intent,
            result: Ok(old),
        });
        assert_eq!(
            h.core.projects[h.core.project_at(id).unwrap()].project,
            latest.project
        );
        assert!(
            h.effects
                .iter()
                .any(|e| matches!(e, crate::Effect::LoadConfiguration { .. }))
        );
    }

    /// A harness with directories `a` and `b`, projects going to `projects/` and `loaded` loaded.
    fn with(loaded: Vec<Loaded>) -> Harness {
        let mut h = Harness::new(&[("a/x", ""), ("b/x", "")]);
        h.listing("", &["a/", "b/"]);
        h.listing("a", &["x"]);
        h.listing("b", &["x"]);
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
            watch: true,
            max_watched_files: None,
            default_agent: None,
            ignore: Vec::new(),
            env: Default::default(),
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

    fn project_text(h: &Harness, name: &str) -> String {
        h.core.projects[h.core.project(name).unwrap()]
            .file
            .as_ref()
            .unwrap()
            .1
            .clone()
    }

    #[test]
    fn create_prepares_a_definition_and_accepts_the_written_reply() {
        let mut h = with(Vec::new());
        let a = h.path("a").display().to_string();
        let tmp = h.dir.as_path().display().to_string();
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
        assert_eq!(project_text(&h, "app"), text, "no empty roots or role");
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
    fn edits_prepare_preserved_comments_and_apply_only_successful_replies() {
        let mut h = with(Vec::new());
        let (a, b) = (
            h.path("a").display().to_string(),
            h.path("b").display().to_string(),
        );
        let text = format!(
            "# Mine\nname = \"app\"\nwatch = true # quiet\n\n[env]\nA = \"1\" # one\n\n[primary]\npath = \"{a}\"\n"
        );
        h.read_reply("projects/app.toml", Some(text.as_bytes()));
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
        call(&mut h, "project-set-watch", &["app", "false"]);
        let disk = project_text(&h, "app");
        assert!(disk.starts_with("# Mine\n"), "{disk}");
        assert!(disk.contains("watch = false # quiet"), "{disk}");
        assert!(disk.contains("[env]\nA = \"1\" # one\n"), "{disk}");
        let p = Project::parse(&disk).unwrap();
        assert_eq!(p, project(&h, "app"));
        assert!(!p.watch);
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
        let err = call(&mut h, "project-set-default-agent", &["app", "gemini"]);
        assert_eq!(err.as_deref(), Some("\"gemini\" is not a registered agent"));
        assert_eq!(project_text(&h, "app"), disk, "refusals write nothing");
        let before = project(&h, "app");
        h.collect_effects = true;
        call(&mut h, "project-set-role", &["app", &a, "Ours"]);
        let intent = std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::WriteProject { intent, .. }) => Some(intent),
                _ => None,
            })
            .unwrap();
        h.send(Msg::ProjectWritten {
            intent,
            result: Err("app.toml changed on disk; reload with C-x C-r".into()),
        });
        assert_eq!(
            h.core.message(),
            Some("project app.toml changed on disk; reload with C-x C-r")
        );
        assert_eq!(project(&h, "app"), before);
        assert_eq!(project_text(&h, "app"), disk);
        h.collect_effects = false;
        call(&mut h, "project-set-watch", &["app", "true"]);
        assert!(project(&h, "app").watch);
        assert!(project_text(&h, "app").contains("watch = true # quiet"));
        call(&mut h, "project-remove-root", &["app", &b]);
        call(&mut h, "project-set-default-agent", &["app", ""]);
        let p = project(&h, "app");
        assert_eq!((p.roots.len(), p.default_agent.as_deref()), (0, None));
        assert_eq!(Project::parse(&project_text(&h, "app")).unwrap(), p);
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
        call(&mut h, "project-set-watch", &["tmp", "false"]);
        let err = call(&mut h, "project-open-file", &["tmp"]);
        assert_eq!(
            err.as_deref(),
            Some("tmp has no file; save it with project-save")
        );
        assert_eq!(h.core.projects[h.core.project("tmp").unwrap()].file, None);
        let edited = project(&h, "tmp");
        assert_eq!(edited.roots[0].role, "Bee");
        assert!(!edited.watch);
        call(&mut h, "project-save", &["tmp"]);
        assert_eq!(Project::parse(&project_text(&h, "tmp")).unwrap(), edited);
        assert_eq!(
            h.core.projects(),
            [&edited],
            "the saved project replaces the ad-hoc one"
        );
        let err = call(&mut h, "project-save", &["tmp"]).unwrap();
        assert!(err.starts_with("tmp is already saved in "), "{err}");
    }

    #[test]
    fn watch_prompt_selects_the_current_value_and_rejects_other_text() {
        let mut h = with(Vec::new());
        h.core.add_project(ad_hoc(&h));
        h.keys("C-x p w tmp RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Watch: ");
        assert_eq!(
            crate::harness::listed(&mut h),
            (vec!["true".into(), "false".into()], Some(0))
        );
        h.keys("C-n RET");
        assert!(!project(&h, "tmp").watch);
        h.keys("M-x project-set-watch RET tmp RET");
        assert_eq!(
            crate::harness::listed(&mut h),
            (vec!["true".into(), "false".into()], Some(1))
        );
        h.keys("C-p RET");
        assert!(project(&h, "tmp").watch);
        assert_eq!(
            call(&mut h, "project-set-watch", &["tmp", "yes"]).as_deref(),
            Some("Watch must be true or false")
        );
        assert!(project(&h, "tmp").watch);
        assert_eq!(
            call(&mut h, "project-set-watch", &["missing", "false"]).as_deref(),
            Some("No project named missing")
        );
    }

    #[test]
    fn an_unknown_default_agent_is_reported_and_the_project_still_loads() {
        let mut h = with(Vec::new());
        let mut gemini = saved(&h, "projects/gemini.toml", "a");
        gemini.project.default_agent = Some("gemini".into());
        let mut claude = saved(&h, "projects/claude.toml", "b");
        claude.project.default_agent = Some("claude".into());
        h.send(Msg::ProjectsLoaded {
            dir: Some(h.path("projects")),
            projects: vec![Ok(gemini.clone()), Ok(claude)],
        });
        assert_eq!(
            h.core.message(),
            Some("project gemini.toml: \"gemini\" is not a registered agent")
        );
        assert_eq!(project(&h, "a").default_agent.as_deref(), Some("gemini"));
        h.core.message = None;
        h.send(Msg::SettingsLoaded(Ok(Some(
            "[agents.gemini]\ncommand = \"gemini\"".into(),
        ))));
        h.send(Msg::ProjectsLoaded {
            dir: Some(h.path("projects")),
            projects: vec![Ok(gemini)],
        });
        assert_eq!(h.core.message(), None);
    }

    #[test]
    fn default_agent_prompt_completes_registered_agents() {
        let mut h = with(Vec::new());
        h.core.add_project(ad_hoc(&h));
        h.send(Msg::SettingsLoaded(Ok(Some(
            "[agents.codex]\ncommand = \"codex-acp\"".into(),
        ))));
        h.keys("M-x project-set-default-agent RET tmp RET");
        let (agents, _) = crate::harness::listed(&mut h);
        assert_eq!(agents, ["claude", "codex"]);
        h.keys("C-n RET");
        assert_eq!(project(&h, "tmp").default_agent.as_deref(), Some("codex"));
        h.keys("M-x project-set-default-agent RET tmp RET M-RET");
        assert_eq!(project(&h, "tmp").default_agent, None);
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
            "project-set-watch",
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
        h.index_reply("a", &["x"], &["a"]);
        h.index_reply("b", &["x"], &["b"]);
        let mut loaded = ad_hoc(&h);
        for root in ["b"].iter().chain(roots) {
            loaded.project.roots.push(Root {
                path: h.path(root),
                role: String::new(),
            });
        }
        h.core.add_project(loaded);
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
        h.core.index[&h.core.project_id("tmp").unwrap()]
            .roots()
            .to_vec()
    }

    #[test]
    fn index_holds_each_roots_files_and_counts_them() {
        let mut h = indexable(&[]);
        h.index_reply("a", &[".env", "x"], &["a"]);
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
        h.indexes.insert(
            h.path("gone"),
            Err(format!("{}: missing root", h.path("gone").display())),
        );
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

    /// `indexable` with the current workspace bound to `tmp`.
    fn findable(roots: &[&str]) -> Harness {
        let mut h = indexable(roots);
        h.core.workspaces[0].project = h.core.project_id("tmp").unwrap();
        h
    }

    fn candidates(h: &mut Harness) -> Vec<String> {
        h.core.minibuffer().unwrap().candidates
    }

    fn visited(h: &Harness) -> Option<std::path::PathBuf> {
        h.core.current().file.clone()
    }

    fn annotation_requests(h: &mut Harness) -> Vec<(u64, Vec<(String, std::path::PathBuf)>)> {
        std::mem::take(&mut h.effects)
            .into_iter()
            .filter_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::Annotate { session, files }) => {
                    Some((session, files))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn project_files_start_unselected_and_support_navigation_tab_and_typed_ret() {
        let mut h = findable(&[]);
        h.index_reply("a", &["a", "x", "y"], &["a"]);
        h.index_reply("b", &["x", "y"], &["b"]);
        h.read_reply("a/a", Some(b"first"));
        h.keys("C-x p f");
        let original = shown(&h);
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(
            (mb.count.trim(), mb.selected, mb.files),
            ("*/5", None, true)
        );
        h.keys("C-n");
        assert_eq!(h.core.minibuffer().unwrap().count.trim(), "1/5");
        let buffer = h.core.windows.others[&h.core.windows.tree.leaves()[0]].buffer();
        assert_eq!(
            h.core.buffers.get(buffer).unwrap().text().to_string(),
            "first"
        );
        h.keys("x C-a C-k");
        assert_eq!(h.core.minibuffer().unwrap().count.trim(), "*/5");
        assert_eq!(shown(&h), original);
        h.keys("C-p");
        assert_eq!(h.core.minibuffer().unwrap().count.trim(), "5/5");
        h.keys("C-g C-x p f TAB");
        assert_eq!(h.text(), "a/a");
        h.keys("C-a C-k");
        assert_eq!(h.core.minibuffer().unwrap().count.trim(), "*/5");
        h.collect_effects = true;
        h.keys("RET");
        assert!(h.effects.iter().any(|effect| matches!(effect,
            crate::Effect::File(crate::FileOp::Open { path, .. }) if *path == h.path("a")
        )));
    }

    #[test]
    fn project_metadata_is_requested_only_for_newly_visible_files_and_never_matched() {
        let mut h = findable(&[]);
        let names: Vec<_> = (0..30).map(|i| format!("f{i:02}")).collect();
        let names: Vec<_> = names.iter().map(String::as_str).collect();
        h.index_reply("a", &names, &["a"]);
        h.index_reply("b", &[], &["b"]);
        h.keys("C-x p f");
        let requests = annotation_requests(&mut h);
        assert_eq!(requests.len(), 1);
        let (session, files) = &requests[0];
        assert_eq!(files.len(), 10);
        assert_eq!(files[0], ("a/f00".into(), h.path("a/f00")));
        assert_eq!(files[9], ("a/f09".into(), h.path("a/f09")));
        h.send(Msg::Annotated {
            session: *session,
            entries: files
                .iter()
                .map(|(name, _)| crate::FileEntry {
                    name: name.clone(),
                    annotation: "-rw-r--r--      41 B       5s ago".into(),
                })
                .collect(),
        });
        assert_eq!(h.core.minibuffer().unwrap().annotations.len(), 10);
        assert!(
            h.core
                .minibuffer()
                .unwrap()
                .annotations
                .iter()
                .all(|n| n.contains("41 B"))
        );
        assert_eq!(annotation_requests(&mut h), Vec::new());
        h.keys("C-u 11 C-n");
        let next = annotation_requests(&mut h);
        assert_eq!(next, [(*session, vec![("a/f10".into(), h.path("a/f10"))])]);
        h.keys("C-p");
        assert_eq!(annotation_requests(&mut h), Vec::new());
        h.send(Msg::Insert {
            text: "41".into(),
            replace: None,
        });
        assert_eq!(candidates(&mut h), Vec::<String>::new());
        h.keys("C-a C-k");
        assert_eq!(annotation_requests(&mut h), Vec::new());
        assert!(h.core.minibuffer().unwrap().annotations[0].contains("41 B"));
    }

    #[test]
    fn project_metadata_ignores_replies_from_a_closed_session() {
        let mut h = findable(&[]);
        h.keys("C-x p f");
        let first = annotation_requests(&mut h).remove(0);
        h.keys("C-g C-x p f");
        let second = annotation_requests(&mut h).remove(0);
        let reply = |session, note: &str| Msg::Annotated {
            session,
            entries: vec![crate::FileEntry {
                name: "a/x".into(),
                annotation: note.into(),
            }],
        };
        h.send(reply(first.0, "stale"));
        assert_eq!(h.core.minibuffer().unwrap().annotations, ["", ""]);
        h.send(reply(second.0, "current"));
        assert_eq!(h.core.minibuffer().unwrap().annotations, ["current", ""]);
    }

    #[test]
    fn project_file_completion_highlights_the_root_and_path() {
        let mut h = findable(&[]);
        h.keys("C-x p f bx");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.candidates, ["b/x"]);
        assert_eq!(mb.highlights, [vec![0..1, 2..3]]);
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
        h.read_reply("q/a/y", Some(b""));
        h.index_reply("q/a", &["y"], &["q/a"]);
        h.keys("C-x p f");
        assert_eq!(candidates(&mut h), ["a/x", "b/x", "a<2>/y"]);
        h.keys("C-g");
        call(&mut h, "project-find-file", &["a<2>/y"]);
        assert_eq!(visited(&h), Some(h.path("q/a/y")));
    }

    #[test]
    fn a_typed_name_is_visited_under_its_root_or_the_primary() {
        let mut h = findable(&[]);
        h.read_reply("b/new.rs", None);
        h.read_reply("a/notes.md", None);
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
        assert_eq!(h.core.minibuffer().unwrap().count.trim(), "*/0");
        let original = shown(&h);
        h.run(pending);
        assert_eq!(candidates(&mut h), ["a/x", "b/x"]);
        assert_eq!(h.core.minibuffer().unwrap().count.trim(), "*/2");
        assert_eq!(shown(&h), original);
        h.keys("C-g C-x p f");
        assert_eq!(h.core.index_requests(), 1, "no second walk");
        assert_eq!(candidates(&mut h), ["a/x", "b/x"]);
    }

    #[test]
    fn typed_root_prefixes_resolve_while_the_first_index_is_held() {
        for (typed, expected) in [("b/new.rs", "b/new.rs"), ("a<2>/new.rs", "q/a/new.rs")] {
            let mut h = findable(&["q/a"]);
            h.collect_effects = true;
            h.keys("C-x p f");
            assert_eq!(candidates(&mut h), Vec::<String>::new());
            h.keys(&format!("{typed} M-RET"));
            let paths: Vec<_> = h
                .effects
                .iter()
                .filter_map(|e| match e {
                    crate::Effect::File(crate::FileOp::Open { path, .. }) => Some(path.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(paths, [h.path(expected)]);
        }
    }

    fn saved_finder() -> (Harness, Loaded) {
        let mut h = Harness::new(&[("old/lib/x", "old"), ("new/lib/x", "new")]);
        h.index_reply("old/lib", &["x"], &["old/lib"]);
        h.index_reply("new/lib", &["x"], &["new/lib"]);
        h.read_reply("old/lib/new.rs", None);
        let mut loaded = Loaded::ad_hoc("p", &h.path("old/lib"));
        loaded.project.watch = false;
        loaded.file = Some((h.path("p.toml"), String::new()));
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(loaded.clone())],
        });
        h.new_workspace("p", "p");
        (h, loaded)
    }

    #[test]
    fn project_metadata_uses_captured_duplicate_root_labels_after_reload() {
        let (mut h, mut loaded) = saved_finder();
        loaded.project.roots.push(Root {
            path: h.path("new/lib"),
            role: String::new(),
        });
        let names: Vec<_> = (0..12).map(|i| format!("f{i:02}")).collect();
        h.index_reply(
            "old/lib",
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
            &["old/lib"],
        );
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(loaded.clone())],
        });
        h.keys("C-x p f");
        let first = annotation_requests(&mut h).remove(0);
        assert_eq!(first.1.len(), 10);
        std::mem::swap(&mut loaded.project.primary, &mut loaded.project.roots[0]);
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(loaded)],
        });
        h.keys("lib<2>/x");
        let next = annotation_requests(&mut h).remove(0);
        assert_eq!(next.1, [("lib<2>/x".into(), h.path("new/lib/x"))]);
        h.send(Msg::Annotated {
            session: next.0,
            entries: vec![crate::FileEntry {
                name: "lib<2>/x".into(),
                annotation: "captured".into(),
            }],
        });
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.candidates, ["lib<2>/x"]);
        assert_eq!(mb.annotations, ["captured"]);
    }

    #[test]
    fn finder_preview_candidates_and_typed_paths_keep_their_original_mapping() {
        for accept in ["RET", "lib/new.rs M-RET"] {
            let (mut h, mut loaded) = saved_finder();
            h.keys("C-x p f");
            assert_eq!(candidates(&mut h), ["lib/x"]);
            loaded.project.primary.path = h.path("new/lib");
            h.send(Msg::ProjectsLoaded {
                dir: None,
                projects: vec![Ok(loaded)],
            });
            assert_eq!(candidates(&mut h), ["lib/x"]);
            h.keys("zzz C-a C-k C-n");
            let origin = h.core.windows.others[&h.core.windows.tree.leaves()[0]].buffer();
            assert_eq!(
                h.core.buffers.get(origin).unwrap().text().to_string(),
                "old"
            );
            h.keys(accept);
            let file = if accept == "RET" { "x" } else { "new.rs" };
            assert_eq!(visited(&h), Some(h.path(&format!("old/lib/{file}"))));
            h.keys("C-x p f C-n RET");
            assert_eq!(visited(&h), Some(h.path("new/lib/x")));
        }
    }

    #[test]
    fn incompatible_initial_results_stay_out_and_reopened_finders_use_current_roots() {
        let (mut h, mut loaded) = saved_finder();
        h.collect_effects = true;
        h.keys("C-x p f");
        let take_walk = |h: &mut Harness| {
            std::mem::take(&mut h.effects)
                .into_iter()
                .find_map(|e| match e {
                    crate::Effect::File(crate::FileOp::Index {
                        project,
                        walk,
                        roots,
                        ..
                    }) => Some((project, walk, roots)),
                    _ => None,
                })
                .unwrap()
        };
        let answer = |(project, walk, roots): (_, _, Vec<std::path::PathBuf>)| Msg::Indexed {
            project,
            walk,
            roots: roots
                .into_iter()
                .map(|root| (root, Ok(vec!["x".into()])))
                .collect(),
            directories: Vec::new(),
        };
        let initial = take_walk(&mut h);
        loaded.project.primary.path = h.path("new/lib");
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(loaded)],
        });
        h.send(answer(initial));
        let latest = take_walk(&mut h);
        h.send(answer(latest));
        assert_eq!(candidates(&mut h), Vec::<String>::new());
        h.keys("lib/new.rs M-RET");
        assert!(h.effects.iter().any(|e| matches!(e, crate::Effect::File(crate::FileOp::Open { path, .. }) if *path == h.path("old/lib/new.rs"))));
        h.keys("C-x p f");
        assert_eq!(candidates(&mut h), ["lib/x"]);
        h.keys("C-n RET");
        assert!(h.effects.iter().any(|e| matches!(e, crate::Effect::File(crate::FileOp::Open { path, .. }) if *path == h.path("new/lib/x"))));
    }

    #[test]
    fn duplicate_labels_survive_reordering_and_a_reopened_finder_waits_for_compatible_files() {
        let (mut h, mut loaded) = saved_finder();
        loaded.project.roots.push(Root {
            path: h.path("new/lib"),
            role: String::new(),
        });
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(loaded.clone())],
        });
        h.keys("C-x p f");
        assert_eq!(candidates(&mut h), ["lib/x", "lib<2>/x"]);
        h.collect_effects = true;
        std::mem::swap(&mut loaded.project.primary, &mut loaded.project.roots[0]);
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(loaded)],
        });
        h.keys("lib<2>/new.rs M-RET");
        assert!(h.effects.iter().any(|e| matches!(e, crate::Effect::File(crate::FileOp::Open { path, .. }) if *path == h.path("new/lib/new.rs"))));
        h.effects.clear();
        h.keys("C-x p f");
        assert_eq!(candidates(&mut h), Vec::<String>::new());
        h.keys("lib<2>/new.rs M-RET");
        assert!(h.effects.iter().any(|e| matches!(e, crate::Effect::File(crate::FileOp::Open { path, .. }) if *path == h.path("old/lib/new.rs"))));
    }

    /// The name of the buffer the first window shows while the minibuffer is open.
    fn shown(h: &Harness) -> String {
        let id = h.core.windows.others[&h.core.windows.tree.leaves()[0]].buffer();
        h.core.buffers.get(id).unwrap().name.clone()
    }

    #[test]
    fn the_finder_previews_files_in_a_hidden_buffer() {
        let mut h = findable(&[]);
        h.read_reply("b/x", Some("bee".as_ref()));
        h.keys("C-x p f C-n C-n");
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
        h.keys("C-x p f C-n C-n RET");
        assert_eq!(visited(&h), Some(h.path("b/x")));
        assert_eq!((name(&h), h.core.current().read_only), ("x", false));
    }

    #[test]
    fn the_finder_previews_a_visited_file_s_own_buffer() {
        let mut h = findable(&[]);
        h.open("b/x").open("a/x");
        h.keys("C-x p f C-n C-n");
        assert_eq!(shown(&h), "x");
    }

    #[test]
    fn accepting_a_prepared_preview_reads_the_file_afresh() {
        let mut h = findable(&[]);
        h.keys("C-x p f C-n");
        let peek = h.core.buffers.visiting(&h.path("a/x")).unwrap();
        h.read_reply("a/x", Some("changed after preview".as_ref()));
        h.keys("RET");
        assert_eq!(h.text(), "changed after preview");
        assert_ne!(h.core.windows.window.buffer(), peek);
        assert!(!h.core.current().read_only);
    }

    #[test]
    fn a_workspace_switch_releases_prepared_previews() {
        let mut h = findable(&[]);
        h.keys("origin C-f");
        let main = h.core.workspace();
        let other = h.new_workspace("other", "tmp");
        h.core.switch_workspace(main).unwrap();
        let original = h.core.windows.window.buffer();
        h.keys("C-x p f C-n");
        let peek = h.core.buffers.visiting(&h.path("a/x")).unwrap();
        h.core.switch_workspace(other).unwrap();
        h.core.switch_workspace(main).unwrap();
        assert_eq!(h.core.windows.window.buffer(), original);
        assert_eq!(h.text(), "origin");
        assert!(h.core.buffers.get(peek).is_none());
    }

    #[test]
    fn a_read_no_longer_awaited_is_dropped() {
        let mut h = findable(&[]);
        h.keys("C-x p f C-n");
        let result = Ok(Some(crate::buffer::prepare_preview(b"late").unwrap()));
        h.send(Msg::Previewed {
            request: crate::PreviewRequest {
                session: 0,
                revision: 0,
                path: h.path("b/x"),
            },
            result,
        });
        assert_eq!(shown(&h), " x");
        assert!(h.core.buffers.visiting(&h.path("b/x")).is_none());
        assert!(h.core.buffers.visiting(&h.path("a/x")).is_some());
    }
}
