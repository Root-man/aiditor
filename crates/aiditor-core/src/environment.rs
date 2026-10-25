//! Each project's process environment: captured from the login shell when the app was not
//! launched from one, else the app's own.

use crate::command::Args;
use crate::project::ProjectId;
use crate::{Core, Ctx, Effect};

#[derive(Default)]
pub(crate) struct Environment {
    /// The request number of the running capture, if any.
    capturing: Option<u64>,
    /// The last capture's variables, kept while a re-capture runs; `None` before the first
    /// answer or after a failed one, when the app's own environment is used.
    vars: Option<Vec<(String, String)>>,
}

impl Core {
    /// Captures each project's environment from now on: the app was not launched from a shell.
    pub fn capture_environments(&mut self) {
        self.capture = true;
    }
}

impl Ctx<'_> {
    /// Starts a capture for each project that has a workspace and none yet.
    pub(crate) fn ensure_environments(&mut self) {
        let core = &*self.core;
        if !core.capture {
            return;
        }
        let missing: Vec<_> = (core.projects.iter())
            .filter(|p| !core.environments.contains_key(&p.id))
            .filter(|p| core.workspaces.iter().any(|w| w.project == p.id))
            .map(|p| p.id)
            .collect();
        for project in missing {
            self.capture_environment(project);
        }
    }

    /// Captures `project`'s environment in its primary repo, superseding any running capture.
    fn capture_environment(&mut self, project: ProjectId) {
        let core = &mut *self.core;
        let Ok(at) = core.project_at(project) else {
            return;
        };
        core.environment_requests += 1;
        let request = core.environment_requests;
        let dir = core.projects[at].project.primary.path.clone();
        core.environments.entry(project).or_default().capturing = Some(request);
        (self.effects).push(Effect::CaptureEnvironment {
            project,
            request,
            dir,
        });
    }

    /// Keeps the answer to the latest capture of `project`, noting it in `*Messages*`.
    pub(crate) fn environment_captured(
        &mut self,
        project: ProjectId,
        request: u64,
        result: Result<Vec<(String, String)>, String>,
    ) {
        let core = &mut *self.core;
        let latest = core.environments.get(&project).and_then(|e| e.capturing) == Some(request);
        if !latest || core.project_at(project).is_err() {
            return;
        }
        let name = core.project_name(project);
        let (line, vars) = match result {
            Ok(vars) => (
                format!(
                    "Captured {name}'s shell environment: {} variables",
                    vars.len()
                ),
                Some(vars),
            ),
            Err(err) => (
                format!("Couldn't capture {name}'s shell environment, using aiditor's own: {err}"),
                None,
            ),
        };
        core.log(&line);
        let environment = Environment {
            capturing: None,
            vars,
        };
        core.environments.insert(project, environment);
    }

    /// `project-capture-environment`: captures the current project's environment again.
    pub(crate) fn recapture_environment(&mut self) -> Result<(), String> {
        if !self.core.capture {
            return Err(
                "aiditor was started from a shell, so it uses the inherited environment".into(),
            );
        }
        let project = self.core.current_workspace().project;
        self.capture_environment(project);
        let name = self.core.project_name(project);
        self.core
            .echo(format!("Capturing {name}'s shell environment..."));
        Ok(())
    }

    /// `getenv`: echoes a variable's value in the current project's environment, the project
    /// file's `env` on top.
    pub(crate) fn getenv(&mut self, args: Args) -> Result<(), String> {
        let core = &mut *self.core;
        let name = &args.strings[0];
        let project = core.current_workspace().project;
        let overrides = &core.projects[core.project_at(project)?].project.env;
        let value = match core.environments.get(&project) {
            _ if overrides.contains_key(name) => overrides.get(name).cloned(),
            Some(Environment {
                vars: Some(vars), ..
            }) => (vars.iter().find(|(n, _)| n == name)).map(|(_, v)| v.clone()),
            Some(Environment {
                capturing: Some(_), ..
            }) => {
                let project = core.project_name(project);
                return Err(format!("Still capturing {project}'s shell environment"));
            }
            _ => std::env::var_os(name).map(|v| v.to_string_lossy().into_owned()),
        };
        core.echo(value.unwrap_or_else(|| format!("{name} not defined")));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::Msg;
    use crate::harness::Harness;

    /// The captures started since the last call.
    fn captures(h: &mut Harness) -> Vec<(ProjectId, u64, PathBuf)> {
        (std::mem::take(&mut h.effects).into_iter())
            .filter_map(|effect| match effect {
                Effect::CaptureEnvironment {
                    project,
                    request,
                    dir,
                } => Some((project, request, dir)),
                _ => None,
            })
            .collect()
    }

    fn reply(
        h: &mut Harness,
        (project, request): (ProjectId, u64),
        result: Result<&[(&str, &str)], &str>,
    ) {
        let result = result
            .map(|vars| {
                vars.iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect()
            })
            .map_err(str::to_string);
        h.send(Msg::EnvironmentCaptured {
            project,
            request,
            result,
        });
    }

    fn call(h: &mut Harness, name: &str, strings: &[&str]) -> String {
        let strings = strings.iter().map(|s| s.to_string()).collect();
        h.call(
            name,
            Args {
                strings,
                ..Args::default()
            },
        );
        h.core.message().unwrap_or_default().to_string()
    }

    fn messages(h: &Harness) -> String {
        let id = h.core.buffers.find("*Messages*").unwrap();
        h.core.buffers.get(id).unwrap().text().to_string()
    }

    fn inherited(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| format!("{name} not defined"))
    }

    #[test]
    fn launched_from_a_shell_captures_nothing() {
        let mut h = Harness::new(&[]);
        h.send(Msg::Tick);
        assert_eq!(captures(&mut h), []);
        assert_eq!(
            call(&mut h, "project-capture-environment", &[]),
            "aiditor was started from a shell, so it uses the inherited environment"
        );
        assert_eq!(captures(&mut h), []);
        assert_eq!(call(&mut h, "getenv", &["PATH"]), inherited("PATH"));
    }

    #[test]
    fn each_project_is_captured_once_and_kept() {
        let mut h = Harness::new(&[]);
        h.core.capture_environments();
        h.send(Msg::Tick);
        let [(home, request, dir)] = captures(&mut h).try_into().unwrap();
        assert_eq!(dir, h.core.projects()[0].primary.path);
        assert_eq!(
            call(&mut h, "getenv", &["PATH"]),
            "Still capturing home's shell environment"
        );
        reply(&mut h, (home, request), Ok(&[("PATH", "/a:/b")]));
        assert!(messages(&h).contains("Captured home's shell environment: 1 variables"));
        assert_eq!(call(&mut h, "getenv", &["PATH"]), "/a:/b");
        assert_eq!(call(&mut h, "getenv", &["NOPE"]), "NOPE not defined");
        h.new_workspace("second", "home");
        h.send(Msg::Tick);
        assert_eq!(captures(&mut h), [], "home is captured already");
        h.new_workspace("other", "other");
        h.send(Msg::Tick);
        let [(other, ..)] = captures(&mut h).try_into().unwrap();
        assert_ne!(other, home);
    }

    #[test]
    fn a_failed_capture_falls_back_with_a_note() {
        let mut h = Harness::new(&[]);
        h.core.capture_environments();
        h.send(Msg::Tick);
        let [(home, request, _)] = captures(&mut h).try_into().unwrap();
        reply(&mut h, (home, request), Err("/bin/zsh took over 5 s"));
        assert!(messages(&h).contains(
            "Couldn't capture home's shell environment, using aiditor's own: /bin/zsh took over 5 s"
        ));
        assert_eq!(call(&mut h, "getenv", &["PATH"]), inherited("PATH"));
        h.send(Msg::Tick);
        assert_eq!(captures(&mut h), [], "a fallback is kept too");
    }

    #[test]
    fn a_recapture_replaces_the_environment_and_earlier_answers_are_ignored() {
        let mut h = Harness::new(&[]);
        h.core.capture_environments();
        h.send(Msg::Tick);
        let [(home, first, _)] = captures(&mut h).try_into().unwrap();
        reply(&mut h, (home, first), Ok(&[("A", "1")]));
        assert_eq!(
            call(&mut h, "project-capture-environment", &[]),
            "Capturing home's shell environment..."
        );
        let [(_, second, _)] = captures(&mut h).try_into().unwrap();
        assert_eq!(
            call(&mut h, "getenv", &["A"]),
            "1",
            "the old environment while a re-capture runs"
        );
        call(&mut h, "project-capture-environment", &[]);
        let [(_, third, _)] = captures(&mut h).try_into().unwrap();
        reply(&mut h, (home, second), Ok(&[("A", "2")]));
        assert_eq!(
            call(&mut h, "getenv", &["A"]),
            "1",
            "a superseded capture's answer"
        );
        reply(&mut h, (home, third), Ok(&[("A", "3")]));
        reply(&mut h, (home, second), Ok(&[("A", "2")]));
        assert_eq!(call(&mut h, "getenv", &["A"]), "3");
    }

    fn set_env(h: &mut Harness, name: &str, value: &str) {
        let env = &mut h.core.projects[0].project.env;
        env.insert(name.into(), value.into());
    }

    /// The extra variables of the terminals spawned since the last call.
    fn spawned_env(h: &mut Harness) -> Vec<Vec<(String, String)>> {
        (h.terminal_ops.drain(..))
            .filter_map(|op| match op {
                crate::TerminalOp::Spawn { env, .. } => Some(env),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_project_files_env_reaches_its_terminals_only() {
        let mut h = Harness::new(&[]);
        set_env(&mut h, "MIX_ENV", "test");
        call(&mut h, "vterm", &[]);
        let env = [("MIX_ENV".to_string(), "test".to_string())];
        assert_eq!(spawned_env(&mut h), [env]);
        h.new_workspace("other", "other");
        call(&mut h, "vterm", &[]);
        assert_eq!(spawned_env(&mut h), [[]]);
    }

    #[test]
    fn getenv_puts_the_project_files_env_on_top() {
        let mut h = Harness::new(&[]);
        set_env(&mut h, "PATH", "/b");
        assert_eq!(
            call(&mut h, "getenv", &["PATH"]),
            "/b",
            "over the inherited"
        );
        h.core.capture_environments();
        h.send(Msg::Tick);
        let [(home, request, _)] = captures(&mut h).try_into().unwrap();
        assert_eq!(call(&mut h, "getenv", &["PATH"]), "/b", "while capturing");
        assert_eq!(
            call(&mut h, "getenv", &["HOME"]),
            "Still capturing home's shell environment"
        );
        reply(
            &mut h,
            (home, request),
            Ok(&[("PATH", "/a"), ("HOME", "/h")]),
        );
        assert_eq!(call(&mut h, "getenv", &["PATH"]), "/b", "over the captured");
        assert_eq!(call(&mut h, "getenv", &["HOME"]), "/h");
    }
}
