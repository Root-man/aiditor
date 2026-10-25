//! Request ownership for runtime configuration reads.

use std::path::PathBuf;

use crate::{Ctx, Effect, Msg, project::Loaded};

#[derive(Clone, Debug, PartialEq)]
pub struct Bundle {
    pub settings: Result<Option<String>, String>,
    pub keymap: Result<Option<String>, String>,
    pub projects_dir: Option<PathBuf>,
    pub projects: Vec<Result<Loaded, String>>,
}

#[derive(Default)]
pub(crate) struct Reload {
    latest: u64,
    active: Option<(u64, u64)>,
    pub(crate) project_revision: u64,
}

impl Ctx<'_> {
    pub(crate) fn reload_configuration(&mut self) {
        self.core.reload.latest += 1;
        if self.core.reload.active.is_none() {
            self.start_configuration_load();
        }
    }

    fn start_configuration_load(&mut self) {
        let reload = &mut self.core.reload;
        reload.active = Some((reload.latest, reload.project_revision));
        self.effects.push(Effect::LoadConfiguration {
            request: reload.latest,
        });
    }

    pub(crate) fn configuration_loaded(&mut self, request: u64, bundle: Bundle) {
        let reload = &mut self.core.reload;
        let Some((active, revision)) = reload.active else {
            return;
        };
        if request != active {
            return;
        }
        reload.active = None;
        if request != reload.latest || revision != reload.project_revision {
            if request == reload.latest {
                reload.latest += 1;
            }
            self.start_configuration_load();
            return;
        }
        self.core.message = None;
        self.dispatch(Msg::SettingsLoaded(bundle.settings));
        self.dispatch(Msg::KeymapLoaded(bundle.keymap));
        self.dispatch(Msg::ProjectsLoaded {
            dir: bundle.projects_dir,
            projects: bundle.projects,
        });
        if self.core.message.is_none() {
            self.core.echo("Configuration reloaded".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Harness;

    fn bundle(size: u32) -> Bundle {
        Bundle {
            settings: Ok(Some(format!("font_size = {size}"))),
            keymap: Ok(None),
            projects_dir: None,
            projects: Vec::new(),
        }
    }

    fn requests(h: &mut Harness) -> Vec<u64> {
        std::mem::take(&mut h.effects)
            .into_iter()
            .filter_map(|e| match e {
                Effect::LoadConfiguration { request } => Some(request),
                _ => None,
            })
            .collect()
    }

    fn log(h: &Harness) -> String {
        h.core
            .buffers
            .recent()
            .find(|(_, b)| b.name == "*Messages*")
            .map(|(_, b)| b.text().to_string())
            .unwrap_or_default()
    }

    #[test]
    fn reload_confirms_application_through_keys_and_m_x() {
        for keys in ["C-x C-r", "M-x reload-settings RET"] {
            let mut h = Harness::new(&[("settings.toml", "font_size = 18")]);
            h.keys(keys);
            assert_eq!(h.core.settings().font_size, 18.);
            assert_eq!(h.core.message(), Some("Configuration reloaded"));
            assert_eq!(log(&h), "Configuration reloaded\n");
            h.keys("x");
            assert_eq!((h.text(), h.core.message()), ("x".into(), None));
        }
    }

    #[test]
    fn reload_reports_read_parse_binding_and_project_errors() {
        let mut read = bundle(18);
        read.settings = Err("unavailable".into());
        let mut parse = bundle(18);
        parse.settings = Ok(Some("font_size = 0".into()));
        let mut binding = bundle(18);
        binding.keymap = Ok(Some("[global]\n'C-c a' = 'missing-command'".into()));
        let mut project = bundle(18);
        project.projects = vec![Err("invalid definition".into())];
        for (loaded, expected) in [
            (read, "settings:"),
            (parse, "settings:"),
            (binding, "keymap:"),
            (project, "project invalid definition"),
        ] {
            let mut h = Harness::new(&[]);
            h.collect_effects = true;
            h.keys("C-x C-r");
            let request = requests(&mut h)[0];
            h.send(Msg::ConfigurationLoaded {
                request,
                bundle: loaded,
            });
            let message = h.core.message().unwrap();
            assert!(message.starts_with(expected), "{message}");
            assert_eq!(log(&h), format!("{message}\n"));
        }
    }

    #[test]
    fn accepted_reload_replaces_an_unrelated_message_and_retries_project_changes() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        h.keys("C-x C-r M-z");
        let first = requests(&mut h)[0];
        h.core.reload.project_revision += 1;
        h.send(Msg::ConfigurationLoaded {
            request: first,
            bundle: bundle(18),
        });
        assert_eq!(h.core.message(), Some("M-z is undefined"));
        let latest = requests(&mut h)[0];
        h.send(Msg::ConfigurationLoaded {
            request: latest,
            bundle: bundle(22),
        });
        assert_eq!(h.core.settings().font_size, 22.);
        assert_eq!(h.core.message(), Some("Configuration reloaded"));
        assert_eq!(log(&h), "M-z is undefined\nConfiguration reloaded\n");
    }

    #[test]
    fn startup_components_keep_their_existing_message_behavior() {
        let mut h = Harness::new(&[]);
        h.send(Msg::SettingsLoaded(Ok(None)));
        h.send(Msg::KeymapLoaded(Ok(None)));
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        });
        assert_eq!((h.core.message(), log(&h)), (None, String::new()));
    }

    #[test]
    fn held_reload_allows_input_and_applies_components_with_independent_fallback() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        h.keys("C-x C-r");
        let request = requests(&mut h)[0];
        h.keys("typed");
        assert_eq!(h.text(), "typed");
        let mut loaded = bundle(18);
        loaded.keymap = Err("keymap unavailable".into());
        loaded.projects = vec![Err("project unavailable".into())];
        h.send(Msg::ConfigurationLoaded {
            request,
            bundle: loaded,
        });
        assert_eq!(h.core.settings().font_size, 18.);
        assert_eq!(
            h.core.message(),
            Some("keymap: keymap unavailable; project project unavailable")
        );
        h.keys("C-x C-r");
        let request = requests(&mut h)[0];
        let mut loaded = bundle(20);
        loaded.settings = Err("settings unavailable".into());
        loaded.keymap = Ok(Some("[global]\n'C-c a' = 'beginning-of-buffer'".into()));
        loaded.projects = vec![Ok(Loaded::ad_hoc("new", std::path::Path::new("/new")))];
        h.send(Msg::ConfigurationLoaded {
            request,
            bundle: loaded,
        });
        assert_eq!(h.core.settings().font_size, 18.);
        assert!(h.core.project_id("new").is_ok());
        h.keys("C-c a");
        assert_eq!(h.point(), 0);
    }

    #[test]
    fn repeat_requests_coalesce_and_obsolete_or_duplicate_bundles_do_not_apply() {
        let mut h = Harness::new(&[]);
        h.collect_effects = true;
        h.keys("C-x C-r");
        let first = requests(&mut h)[0];
        h.keys("C-x C-r C-x C-r");
        assert!(requests(&mut h).is_empty());
        let before = h.core.settings().font_size;
        h.send(Msg::ConfigurationLoaded {
            request: first,
            bundle: bundle(18),
        });
        assert_eq!(h.core.settings().font_size, before);
        assert_eq!(log(&h), "");
        let latest = requests(&mut h);
        assert_eq!(latest, [first + 2]);
        h.send(Msg::ConfigurationLoaded {
            request: latest[0],
            bundle: bundle(22),
        });
        h.send(Msg::ConfigurationLoaded {
            request: first,
            bundle: bundle(18),
        });
        assert_eq!(h.core.settings().font_size, 22.);
        h.send(Msg::ConfigurationLoaded {
            request: latest[0],
            bundle: bundle(22),
        });
        assert_eq!(log(&h), "Configuration reloaded\n");
        assert!(requests(&mut h).is_empty());
    }
}
