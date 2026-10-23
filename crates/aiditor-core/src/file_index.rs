//! Project index transitions and snapshots; disk walks run in services.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::project::{Project, ProjectId};
use crate::{Core, Ctx, Disk, Effect, FileOp};

pub(crate) type Scope = (Vec<PathBuf>, Vec<String>);

pub(crate) fn scope(project: &Project) -> Scope {
    (
        project.all_roots().map(|root| root.path.clone()).collect(),
        project.ignore.clone(),
    )
}

#[derive(Default)]
pub(crate) struct FileIndex {
    requests: u64,
    active: Option<u64>,
    pending: Option<u64>,
    requested: Option<Scope>,
    accepted: Option<Scope>,
    ready: bool,
    loud: bool,
    roots: Vec<(PathBuf, Vec<PathBuf>)>,
    directories: usize,
    missed: BTreeSet<PathBuf>,
}

enum Reply {
    Obsolete,
    Walk(u64, Scope),
    Accepted { loud: bool, errors: String },
}

impl FileIndex {
    fn request(&mut self, scope: Scope, quiet: bool) -> Option<(u64, Scope)> {
        if self.requested.as_ref() != Some(&scope) {
            self.missed.clear();
            self.ready = false;
        }
        self.requests += 1;
        self.requested = Some(scope.clone());
        self.loud |= !quiet;
        if self.active.is_some() {
            self.pending = Some(self.requests);
            None
        } else {
            self.active = Some(self.requests);
            Some((self.requests, scope))
        }
    }

    fn apply(
        &mut self,
        walk: u64,
        roots: Vec<(PathBuf, Result<Vec<PathBuf>, String>)>,
        directories: usize,
    ) -> Reply {
        if self.active != Some(walk) {
            return Reply::Obsolete;
        }
        self.active = self.pending.take();
        if let Some(walk) = self.active {
            return Reply::Walk(walk, self.requested.clone().expect("requested walk"));
        }
        let mut errors = String::new();
        self.roots = roots
            .into_iter()
            .map(|(root, files)| {
                let files = files.unwrap_or_else(|err| {
                    errors.push_str(&format!("; {err}"));
                    Vec::new()
                });
                (root, files)
            })
            .collect();
        self.ready = errors.is_empty();
        self.accepted = self.requested.clone();
        self.directories = directories;
        Reply::Accepted {
            loud: std::mem::take(&mut self.loud),
            errors,
        }
    }

    pub(crate) fn roots(&self) -> &[(PathBuf, Vec<PathBuf>)] {
        &self.roots
    }

    pub(crate) fn counts(&self, scope: &Scope) -> Option<(usize, usize)> {
        (self.ready && self.accepted.as_ref() == Some(scope) && self.requested(scope)).then(|| {
            (
                self.roots.iter().map(|(_, files)| files.len()).sum(),
                self.directories,
            )
        })
    }

    pub(crate) fn requested(&self, scope: &Scope) -> bool {
        self.requested.as_ref() == Some(scope)
    }

    pub(crate) fn announce_initial(&mut self) {
        self.loud |= self.roots.is_empty();
    }

    /// Whether a reported path under one of the index's roots is a file it lacks or a directory
    /// holding none of its files (an empty one changes the directory count), or is gone while it
    /// holds that file or files under it. Such a path starts a walk once, so writes to a file the
    /// walk ignores (a visited `.env`) start none.
    // shortcut: a create or delete re-walks the whole project; update the index per path if large projects lag.
    pub(crate) fn stale(&mut self, paths: &[(PathBuf, Disk)]) -> bool {
        let mut stale = false;
        for (path, disk) in paths {
            for (root, files) in &self.roots {
                let Ok(rel) = path.strip_prefix(root) else {
                    continue;
                };
                stale |= match disk {
                    Disk::File => {
                        !files.iter().any(|f| f == rel) && self.missed.insert(path.clone())
                    }
                    Disk::Gone => {
                        self.missed.remove(path);
                        files.iter().any(|f| f.starts_with(rel))
                    }
                    Disk::Dir => {
                        !files.iter().any(|f| f.starts_with(rel))
                            && self.missed.insert(path.clone())
                    }
                };
            }
        }
        stale
    }
}

impl Core {
    pub(crate) fn release_indexes(&mut self) {
        self.index
            .retain(|id, _| self.projects.iter().any(|p| p.id == *id));
    }

    #[cfg(test)]
    pub(crate) fn index_requests(&self) -> u64 {
        self.index.values().map(|index| index.requests).sum()
    }
}

impl Ctx<'_> {
    pub(crate) fn index_project(&mut self, project: ProjectId, quiet: bool) -> Result<(), String> {
        let scope = scope(&self.core.projects[self.core.project_at(project)?].project);
        let next = self
            .core
            .index
            .entry(project)
            .or_default()
            .request(scope, quiet);
        if !quiet {
            self.core
                .echo(format!("Indexing {}...", self.core.project_name(project)));
        }
        if let Some((walk, (roots, ignore))) = next {
            self.effects.push(Effect::File(FileOp::Index {
                project,
                walk,
                roots,
                ignore,
            }));
        }
        Ok(())
    }

    pub(crate) fn indexed(
        &mut self,
        project: ProjectId,
        walk: u64,
        roots: Vec<(PathBuf, Result<Vec<PathBuf>, String>)>,
        directories: usize,
    ) {
        if self.core.project_at(project).is_err() {
            return;
        }
        let Some(index) = self.core.index.get_mut(&project) else {
            return;
        };
        match index.apply(walk, roots, directories) {
            Reply::Obsolete => {}
            Reply::Walk(walk, (roots, ignore)) => {
                self.effects.push(Effect::File(FileOp::Index {
                    project,
                    walk,
                    roots,
                    ignore,
                }));
            }
            Reply::Accepted { loud, errors } => {
                if !loud {
                    if !errors.is_empty() {
                        tracing::info!(?project, errors, "re-walk after a file change");
                    }
                    return;
                }
                let n: usize = index.roots.iter().map(|(_, files)| files.len()).sum();
                let s = if n == 1 { "" } else { "s" };
                self.core.echo(format!(
                    "Indexed {n} file{s} in {}{errors}",
                    self.core.project_name(project)
                ));
                if self.core.current_workspace().project == project {
                    let files = self.core.project_files(project);
                    self.core.add_candidates("project-find-file", files);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Msg;
    use crate::harness::Harness;
    use crate::project::Loaded;

    type Walk = (ProjectId, u64, Vec<PathBuf>, Vec<String>);

    fn setup() -> (Harness, ProjectId) {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        let mut loaded = Loaded::ad_hoc("p", &PathBuf::from("/root-a"));
        loaded.file = Some(("/p.toml".into(), String::new()));
        let project = h.core.add_project(loaded);
        h.core.new_workspace("p", project);
        h.send(Msg::Tick);
        (h, project)
    }

    fn walks(h: &mut Harness) -> Vec<Walk> {
        std::mem::take(&mut h.effects)
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::File(FileOp::Index {
                    project,
                    walk,
                    roots,
                    ignore,
                }) => Some((project, walk, roots, ignore)),
                _ => None,
            })
            .collect()
    }

    fn request(h: &mut Harness, project: ProjectId, quiet: bool) {
        let mut ctx = h.ctx();
        ctx.index_project(project, quiet).unwrap();
        let effects = ctx.effects;
        h.run(effects);
    }

    fn answer(walk: &Walk, files: &[&str], directories: usize) -> Msg {
        Msg::Indexed {
            project: walk.0,
            walk: walk.1,
            roots: walk
                .2
                .iter()
                .map(|root| (root.clone(), Ok(files.iter().map(PathBuf::from).collect())))
                .collect(),
            directories,
        }
    }

    #[test]
    fn a_burst_runs_only_the_latest_scope_and_announces_once() {
        let (mut h, project) = setup();
        let first = walks(&mut h).pop().unwrap();
        request(&mut h, project, false);
        for root in ["/root-b", "/root-c"] {
            let at = h.core.project_at(project).unwrap();
            h.core.projects[at].project.primary.path = root.into();
            h.core.projects[at].project.ignore = vec![format!("{root}.ignore")];
            request(&mut h, project, true);
        }
        h.keys("typing");
        assert_eq!(h.text(), "typing");
        assert_eq!(walks(&mut h).len(), 0);
        h.send(answer(&first, &["obsolete"], 1));
        let pending = walks(&mut h);
        assert_eq!(pending.len(), 1);
        let latest = &pending[0];
        assert_eq!(
            (&latest.2[..], &latest.3[..]),
            (
                &[PathBuf::from("/root-c")][..],
                &["/root-c.ignore".to_string()][..]
            )
        );
        h.send(answer(&first, &["duplicate"], 1));
        assert_eq!(walks(&mut h).len(), 0);
        h.send(answer(latest, &["final"], 2));
        assert_eq!(h.core.message(), Some("Indexed 1 file in p"));
        assert_eq!(
            h.core.index[&project].roots(),
            &[(PathBuf::from("/root-c"), vec![PathBuf::from("final")])]
        );
        h.send(answer(&first, &["late"], 99));
        let messages = h
            .core
            .buffers
            .recent()
            .find(|(_, b)| b.name == "*Messages*")
            .unwrap()
            .1
            .text()
            .to_string();
        assert_eq!(messages.matches("Indexed 1 file in p").count(), 1);
        assert_eq!(walks(&mut h).len(), 0);
    }

    #[test]
    fn same_scope_refreshes_preserve_snapshot_counts_watches_and_mode_lines() {
        let (mut h, project) = setup();
        let first = walks(&mut h).pop().unwrap();
        h.send(answer(&first, &["old"], 2));
        let accepted = h.core.watch_set();
        let line = h.frame(800., 600.).windows[0].display.mode_line.clone();
        request(&mut h, project, false);
        let active = walks(&mut h).pop().unwrap();
        request(&mut h, project, true);
        assert_eq!(h.core.watch_set(), accepted);
        assert_eq!(h.frame(800., 600.).windows[0].display.mode_line, line);
        assert_eq!(
            h.core.index[&project].counts(&(first.2.clone(), first.3.clone())),
            Some((1, 2))
        );
        h.send(answer(&active, &["superseded"], 1000));
        let latest = walks(&mut h).pop().unwrap();
        assert_eq!(h.core.watch_set(), accepted);
        assert_eq!(h.core.index[&project].roots()[0].1, [PathBuf::from("old")]);
        h.send(answer(&latest, &["new", "second"], 3));
        assert_eq!(
            h.core.index[&project].counts(&(first.2, first.3)),
            Some((2, 3))
        );
        assert_eq!(h.core.watch_set(), accepted);
    }

    #[test]
    fn changed_scopes_wait_for_fresh_counts_and_partial_errors_keep_reporting() {
        let (mut h, project) = setup();
        let first = walks(&mut h).pop().unwrap();
        h.send(answer(&first, &["old"], 1));
        let at = h.core.project_at(project).unwrap();
        h.core.projects[at].project.primary.path = "/new".into();
        h.core.projects[at]
            .project
            .roots
            .push(crate::project::Root {
                path: "/missing".into(),
                role: String::new(),
            });
        request(&mut h, project, false);
        let next = walks(&mut h).pop().unwrap();
        assert_eq!(h.core.index[&project].roots()[0].1, [PathBuf::from("old")]);
        assert_eq!(h.core.watch_set().roots, vec![]);
        assert!(
            h.frame(800., 600.).windows[0]
                .display
                .mode_line
                .ends_with("[unwatched]")
        );
        h.send(Msg::Indexed {
            project,
            walk: next.1,
            roots: vec![
                ("/new".into(), Ok(vec!["good".into()])),
                ("/missing".into(), Err("/missing: unavailable".into())),
            ],
            directories: 1,
        });
        assert_eq!(
            h.core.message(),
            Some("Indexed 1 file in p; /missing: unavailable")
        );
        assert_eq!(h.core.watch_set().roots, vec![]);
        assert_eq!(
            h.core.index[&project].roots(),
            &[
                (PathBuf::from("/new"), vec![PathBuf::from("good")]),
                (PathBuf::from("/missing"), vec![])
            ]
        );
        request(&mut h, project, true);
        let recovery = walks(&mut h).pop().unwrap();
        h.send(answer(&recovery, &["good"], 2));
        assert_eq!(
            h.core.watch_set().roots,
            vec![
                (PathBuf::from("/missing"), vec![]),
                (PathBuf::from("/new"), vec![])
            ]
        );
    }

    #[test]
    fn changing_scope_back_still_waits_for_the_queued_result() {
        let (mut h, project) = setup();
        let first = walks(&mut h).pop().unwrap();
        h.send(answer(&first, &["old"], 1));
        let at = h.core.project_at(project).unwrap();
        h.core.projects[at].project.ignore = vec!["*.log".into()];
        request(&mut h, project, true);
        let changed = walks(&mut h).pop().unwrap();
        h.core.projects[at].project.ignore.clear();
        request(&mut h, project, true);
        assert_eq!(h.core.watch_set().roots, vec![]);
        h.send(answer(&changed, &["intermediate"], 1));
        let latest = walks(&mut h).pop().unwrap();
        assert_eq!(h.core.watch_set().roots, vec![]);
        h.send(answer(&latest, &["fresh"], 2));
        assert_eq!(
            h.core.watch_set().roots,
            vec![(PathBuf::from("/root-a"), vec![])]
        );
    }

    #[test]
    fn releasing_a_project_discards_its_active_and_queued_work() {
        let (mut h, project) = setup();
        let first = walks(&mut h).pop().unwrap();
        request(&mut h, project, false);
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![],
        });
        h.core.kill_workspace(h.core.workspace()).unwrap();
        h.core.echo("remaining workspace".into());
        h.send(answer(&first, &["late"], 1));
        assert_eq!(walks(&mut h).len(), 0);
        assert_eq!(h.core.message(), Some("remaining workspace"));
        assert_eq!(h.core.index.get(&project).map(FileIndex::roots), None);
    }
}
