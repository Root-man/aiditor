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
        self.dispatch(Msg::SettingsLoaded(bundle.settings));
        self.dispatch(Msg::KeymapLoaded(bundle.keymap));
        self.dispatch(Msg::ProjectsLoaded {
            dir: bundle.projects_dir,
            projects: bundle.projects,
        });
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
        assert_eq!(requests(&mut h), []);
        let before = h.core.settings().font_size;
        h.send(Msg::ConfigurationLoaded {
            request: first,
            bundle: bundle(18),
        });
        assert_eq!(h.core.settings().font_size, before);
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
        assert_eq!(requests(&mut h), []);
    }
}
