//! Project grep: the grep service's hits listed live in the minibuffer, as consult-ripgrep.

use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    /// The project's roots with their names, the primary first.
    roots: Vec<(PathBuf, String)>,
    /// The project's ignore patterns.
    ignore: Vec<String>,
    /// The listed hits with their labels, kept after the prompt closes for its handler.
    hits: Vec<(String, Hit)>,
}

impl ProjectGrep {
    fn hit(&self, label: &str) -> Option<&Hit> {
        self.hits
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, hit)| hit)
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
        let text = self.text();
        let grep = &mut self.grep;
        if text != grep.typed {
            grep.typed = text;
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
            if std::mem::take(&mut grep.running) {
                effects.push(Effect::Grep(GrepOp::Cancel));
            }
            grep.hits.clear();
            return self.set_items("project-grep", Vec::new(), true);
        }
        grep.searches += 1;
        (grep.running, grep.answered) = (true, false);
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
        }
        let labels = match hits {
            Ok(hits) => {
                let labels: Vec<String> = hits.iter().map(|hit| grep.label(hit)).collect();
                grep.hits.extend(labels.iter().cloned().zip(hits));
                labels
            }
            Err(err) => {
                // A regexp error spans lines, the last saying what is wrong.
                self.echo(err.lines().last().unwrap_or_default().to_string());
                Vec::new()
            }
        };
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
    pub(crate) fn project_grep(&mut self, args: Args) -> Result<(), String> {
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
        self.core.grep = ProjectGrep {
            searches: self.core.grep.searches,
            roots: roots.into_iter().zip(names).collect(),
            ignore,
            ..ProjectGrep::default()
        };
        let command = self.core.commands["project-grep"];
        self.read_string(command, args, format!("Grep in {}: ", loaded.name), None)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::harness::{Harness, line_start};
    use crate::project::Root;
    use crate::{Hit, Msg};

    const WAIT: Duration = Duration::from_millis(200);

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
        assert_eq!(
            listed(&mut h),
            ["app/src/a.rs:3:let needle = 1;", "lib/n:1:Needle"]
        );
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
        assert_eq!(listed(&mut h), ["app/a.rs:1:needles"]);
        h.send(Msg::Grepped {
            search,
            hits: Ok(vec![Hit { line: 1, ..hit }]),
            done: true,
        });
        assert_eq!(listed(&mut h), ["app/a.rs:1:needles", "app/a.rs:2:needles"]);
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
        assert_eq!((grown, selected), (rows, Some(2)), "the list stays put");

        h.keys("s").advance(WAIT);
        let search = h.core.grep.searches;
        let (count, _, selected) = send(&mut h, search, batch(0..9));
        assert_eq!(count, "    1/9 ", "the field never narrows");
        assert_eq!(selected, Some(0), "a new search starts at its first hit");

        h.keys("C-g C-x p g").advance(WAIT);
        assert_eq!(h.core.minibuffer().unwrap().count, "   0/0 ");
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
