//! Project grep: the grep service's hits listed live in the minibuffer, as consult-ripgrep.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};

use crate::command::{Args, Preview};
use crate::project_commands::root_names;
use crate::timer::TimerId;
use crate::{Core, Ctx, Effect, Effects, GrepOp, GrepSearch, Hit, Msg};

/// How long the text must stay unchanged before it is searched, consult's
/// `consult-async-input-debounce`.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// The fewest chars searched, consult's `consult-async-min-input`.
const MIN_INPUT: usize = 3;

/// The most hits a search lists.
const LIMIT: usize = 10_000;

pub(crate) const STATUS_COLS: usize = "[no matches]".len() + 2;

/// The open grep prompt's search and its hits.
#[derive(Default)]
pub(crate) struct ProjectGrep {
    /// How many searches were started; numbers each one, so late answers are dropped.
    searches: u64,
    /// The prompt's text when last polled, and when it is due to be searched.
    typed: String,
    deadline: Option<Instant>,
    /// Wakes the core at `deadline`.
    timer: Option<TimerId>,
    /// Whether search `searches` is running, and whether it has answered yet.
    running: bool,
    answered: bool,
    no_matches: bool,
    /// The searched roots with their finder names, in search order.
    roots: Vec<(PathBuf, String)>,
    choosing_root: bool,
    /// The project's ignore patterns.
    ignore: Vec<String>,
    /// The listed hits with their labels, kept after the prompt closes for its handler.
    hits: Vec<(String, Hit)>,
    pattern: Option<RegexMatcher>,
    pending_pattern: Option<RegexMatcher>,
}

impl crate::display::Search for ProjectGrep {
    fn matches(
        &self,
        text: &str,
        displayed: &[std::ops::Range<usize>],
    ) -> Vec<crate::display::SearchMatch> {
        let mut ranges = Vec::new();
        let end = displayed.last().map_or(0, |r| r.end);
        let mut first = true;
        if let Some(pattern) = &self.pattern {
            let _ = pattern.find_iter(text.as_bytes(), |m| {
                if !m.is_empty() {
                    crate::display::retain_match(m.start()..m.end(), first, displayed, &mut ranges);
                    first = false;
                }
                m.start() < end
            });
        }
        ranges
    }
}

impl ProjectGrep {
    pub(crate) fn choosing_root(&self) -> bool {
        self.choosing_root
    }

    pub(crate) fn status(&self) -> Option<&'static str> {
        if self.deadline.is_some() {
            None
        } else if self.running {
            Some("[searching]")
        } else if self.no_matches {
            Some("[no matches]")
        } else {
            None
        }
    }

    pub(crate) fn hit(&self, label: &str) -> Option<&Hit> {
        self.hits
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, hit)| hit)
    }

    pub(crate) fn file_label(&self, hit: &Hit) -> String {
        if self.roots.len() == 1 {
            hit.path.display().to_string()
        } else {
            let name = self.roots.iter().find(|(path, _)| *path == hit.root);
            format!("{}/{}", name.map_or("", |(_, n)| n), hit.path.display())
        }
    }

    pub(crate) fn line_number_width(&self) -> usize {
        self.hits
            .iter()
            .map(|(_, h)| (h.line + 1).to_string().len())
            .max()
            .unwrap_or(0)
            .max(3)
    }

    /// `<root name>/<path>:<1-based line>:<text>`.
    fn label(&self, hit: &Hit) -> String {
        let root = self.roots.iter().find(|(path, _)| *path == hit.root);
        let name = root.map_or("", |(_, name)| name.as_str());
        format!(
            "{name}/{}:{}:{}",
            hit.path.display(),
            hit.line + 1,
            hit.text
        )
    }
}

/// The selected hit's file at its match.
pub(crate) fn preview(core: &Core, label: &str) -> Option<Preview> {
    let hit = core.grep.hit(label)?;
    Some(Preview::FileAt(hit.root.join(&hit.path), hit.line, hit.col))
}

impl Core {
    /// Searches the grep prompt's text once it has stayed unchanged for `DEBOUNCE`, and cancels
    /// the search once the prompt is gone. Runs after every message.
    pub(crate) fn poll_grep(&mut self, now: Instant, effects: &mut Effects) {
        if self.prompts.command() != Some("project-grep") {
            if std::mem::take(&mut self.grep.running) {
                effects.push(Effect::Grep(GrepOp::Cancel));
            }
            return;
        }
        if self.grep.choosing_root {
            return;
        }
        let text = self.text();
        let grep = &mut self.grep;
        if text != grep.typed {
            grep.typed = text;
            grep.no_matches = false;
            grep.deadline = Some(now + DEBOUNCE);
            if let Some(id) = grep.timer.take() {
                self.timers.cancel(id);
            }
            grep.timer = Some(self.timers.start(now, DEBOUNCE, None, Msg::Tick));
            return;
        }
        if grep.deadline.is_none_or(|deadline| now < deadline) {
            return;
        }
        (grep.deadline, grep.timer) = (None, None);
        if text.chars().count() < MIN_INPUT {
            grep.no_matches = false;
            if std::mem::take(&mut grep.running) {
                effects.push(Effect::Grep(GrepOp::Cancel));
            }
            grep.hits.clear();
            grep.pattern = None;
            grep.pending_pattern = None;
            return self.set_items("project-grep", Vec::new(), true);
        }
        grep.searches += 1;
        (grep.running, grep.answered) = (true, false);
        grep.no_matches = false;
        grep.pending_pattern = RegexMatcherBuilder::new()
            .case_smart(true)
            .build(&text)
            .ok();
        effects.push(Effect::Grep(GrepOp::Search(GrepSearch {
            id: grep.searches,
            roots: grep.roots.iter().map(|(root, _)| root.clone()).collect(),
            ignore: grep.ignore.clone(),
            pattern: text,
            limit: LIMIT,
            workers: self.settings.grep_workers,
        })));
    }

    /// Takes the grep service's answer for the running search: its first answer replaces the
    /// listed hits, later ones add to them, and an invalid pattern empties the list.
    pub(crate) fn on_grepped(&mut self, search: u64, hits: Result<Vec<Hit>, String>, done: bool) {
        let grep = &mut self.grep;
        if search != grep.searches || !grep.running {
            return;
        }
        grep.running = !done;
        let replace = !std::mem::replace(&mut grep.answered, true);
        if replace {
            grep.hits.clear();
            grep.pattern = grep.pending_pattern.take();
        }
        match hits {
            Ok(hits) => {
                let labels: Vec<String> = hits.iter().map(|hit| grep.label(hit)).collect();
                grep.hits.extend(labels.into_iter().zip(hits));
                let mut files = HashMap::new();
                for (_, hit) in &grep.hits {
                    let next = files.len();
                    files
                        .entry((hit.root.clone(), hit.path.clone()))
                        .or_insert(next);
                }
                grep.hits
                    .sort_by_cached_key(|(_, hit)| files[&(hit.root.clone(), hit.path.clone())]);
                grep.no_matches = done && grep.hits.is_empty();
            }
            Err(err) => {
                // A regexp error spans lines, the last saying what is wrong.
                self.echo(err.lines().last().unwrap_or_default().to_string());
            }
        }
        let labels = self
            .grep
            .hits
            .iter()
            .map(|(label, _)| label.clone())
            .collect();
        self.set_items("project-grep", labels, replace);
    }

    pub(crate) fn goto_line_col(&mut self, line: usize, col: usize) {
        let buffer = self.current();
        // A line past the end, the file having changed since, is the end.
        let p = buffer.char_idx(line, col).unwrap_or(buffer.len_chars());
        if let Err(err) = self.windows.window.jump(&mut self.buffers, p) {
            tracing::debug!(%err, "grep jump outside the buffer");
        }
    }
}

impl Ctx<'_> {
    /// Asks for a regexp and lists its hits in every root of the current workspace's project as
    /// it is typed; given a listed hit, visits its file at the match.
    pub(crate) fn project_grep(&mut self, mut args: Args) -> Result<(), String> {
        let command = self.core.commands["project-grep"];
        if self.core.grep.choosing_root
            && let Some(name) = args.strings.first()
        {
            let root = self
                .core
                .grep
                .roots
                .iter()
                .find(|(_, n)| n == name)
                .cloned()
                .ok_or("No root selected")?;
            let prompt = format!("Grep in {}: ", root.1);
            self.core.grep.roots = vec![root];
            self.core.grep.choosing_root = false;
            args.strings.clear();
            return self.read_string(command, args, prompt, None);
        }
        if let Some(label) = args.strings.first() {
            let hit = self.core.grep.hit(label).ok_or("No match")?;
            let (path, line, col) = (hit.root.join(&hit.path), hit.line, hit.col);
            let from = self.core.windows.window.buffer();
            self.find_file(&path.to_string_lossy())?;
            // A visited file is switched to at once; another jumps once read.
            match self.core.buffers.visiting(&path) == Some(self.core.windows.window.buffer()) {
                true => {
                    // Switching to another buffer recorded the jump already.
                    if self.core.windows.window.buffer() == from {
                        self.core.push_jump();
                    }
                    self.core.goto_line_col(line, col)
                }
                false => {
                    if let Some(pending) = &mut self.core.pending_visit {
                        pending.destination = Some((line, col));
                    }
                }
            }
            return Ok(());
        }
        let project = self.core.current_workspace().project;
        let loaded = &self.core.projects[self.core.project_at(project)?].project;
        let roots: Vec<PathBuf> = loaded.all_roots().map(|root| root.path.clone()).collect();
        let ignore = loaded.ignore.clone();
        let names = root_names(roots.iter().map(PathBuf::as_path));
        let mut roots: Vec<_> = roots.into_iter().zip(names).collect();
        let prompt = format!("Grep in {}: ", loaded.name);
        if args.prefix.is_none()
            && let Some(file) = &self.core.current().file
            && let Some(at) = roots
                .iter()
                .enumerate()
                .filter(|(_, (root, _))| file.starts_with(root))
                .max_by_key(|(_, (root, _))| root.components().count())
                .map(|(at, _)| at)
        {
            let root = roots.remove(at);
            roots.insert(0, root);
        }
        self.core.grep = ProjectGrep {
            searches: self.core.grep.searches,
            roots,
            choosing_root: args.prefix.is_some(),
            ignore,
            ..ProjectGrep::default()
        };
        if self.core.grep.choosing_root {
            let names: Vec<_> = self
                .core
                .grep
                .roots
                .iter()
                .map(|(_, n)| n.clone())
                .collect();
            let choices: Vec<_> = names.iter().map(String::as_str).collect();
            return self.read_string(command, args, "Grep root: ".into(), Some(&choices));
        }
        self.read_string(command, args, prompt, None)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::harness::{Harness, line_start};
    use crate::project::Root;
    use crate::{Hit, Msg};

    const WAIT: Duration = Duration::from_millis(200);

    fn preview_spans(h: &mut Harness) -> Vec<(String, Option<u32>)> {
        h.frame(800., 600.).windows.remove(0).display.lines[0]
            .spans
            .iter()
            .map(|s| (s.text.clone(), s.face.bg))
            .collect()
    }

    #[test]
    fn preview_faces_use_grep_smart_case_and_clear_on_close() {
        use crate::settings::FaceName;
        for (query, expected) in [
            (
                "foo",
                vec![
                    ("été ", None),
                    ("Foo", Some(FaceName::MatchCurrent)),
                    (" ", None),
                    ("foo", Some(FaceName::Match)),
                ],
            ),
            (
                "Foo",
                vec![
                    ("été ", None),
                    ("Foo", Some(FaceName::MatchCurrent)),
                    (" foo", None),
                ],
            ),
            (
                r"\x66oo",
                vec![
                    ("été ", None),
                    ("Foo", Some(FaceName::MatchCurrent)),
                    (" ", None),
                    ("foo", Some(FaceName::Match)),
                ],
            ),
        ] {
            for close in ["RET", "C-g"] {
                let mut h = grepping(
                    &[("app/a.txt", "été Foo foo\n")],
                    &[("app", "a.txt", 0, 4, "été Foo foo")],
                );
                h.open("app/a.txt")
                    .keys("C-x p g")
                    .keys(query)
                    .advance(WAIT);
                let expected: Vec<_> = expected
                    .iter()
                    .map(|(s, face)| {
                        (
                            s.to_string(),
                            face.and_then(|name| h.core.settings().theme.face(name).bg),
                        )
                    })
                    .collect();
                assert_eq!(preview_spans(&mut h), expected, "{query}");
                h.keys(close);
                assert_eq!(preview_spans(&mut h), [("été Foo foo".into(), None)]);
            }
        }
    }

    #[test]
    fn preview_faces_keep_the_displayed_query_until_replacement_results() {
        use crate::settings::FaceName;
        let mut h = grepping(
            &[("app/a.txt", "needle other\n")],
            &[("app", "a.txt", 0, 0, "needle other")],
        );
        h.open("app/a.txt").keys("C-x p g");
        let first = search(&mut h);
        let hit = Hit {
            root: h.path("app"),
            path: "a.txt".into(),
            line: 0,
            col: 0,
            text: "needle other".into(),
        };
        h.send(Msg::Grepped {
            search: first.id,
            hits: Ok(vec![hit.clone()]),
            done: true,
        });
        let current = h.core.settings().theme.face(FaceName::MatchCurrent).bg;
        let old = vec![("needle".into(), current), (" other".into(), None)];
        assert_eq!(preview_spans(&mut h), old);
        h.keys("C-a C-k other");
        assert_eq!(preview_spans(&mut h), old);
        h.advance(WAIT);
        assert_eq!(preview_spans(&mut h), old);
        let id = h
            .effects
            .iter()
            .find_map(|e| match e {
                crate::Effect::Grep(crate::GrepOp::Search(op)) => Some(op.id),
                _ => None,
            })
            .unwrap();
        h.send(Msg::Grepped {
            search: id,
            hits: Ok(vec![Hit { col: 7, ..hit }]),
            done: true,
        });
        assert_eq!(
            preview_spans(&mut h),
            [("needle ".into(), None), ("other".into(), current)]
        );
    }

    #[test]
    fn a_pending_file_preview_keeps_ordinary_faces_until_loaded() {
        let mut h = grepping(
            &[("app/a.txt", "needle a"), ("app/b.txt", "needle b")],
            &[
                ("app", "a.txt", 0, 0, "needle a"),
                ("app", "b.txt", 0, 0, "needle b"),
            ],
        );
        h.open("app/a.txt").keys("C-x p g needle").advance(WAIT);
        h.collect_effects = true;
        h.keys("C-n");
        assert_eq!(preview_spans(&mut h), [("needle a".into(), None)]);
        let request = h
            .effects
            .iter()
            .find_map(|e| match e {
                crate::Effect::File(crate::FileOp::Preview(r)) => Some(r.clone()),
                _ => None,
            })
            .unwrap();
        h.send(Msg::Previewed {
            request,
            result: Ok(Some(crate::buffer::prepare_preview(b"needle b").unwrap())),
        });
        let current = h
            .core
            .settings()
            .theme
            .face(crate::settings::FaceName::MatchCurrent)
            .bg;
        assert_eq!(
            preview_spans(&mut h),
            [("needle".into(), current), (" b".into(), None)]
        );
    }

    /// The current workspace's project at `app`, with `lib` as a second root.
    fn grepping(files: &[(&str, &str)], hits: &[(&str, &str, usize, usize, &str)]) -> Harness {
        let mut h = Harness::new(files);
        h.grep_reply(hits);
        let (app, lib) = (h.path("app"), h.path("lib"));
        let project = &mut h.core.projects[0].project;
        project.primary.path = app;
        project.roots = vec![Root {
            path: lib,
            role: String::new(),
        }];
        h
    }

    fn listed(h: &mut Harness) -> Vec<String> {
        h.core
            .minibuffer()
            .expect("the minibuffer is open")
            .candidates
    }

    /// The window the prompt was opened from: its file and point.
    fn origin(h: &Harness) -> (Option<std::path::PathBuf>, usize) {
        let window = &h.core.windows.others[&h.core.windows.tree.leaves()[0]];
        let buffer = h.core.buffers.get(window.buffer()).unwrap();
        (buffer.file.clone(), window.point(&h.core.buffers))
    }

    const HITS: &[(&str, &str, usize, usize, &str)] = &[
        ("app", "a.rs", 0, 0, "needle"),
        ("app", "b.rs", 1, 2, "  needle"),
    ];

    const FILES: &[(&str, &str)] = &[("app/a.rs", "needle\n"), ("app/b.rs", "x\n  needle\n")];

    fn search(h: &mut Harness) -> crate::GrepSearch {
        h.collect_effects = true;
        h.keys("needle").advance(WAIT);
        std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|e| match e {
                crate::Effect::Grep(crate::GrepOp::Search(op)) => Some(op),
                _ => None,
            })
            .expect("a search starts")
    }

    #[test]
    fn current_root_precedes_remaining_roots_in_project_order() {
        for (file, expected) in [
            (None, vec!["app", "lib", "docs"]),
            (Some("elsewhere/a"), vec!["app", "lib", "docs"]),
            (Some("app/a"), vec!["app", "lib", "docs"]),
            (Some("lib/a"), vec!["lib", "app", "docs"]),
            (Some("docs/a"), vec!["docs", "app", "lib"]),
        ] {
            let mut h = grepping(&[], &[]);
            let docs = h.path("docs");
            h.core.projects[0].project.roots.push(Root {
                path: docs,
                role: String::new(),
            });
            h.core.current_mut().file = file.map(|p| h.path(p));
            h.keys("C-x p g");
            let op = search(&mut h);
            assert_eq!(
                op.roots,
                expected.iter().map(|p| h.path(p)).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn most_specific_root_is_first_and_duplicate_finder_names_stay_stable() {
        let mut h = grepping(&[], &[]);
        let nested = h.path("app/lib");
        h.core.projects[0].project.roots.push(Root {
            path: nested.clone(),
            role: String::new(),
        });
        h.core.current_mut().file = Some(nested.join("a"));
        h.keys("C-x p g");
        assert_eq!(
            search(&mut h).roots,
            [nested.clone(), h.path("app"), h.path("lib")]
        );
        let hit = Hit {
            root: nested,
            path: "a".into(),
            line: 0,
            col: 0,
            text: "needle".into(),
        };
        assert_eq!(h.core.grep.label(&hit), "lib<2>/a:1:needle");
    }

    #[test]
    fn prefix_completes_a_single_root_then_searches_previews_and_jumps() {
        let mut h = grepping(&[("lib/a.rs", "top\n  needle\n")], &[]);
        h.keys("C-u C-x p g");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep root: ");
        assert_eq!(listed(&mut h), ["app", "lib"]);
        h.keys("lb").advance(WAIT);
        assert_eq!(listed(&mut h), ["lib"]);
        assert_eq!(h.greps.len(), 0);
        h.keys("RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in lib: ");
        let op = search(&mut h);
        assert_eq!(op.roots, [h.path("lib")]);
        h.collect_effects = false;
        h.send(Msg::Grepped {
            search: op.id,
            hits: Ok(vec![Hit {
                root: h.path("lib"),
                path: "a.rs".into(),
                line: 1,
                col: 2,
                text: "  needle".into(),
            }]),
            done: true,
        });
        assert_eq!(listed(&mut h), ["  2:   needle"]);
        assert_eq!(h.core.minibuffer().unwrap().groups, [(0, "a.rs".into())]);
        h.keys("RET");
        assert_eq!(h.core.current().file, Some(h.path("lib/a.rs")));
        assert_eq!(h.point(), line_start(&h, 1) + 2);
        h.keys("C-u C-x p g lib RET M-p");
        assert_eq!(h.text(), "needle");
    }

    #[test]
    fn root_selection_keeps_duplicate_names_ignores_and_cancellation() {
        let mut h = grepping(&[], &[]);
        let other = h.path("other/lib");
        let project = &mut h.core.projects[0].project;
        project.roots.push(Root {
            path: other.clone(),
            role: String::new(),
        });
        project.ignore = vec!["target/".into()];
        let origin = h.core.windows.window.buffer();
        h.keys("C-u C-x p g lib").advance(WAIT).keys("C-g");
        assert_eq!(h.core.windows.window.buffer(), origin);
        assert_eq!(h.greps.len(), 0);
        h.keys("C-u C-x p g lib<2> RET");
        let op = search(&mut h);
        assert_eq!(op.roots, [other]);
        assert_eq!(op.ignore, ["target/"]);
        h.keys("C-g C-x p g");
        assert_eq!(
            search(&mut h).roots,
            [h.path("app"), h.path("lib"), h.path("other/lib")]
        );
    }

    #[test]
    fn deferred_grep_destination_belongs_to_its_visit_and_uses_character_columns() {
        let mut h = grepping(
            &[("app/a.rs", "top\né🙂 needle\n")],
            &[("app", "a.rs", 1, 3, "é🙂 needle")],
        );
        h.keys("C-x p g needle").advance(WAIT);
        h.collect_effects = true;
        h.keys("RET");
        let (intent, path) = std::mem::take(&mut h.effects)
            .into_iter()
            .find_map(|effect| match effect {
                crate::Effect::File(crate::FileOp::Open { intent, path }) => Some((intent, path)),
                _ => None,
            })
            .unwrap();
        let text = "top\né🙂 needle\n";
        h.send(Msg::Opened {
            intent,
            path,
            result: Ok(Some((
                text.as_bytes().to_vec(),
                crate::buffer::hash(text.as_bytes()),
            ))),
        });
        assert_eq!((h.text(), h.point()), (text.into(), 7));
    }

    #[test]
    fn search_move_jump() {
        let mut h = grepping(FILES, HITS);
        h.keys("C-x p g needle").advance(WAIT).keys("C-n RET");
        assert_eq!(h.core.current().file, Some(h.path("app/b.rs")));
        assert_eq!(h.point(), line_start(&h, 1) + 2);
        assert!(h.core.prompts.minibuffer.is_none());
        // A visited file is switched to at once.
        h.grep_reply(HITS);
        h.grep_reply(HITS);
        h.keys("C-x p g needle").advance(WAIT).keys("C-n RET");
        assert_eq!(h.point(), line_start(&h, 1) + 2);
        h.keys("C-x p g needle").advance(WAIT).keys("RET");
        assert_eq!(
            (h.core.current().file.clone(), h.point()),
            (Some(h.path("app/a.rs")), 0)
        );
    }

    #[test]
    fn opening_and_history() {
        let mut h = grepping(FILES, HITS);
        h.keys("C-x p g");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        assert_eq!(listed(&mut h), Vec::<String>::new());
        h.keys("RET");
        assert_eq!(h.core.message(), Some("Match required"));
        h.keys("needle").advance(WAIT).keys("RET C-x p g M-p");
        assert_eq!(h.text(), "needle");
    }

    #[test]
    fn prompt_tracks_running_batches_and_successful_completion() {
        let mut h = grepping(FILES, &[]);
        h.hits.clear();
        h.keys("C-x p g needle");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        h.advance(WAIT);
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(
            (mb.prompt.as_str(), mb.status, mb.count.as_str()),
            ("Grep in home: ", Some("[searching]"), "   0/0 ")
        );
        let search = h.core.grep.searches;
        for line in 0..2 {
            h.send(Msg::Grepped {
                search,
                hits: Ok(vec![Hit {
                    root: h.path("app"),
                    path: "a.rs".into(),
                    line,
                    col: 0,
                    text: "needle".into(),
                }]),
                done: false,
            });
            let mb = h.core.minibuffer().unwrap();
            assert_eq!(mb.prompt, "Grep in home: ");
            assert_eq!(mb.status, Some("[searching]"));
            assert_eq!(mb.count, format!("   1/{} ", line + 1));
        }
        h.send(Msg::Grepped {
            search,
            hits: Ok(Vec::new()),
            done: true,
        });
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(
            (mb.prompt.as_str(), mb.count.as_str()),
            ("Grep in home: ", "   1/2 ")
        );
    }

    #[test]
    fn status_transitions_keep_short_and_scrolled_input_in_place() {
        for query in ["needle".into(), "needle".repeat(40)] {
            for outcome in 0..3 {
                let mut h = grepping(&[], &[]);
                h.hits.clear();
                h.keys("C-x p g").keys(&query);
                let before = h.frame(400., 600.).minibuffer.unwrap();
                assert_eq!(before.status, None);
                h.advance(WAIT);
                let running = h.frame(400., 600.).minibuffer.unwrap();
                assert_eq!(running.status, Some("[searching]"));
                assert_eq!(running.prompt, before.prompt);
                assert_eq!(running.display, before.display);
                let hits = match outcome {
                    0 => Ok(Vec::new()),
                    1 => Ok(vec![Hit {
                        root: h.path("app"),
                        path: "a.rs".into(),
                        line: 0,
                        col: 0,
                        text: query.clone(),
                    }]),
                    _ => Err("error: invalid pattern".into()),
                };
                h.send(Msg::Grepped {
                    search: h.core.grep.searches,
                    hits,
                    done: true,
                });
                let after = h.frame(400., 600.).minibuffer.unwrap();
                assert_eq!(after.status, (outcome == 0).then_some("[no matches]"));
                assert_eq!(after.prompt, before.prompt);
                assert_eq!(after.display, before.display);
                assert_eq!(h.text(), query);
            }
        }
    }

    #[test]
    fn empty_completion_clears_on_edit_and_keeps_history() {
        let mut h = grepping(FILES, &[]);
        h.keys("C-x p g needle").advance(WAIT);
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(
            (mb.prompt.as_str(), mb.status, mb.count.as_str()),
            ("Grep in home: ", Some("[no matches]"), "   0/0 ")
        );
        h.keys("C-g C-x p g M-p");
        assert_eq!(h.text(), "needle");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        h.advance(WAIT);
        assert_eq!(h.core.minibuffer().unwrap().status, Some("[searching]"));
        let search = h.core.grep.searches;
        h.keys("DEL DEL DEL DEL");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        h.send(Msg::Grepped {
            search,
            hits: Ok(Vec::new()),
            done: true,
        });
        h.advance(WAIT);
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(
            (mb.prompt.as_str(), mb.count.as_str()),
            ("Grep in home: ", "   0/0 ")
        );
    }

    #[test]
    fn obsolete_replies_leave_the_current_prompt_status_alone() {
        let mut h = grepping(FILES, &[]);
        h.hits.clear();
        h.keys("C-x p g needle").advance(WAIT);
        let old = h.core.grep.searches;
        h.keys("s");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        h.advance(WAIT);
        for hits in [Ok(Vec::new()), Err("obsolete error".into())] {
            h.send(Msg::Grepped {
                search: old,
                hits,
                done: true,
            });
            assert_eq!(h.core.minibuffer().unwrap().status, Some("[searching]"));
        }
        let cancelled = h.core.grep.searches;
        h.keys("C-g C-x p g");
        h.send(Msg::Grepped {
            search: cancelled,
            hits: Ok(Vec::new()),
            done: true,
        });
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        h.keys("ab(").advance(WAIT);
        h.send(Msg::Grepped {
            search: h.core.grep.searches,
            hits: Err("error: unclosed group".into()),
            done: true,
        });
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
        assert_eq!(h.core.message(), Some("error: unclosed group"));
        h.keys("C-g C-x p g needle")
            .advance(WAIT)
            .keys("C-a C-k")
            .advance(WAIT);
        assert_eq!(h.greps.last(), Some(&None));
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Grep in home: ");
    }

    #[test]
    fn a_search_carries_the_worker_count_set_when_it_starts() {
        let mut h = grepping(FILES, HITS);
        h.collect_effects = true;
        let workers = |h: &mut Harness| {
            h.keys("C-x p g needle").advance(WAIT);
            let effects = std::mem::take(&mut h.effects);
            h.keys("C-g");
            effects.into_iter().find_map(|effect| match effect {
                crate::Effect::Grep(crate::GrepOp::Search(op)) => Some(op.workers.get()),
                _ => None,
            })
        };
        assert_eq!(workers(&mut h), Some(1));
        h.send(Msg::SettingsLoaded(Ok(Some("grep_workers = 4".into()))));
        assert_eq!(workers(&mut h), Some(4));
    }

    #[test]
    fn searches_wait_for_a_pause_and_three_chars() {
        let mut h = grepping(FILES, HITS);
        h.keys("C-x p g nee").advance(WAIT / 2);
        assert_eq!(h.greps, []);
        h.keys("d").advance(WAIT / 2);
        assert_eq!(h.greps, [], "each change restarts the wait");
        h.advance(WAIT / 2);
        assert_eq!(h.greps, [Some("need".into())]);
        assert_eq!(listed(&mut h).len(), 2);
        h.keys("DEL DEL").advance(WAIT);
        assert_eq!(listed(&mut h), Vec::<String>::new());
        assert_eq!(h.greps.len(), 1, "too short searches nothing");
    }

    #[test]
    fn hits_are_labelled_by_root_and_line() {
        let files = [
            ("app/src/a.rs", "\n\nlet needle = 1;\n"),
            ("lib/n", "Needle\n"),
        ];
        let mut h = grepping(
            &files,
            &[
                ("app", "src/a.rs", 2, 4, "let needle = 1;"),
                ("lib", "n", 0, 0, "Needle"),
            ],
        );
        h.keys("C-x p g needle").advance(WAIT);
        assert_eq!(listed(&mut h), ["  3: let needle = 1;", "  1: Needle"]);
        assert_eq!(
            h.core.minibuffer().unwrap().groups,
            [(0, "app/src/a.rs".into()), (1, "lib/n".into())]
        );
    }

    #[test]
    fn interleaved_hits_join_existing_groups_and_keep_the_selected_hit() {
        let mut h = grepping(
            &[
                ("app/same:名.rs", "needle a\nneedle later"),
                ("lib/same:名.rs", "needle b"),
            ],
            &[],
        );
        h.hits.clear();
        h.keys("C-x p g needle").advance(WAIT);
        let search = h.core.grep.searches;
        let a = Hit {
            root: h.path("app"),
            path: "same:名.rs".into(),
            line: 0,
            col: 0,
            text: "needle a".into(),
        };
        let b = Hit {
            root: h.path("lib"),
            text: "needle b".into(),
            ..a.clone()
        };
        h.send(Msg::Grepped {
            search,
            hits: Ok(vec![a.clone(), b]),
            done: false,
        });
        h.keys("C-n");
        assert_eq!(origin(&h), (Some(h.path("lib/same:名.rs")), 0));
        h.send(Msg::Grepped {
            search,
            hits: Ok(vec![Hit {
                line: 1,
                text: "needle later".into(),
                ..a
            }]),
            done: true,
        });
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(
            mb.candidates,
            ["  1: needle a", "  2: needle later", "  1: needle b"]
        );
        assert_eq!(
            mb.groups,
            [(0, "app/same:名.rs".into()), (2, "lib/same:名.rs".into())]
        );
        assert_eq!(
            (mb.count.as_str(), mb.selected, mb.rows),
            ("   3/3 ", Some(2), 5)
        );
        assert_eq!(origin(&h), (Some(h.path("lib/same:名.rs")), 0));
        h.keys("C-p");
        assert_eq!(h.core.minibuffer().unwrap().count, "   2/3 ");
        assert_eq!(origin(&h), (Some(h.path("app/same:名.rs")), 9));
        h.keys("C-n RET");
        assert_eq!(h.core.current().file, Some(h.path("lib/same:名.rs")));
    }

    #[test]
    fn grouped_viewport_counts_headers_keeps_selection_and_grows_only() {
        let mut h = grepping(&[], &[]);
        h.hits.clear();
        h.keys("C-x p g needle").advance(WAIT);
        let search = h.core.grep.searches;
        let hits = (0..12)
            .map(|n| Hit {
                root: h.path(if n < 6 { "app" } else { "lib" }),
                path: format!("{n}.rs").into(),
                line: n,
                col: 0,
                text: format!("needle {n}"),
            })
            .collect();
        h.send(Msg::Grepped {
            search,
            hits: Ok(hits),
            done: false,
        });
        for n in 0..12 {
            let mb = h.core.minibuffer().unwrap();
            let selected = mb.selected.unwrap();
            assert_eq!(mb.candidates[selected], format!("{:>3}: needle {n}", n + 1));
            assert_eq!(mb.candidates.len() + mb.groups.len(), 10);
            assert_eq!(mb.groups.len(), 5);
            assert_eq!(mb.groups[0].0, 0);
            assert_eq!(mb.rows, 10);
            assert_eq!(mb.count.trim(), format!("{}/12", n + 1));
            h.keys("C-n");
        }
        h.keys("C-u 11 C-p");
        assert_eq!(h.core.minibuffer().unwrap().selected, Some(0));
        h.keys("s").advance(WAIT);
        h.send(Msg::Grepped {
            search: h.core.grep.searches,
            hits: Ok(vec![Hit {
                root: h.path("lib"),
                path: "only.rs".into(),
                line: 0,
                col: 0,
                text: "needles".into(),
            }]),
            done: true,
        });
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.groups, [(0, "lib/only.rs".into())]);
        assert_eq!(mb.candidates, ["  1: needles"]);
        assert_eq!(
            (mb.rows, mb.selected, mb.count.trim()),
            (10, Some(0), "1/1")
        );
        h.keys("C-g C-x p g");
        assert_eq!(h.core.minibuffer().unwrap().rows, 0);
    }

    #[test]
    fn scrolling_within_a_file_repeats_its_header() {
        let mut h = grepping(&[], &[]);
        h.hits.clear();
        h.keys("C-x p g needle").advance(WAIT);
        h.send(Msg::Grepped {
            search: h.core.grep.searches,
            hits: Ok((0..20)
                .map(|line| Hit {
                    root: h.path("app"),
                    path: "a.rs".into(),
                    line,
                    col: 0,
                    text: "needle".into(),
                })
                .collect()),
            done: true,
        });
        h.keys("C-u 19 C-n");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.groups, [(0, "app/a.rs".into())]);
        assert_eq!(mb.candidates.len(), 9);
        assert_eq!(mb.candidates[0], " 12: needle");
        assert_eq!(mb.candidates[mb.selected.unwrap()], " 20: needle");
        assert_eq!(mb.count.trim(), "20/20");
    }

    #[test]
    fn old_hits_stay_until_the_new_search_answers() {
        let mut h = grepping(FILES, HITS);
        h.keys("C-x p g needle").advance(WAIT);
        h.hits.clear();
        h.keys("s").advance(WAIT);
        assert_eq!(h.greps.last(), Some(&Some("needles".into())));
        assert_eq!(listed(&mut h).len(), 2);
        let hit = Hit {
            root: h.path("app"),
            path: "a.rs".into(),
            line: 0,
            col: 0,
            text: "needles".into(),
        };
        let search = h.core.grep.searches;
        // An answer from the superseded search is dropped.
        h.send(Msg::Grepped {
            search: search - 1,
            hits: Ok(vec![hit.clone()]),
            done: true,
        });
        assert_eq!(listed(&mut h).len(), 2);
        h.send(Msg::Grepped {
            search,
            hits: Ok(vec![hit.clone()]),
            done: false,
        });
        assert_eq!(listed(&mut h), ["  1: needles"]);
        h.send(Msg::Grepped {
            search,
            hits: Ok(vec![Hit { line: 1, ..hit }]),
            done: true,
        });
        assert_eq!(listed(&mut h), ["  1: needles", "  2: needles"]);
    }

    #[test]
    fn the_prompt_and_the_list_hold_while_hits_stream_in() {
        let mut h = grepping(FILES, HITS);
        h.keys("C-x p g needle").advance(WAIT);
        h.hits.clear();
        h.keys("s").advance(WAIT);
        let hit = Hit {
            root: h.path("app"),
            path: "a.rs".into(),
            line: 0,
            col: 0,
            text: "needles".into(),
        };
        let batch = |lines: std::ops::Range<usize>| {
            let hit = hit.clone();
            lines
                .map(|line| Hit {
                    line,
                    ..hit.clone()
                })
                .collect()
        };
        let search = h.core.grep.searches;
        let send = |h: &mut Harness, search, hits: Vec<Hit>| {
            h.send(Msg::Grepped {
                search,
                hits: Ok(hits),
                done: false,
            });
            let mb = h.core.minibuffer().unwrap();
            (mb.count, mb.candidates, mb.selected)
        };
        let (count, ..) = send(&mut h, search, batch(0..9));
        assert_eq!(count, "   1/9 ");
        h.keys("C-u 2 C-n");
        let (count, rows, selected) = send(&mut h, search, batch(9..100));
        assert_eq!(count, " 3/100 ", "as wide as with 9 hits");
        assert_eq!(selected, Some(2));
        let (count, grown, selected) = send(&mut h, search, batch(100..10_000));
        assert_eq!(count, "3/10000 ", "a wider count widens the field");
        assert_eq!(
            grown.iter().map(|s| s.trim_start()).collect::<Vec<_>>(),
            rows.iter().map(|s| s.trim_start()).collect::<Vec<_>>(),
            "the same hits stay visible"
        );
        assert_eq!(selected, Some(2));

        h.keys("s").advance(WAIT);
        let search = h.core.grep.searches;
        let (count, _, selected) = send(&mut h, search, batch(0..9));
        assert_eq!(count, "    1/9 ", "the field never narrows");
        assert_eq!(selected, Some(0), "a new search starts at its first hit");

        h.keys("C-g C-x p g").advance(WAIT);
        assert_eq!(h.core.minibuffer().unwrap().count, "   0/0 ");
    }

    #[test]
    fn completion_keeps_arrival_order_and_the_selected_preview() {
        let mut h = grepping(
            &[
                ("app/a.rs", "needle\n"),
                ("app/c.rs", "needle\n"),
                ("lib/b.rs", "needle\n"),
            ],
            &[],
        );
        h.hits.clear();
        h.keys("C-x p g needle").advance(WAIT);
        let search = h.core.grep.searches;
        for (root, path) in [("lib", "b.rs"), ("app", "a.rs")] {
            h.send(Msg::Grepped {
                search,
                hits: Ok(vec![Hit {
                    root: h.path(root),
                    path: path.into(),
                    line: 0,
                    col: 0,
                    text: "needle".into(),
                }]),
                done: false,
            });
        }
        assert_eq!(h.core.minibuffer().unwrap().count, "   1/2 ");
        assert_eq!(h.core.minibuffer().unwrap().status, Some("[searching]"));
        h.keys("C-n");
        let selected = origin(&h);
        assert_eq!(selected, (Some(h.path("app/a.rs")), 0));
        let rows = listed(&mut h);
        h.send(Msg::Grepped {
            search,
            hits: Ok(vec![Hit {
                root: h.path("app"),
                path: "c.rs".into(),
                line: 0,
                col: 0,
                text: "needle".into(),
            }]),
            done: true,
        });
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.count, "   2/3 ");
        assert_eq!(mb.prompt, "Grep in home: ");
        assert_eq!(mb.selected, Some(1));
        assert_eq!(&mb.candidates[..2], rows);
        assert_eq!(mb.candidates[2], "  1: needle");
        assert_eq!(origin(&h), selected);
    }

    #[test]
    fn an_invalid_pattern_shows_its_error() {
        let mut h = grepping(FILES, HITS);
        h.hits
            .push_back(Err("regex parse error\nerror: unclosed group".into()));
        h.keys("C-x p g needle")
            .advance(WAIT)
            .keys("DEL DEL DEL DEL (")
            .advance(WAIT);
        assert_eq!(h.core.message(), Some("error: unclosed group"));
        assert_eq!(listed(&mut h), Vec::<String>::new());
    }

    #[test]
    fn leaving_cancels_a_running_search() {
        let mut h = grepping(FILES, HITS);
        h.hits.clear();
        h.keys("C-x p g needle").advance(WAIT).keys("C-g");
        assert_eq!(h.greps, [Some("needle".into()), None]);
    }

    #[test]
    fn a_far_hit_is_centered_and_c_g_restores_the_window() {
        let long: String = (0..300)
            .map(|i| match i {
                150 => "  needle\n".to_string(),
                i => format!("l{i}\n"),
            })
            .collect();
        let mut h = grepping(
            &[("app/long.rs", &long), ("start.txt", "one\ntwo\n")],
            &[("app", "long.rs", 150, 2, "  needle")],
        );
        h.open("start.txt");
        h.display(20);
        h.keys("C-n C-f C-x p g needle").advance(WAIT);
        let at: usize = long.lines().take(150).map(|l| l.len() + 1).sum();
        assert_eq!(origin(&h), (Some(h.path("app/long.rs")), at + 2));
        let id = h.core.windows.tree.leaves()[0];
        let window = h.core.windows.others.get_mut(&id).unwrap();
        assert_eq!(
            window.scroll(&h.core.buffers, 20),
            140,
            "out of view centers"
        );
        h.keys("C-g");
        assert_eq!(h.core.current().file, Some(h.path("start.txt")));
        assert_eq!(h.point(), 5);
        assert_eq!(h.core.windows.window.scroll(&h.core.buffers, 20), 0);
    }

    #[test]
    fn the_selected_hit_is_previewed() {
        let mut h = grepping(FILES, HITS);
        h.keys("C-x p g needle").advance(WAIT);
        assert_eq!(origin(&h), (Some(h.path("app/a.rs")), 0));
        h.keys("C-n");
        assert_eq!(origin(&h), (Some(h.path("app/b.rs")), 4));
        assert!(
            h.core.prompts.minibuffer.is_some(),
            "the minibuffer stays selected"
        );
        h.keys("C-g");
        assert_eq!((h.core.current().file.clone(), h.point()), (None, 0));
    }

    #[test]
    fn same_file_hits_reuse_prepared_text_and_follow_the_selected_position() {
        let mut h = grepping(
            &[("app/a.rs", "needle\nété needle\n")],
            &[
                ("app", "a.rs", 0, 0, "needle"),
                ("app", "a.rs", 1, 4, "été needle"),
            ],
        );
        h.keys("C-x p g needle").advance(WAIT);
        let path = h.path("app/a.rs");
        let first = h.core.buffers.visiting(&path).unwrap();
        assert_eq!(origin(&h), (Some(path.clone()), 0));
        h.keys("C-n");
        assert_eq!(h.core.buffers.visiting(&path), Some(first));
        assert_eq!(origin(&h), (Some(path), 11));
        h.keys("C-g");
        assert_eq!((h.core.current().file.clone(), h.point()), (None, 0));
    }
}
