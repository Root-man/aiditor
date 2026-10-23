//! Shared configuration reads for startup and runtime reloads.

use crate::projects;
use aiditor_core::{Msg, configuration::Bundle};
use std::io::ErrorKind;
use std::path::Path;

pub fn load(dir: Option<&Path>, request: u64) -> Msg {
    let settings = read_config(dir, "settings.toml");
    let keymap = read_config(dir, "keymap.toml");
    let projects_dir = dir.map(|dir| dir.join("projects"));
    let projects = projects_dir
        .as_deref()
        .map_or_else(Vec::new, projects::read);
    Msg::ConfigurationLoaded {
        request,
        bundle: Bundle {
            settings,
            keymap,
            projects_dir,
            projects,
        },
    }
}

/// The text of the config file `name`; `None` when there is no file or no config dir.
pub fn read_config(dir: Option<&Path>, name: &str) -> Result<Option<String>, String> {
    let Some(path) = dir.map(|dir| dir.join(name)) else {
        return Ok(None);
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

/// The project files in the config dir's `projects/`, for the core.
pub fn read_projects(dir: Option<&Path>) -> Msg {
    let dir = dir.map(|dir| dir.join("projects"));
    let projects = dir.as_deref().map_or_else(Vec::new, projects::read);
    Msg::ProjectsLoaded { dir, projects }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aiditor_core::{Core, Ctx, Effect, FileOp, command::Args};
    use std::{fs, time::Instant};

    fn call(core: &mut Core, name: &str, strings: &[&str]) -> Vec<Effect> {
        let mut ctx = Ctx {
            core,
            now: Instant::now(),
            effects: Vec::new(),
        };
        ctx.call(
            name,
            Args {
                strings: strings.iter().map(|s| (*s).into()).collect(),
                ..Default::default()
            },
        );
        ctx.effects
    }

    fn request(effects: Vec<Effect>) -> u64 {
        let requests: Vec<_> = effects
            .into_iter()
            .filter_map(|e| match e {
                Effect::LoadConfiguration { request } => Some(request),
                _ => None,
            })
            .collect();
        assert_eq!(requests.len(), 1);
        requests[0]
    }

    #[test]
    fn project_writes_and_reload_snapshots_reconcile_in_both_reply_orders() {
        for write_first in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let file = root.join("projects/p.toml");
            let text = format!("name = 'p'\nwatch = false\n[primary]\npath = {root:?}");
            projects::write(&file, None, text).unwrap();
            let mut core = Core::default();
            core.handle(Instant::now(), read_projects(Some(root)));
            let id = core.project_id("p").unwrap();
            core.new_workspace("task", id);
            let edit = call(&mut core, "project-set-default-agent", &["p", "codex"])
                .into_iter()
                .find_map(|e| match e {
                    Effect::File(op @ FileOp::WriteProject { .. }) => Some(op),
                    _ => None,
                })
                .unwrap();
            let first = request(call(&mut core, "reload-settings", &[]));
            let old = load(Some(root), first);
            let written = crate::files::run(edit);
            let refresh = if write_first {
                core.handle(Instant::now(), written);
                assert_eq!(
                    core.projects()
                        .into_iter()
                        .find(|p| p.name == "p")
                        .unwrap()
                        .default_agent
                        .as_deref(),
                    Some("codex")
                );
                let effects = core.handle(Instant::now(), old);
                assert_eq!(
                    core.projects()
                        .into_iter()
                        .find(|p| p.name == "p")
                        .unwrap()
                        .default_agent
                        .as_deref(),
                    Some("codex")
                );
                request(effects)
            } else {
                core.handle(Instant::now(), old);
                request(core.handle(Instant::now(), written))
            };
            core.handle(Instant::now(), load(Some(root), refresh));
            assert_eq!(core.project_id("p").unwrap(), id);
            assert_eq!(
                core.projects()
                    .into_iter()
                    .find(|p| p.name == "p")
                    .unwrap()
                    .default_agent
                    .as_deref(),
                Some("codex")
            );
            assert!(fs::read_to_string(file).unwrap().contains("codex"));
        }
    }

    #[test]
    fn loader_keeps_component_errors_separate_and_validates_project_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("settings.toml")).unwrap();
        fs::write(dir.path().join("keymap.toml"), "[global]").unwrap();
        fs::create_dir(dir.path().join("projects")).unwrap();
        fs::write(dir.path().join("projects/bad.toml"), "name = 'bad'").unwrap();
        let Msg::ConfigurationLoaded { request, bundle } = load(Some(dir.path()), 42) else {
            panic!()
        };
        assert_eq!(request, 42);
        assert!(bundle.settings.unwrap_err().contains("settings.toml"));
        assert_eq!(bundle.keymap, Ok(Some("[global]".into())));
        assert!(
            bundle.projects[0]
                .as_ref()
                .unwrap_err()
                .contains("bad.toml")
        );
        let Msg::ConfigurationLoaded { bundle, .. } = load(None, 43) else {
            panic!()
        };
        assert_eq!(
            (bundle.settings, bundle.keymap, bundle.projects),
            (Ok(None), Ok(None), vec![])
        );
    }

    #[test]
    fn a_held_configuration_worker_leaves_input_responsive() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.toml"), "font_size = 18").unwrap();
        let mut core = Core::default();
        let id = request(call(&mut core, "reload-settings", &[]));
        let (started, running) = std::sync::mpsc::channel();
        let (release, held) = std::sync::mpsc::channel();
        let path = dir.path().to_path_buf();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            held.recv().unwrap();
            load(Some(&path), id)
        });
        running.recv().unwrap();
        core.handle(
            Instant::now(),
            Msg::Insert {
                text: "typed during reload".into(),
                replace: None,
            },
        );
        assert_eq!(core.text(), "typed during reload");
        release.send(()).unwrap();
        core.handle(Instant::now(), worker.join().unwrap());
        assert_eq!(core.settings().font_size, 18.);
        assert_eq!(core.text(), "typed during reload");
    }
}
