//! Swiper: the buffer's matching lines listed in the minibuffer, as ivy's `swiper`.

use std::borrow::Cow;
use std::ops::Range;

use regex::{Regex, RegexBuilder};
use ropey::Rope;

use crate::command::{Args, Preview};
use crate::{Core, Ctx};

/// How many lines swiper lists: a final newline adds none.
fn lines(text: &Rope) -> usize {
    let n = text.len_lines();
    n - usize::from(text.line(n - 1).len_chars() == 0)
}

pub(crate) struct Search {
    snapshot: String,
    lines: Vec<Range<usize>>,
    positive: Option<Regex>,
    negative: Vec<Regex>,
}

impl crate::display::Search for Search {
    fn matches(&self, text: &str) -> Vec<Range<usize>> {
        if self.negative.iter().any(|r| r.is_match(text)) {
            return Vec::new();
        }
        self.positive
            .iter()
            .flat_map(|r| r.find_iter(text))
            .filter(|m| !m.is_empty())
            .map(|m| m.range())
            .collect()
    }
}

impl Search {
    pub(crate) fn new(text: &Rope) -> Self {
        let snapshot = text.to_string();
        let mut lines = Vec::new();
        let mut start = 0;
        for (end, _) in snapshot.match_indices('\n') {
            lines.push(start..end);
            start = end + 1;
        }
        if start < snapshot.len() {
            lines.push(start..snapshot.len());
        }
        Self {
            snapshot,
            lines,
            positive: None,
            negative: Vec::new(),
        }
    }

    pub(crate) fn filter(&mut self, text: &str) -> Vec<usize> {
        (self.positive, self.negative) = regexes(text);
        // shortcut: each query scans all lines on the core thread; move scans off-thread if measured input latency warrants it.
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                let content = &self.snapshot[line.clone()];
                (self.positive.as_ref().is_none_or(|r| r.is_match(content))
                    && !self.negative.iter().any(|r| r.is_match(content)))
                .then_some(i)
            })
            .collect()
    }

    pub(crate) fn candidate(&self, line: usize) -> String {
        let width = self.lines.len().to_string().len();
        let content = &self.snapshot[self.lines[line].clone()];
        format!("{:<width$} {}", line + 1, content)
    }

    fn target(&self, text: &Rope, line: usize) -> Option<usize> {
        let content = Cow::<str>::from(text.get_line(line)?);
        let start = text.line_to_char(line);
        let content = content.trim_end_matches('\n');
        let end = self
            .positive
            .as_ref()
            .and_then(|r| r.find(content))
            .map_or(0, |m| m.end());
        Some(start + content[..end].chars().count())
    }
}

/// The line index a candidate shows.
fn line_number(candidate: &str) -> Option<usize> {
    let number: usize = candidate.split(' ').next()?.parse().ok()?;
    number.checked_sub(1)
}

/// Swiper's regexps for `text`, as ivy's `ivy--regex-plus`: the words before the first `!` in
/// order, and the words after it, each excluding the lines it matches. A word that is not a valid
/// regexp is matched literally, and case is ignored while `text` has no upper-case letter.
fn regexes(text: &str) -> (Option<Regex>, Vec<Regex>) {
    let fold = text == text.to_lowercase();
    let build = |pattern: &str| (RegexBuilder::new(pattern).case_insensitive(fold).build()).ok();
    let or_literal = |w: &str| match Regex::new(w) {
        Ok(_) => w.to_string(),
        Err(_) => regex::escape(w),
    };
    let words = |part: &str| -> Vec<String> {
        let part = part.replace("\\!", "!").replace("\\ ", "\0");
        let words = part.split(' ').filter(|w| !w.is_empty());
        words.map(|w| w.replace('\0', " ")).collect()
    };
    let join = |words: &[String], quote: fn(&str) -> String| match words {
        [word] => quote(word),
        words => {
            let groups: Vec<String> = words.iter().map(|w| format!("({})", quote(w))).collect();
            groups.join(".*?")
        }
    };
    let bang = (text.char_indices()).find(|&(i, c)| c == '!' && !text[..i].ends_with('\\'));
    let (positive, negative) = match bang {
        Some((i, _)) => (&text[..i], &text[i + 1..]),
        None => (text, ""),
    };
    let positive = words(positive);
    // Words valid alone can still clash when joined, as two groups of one name.
    let positive = (!positive.is_empty())
        .then(|| {
            build(&join(&positive, or_literal)).or_else(|| build(&join(&positive, regex::escape)))
        })
        .flatten();
    let negative = words(negative)
        .iter()
        .filter_map(|w| build(&or_literal(w)))
        .collect();
    (positive, negative)
}

/// The line at point, selected when the list opens.
pub(crate) fn preselect(core: &Core, _: &Args) -> usize {
    let buffer = core.current();
    let (line, _) = buffer.line_col(core.point()).unwrap_or_default();
    line.min(lines(buffer.text()).saturating_sub(1))
}

/// The selected line's match in the window the search started from; with no text, the starting
/// line keeps its point.
pub(crate) fn preview(core: &Core, candidate: &str) -> Option<Preview> {
    let origin = core.origin()?;
    let buffer = core.buffers.get(origin.buffer())?;
    let (line, text) = (line_number(candidate)?, core.text());
    let point = origin.point(&core.buffers);
    if text.is_empty() && buffer.line_col(point).ok()?.0 == line {
        return Some(Preview::Point(point));
    }
    core.prompts
        .swiper()?
        .target(buffer.text(), line)
        .map(Preview::Point)
}

impl Ctx<'_> {
    /// Moves point to the end of the search's match on the chosen line, saving the mark where it
    /// started when point moved and no region is active.
    pub(crate) fn swiper(&mut self, candidate: &str) -> Result<(), String> {
        let line = line_number(candidate).ok_or("No candidates")?;
        let core = &mut *self.core;
        let start = core.point();
        let p = core
            .prompts
            .accepted_swiper
            .as_ref()
            .and_then(|search| search.target(core.current().text(), line))
            .ok_or("No candidates")?;
        if p != start {
            core.push_jump();
        }
        core.windows.window.jump(&mut core.buffers, p)?;
        if p != start && !core.windows.window.active() {
            core.windows
                .window
                .push_mark(&mut core.buffers, start, false)?;
            core.echo("Mark saved where search started".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn preview_matches_use_swiper_semantics() {
        use crate::display::Search as _;
        let text = "été Foo gap bar foo bar";
        let mut search = super::Search::new(&ropey::Rope::from_str(text));
        for (query, expected) in [
            ("foo bar", vec!["Foo gap bar", "foo bar"]),
            ("Foo", vec!["Foo"]),
            ("foo !gap", vec![]),
            ("!missing", vec![]),
            ("", vec![]),
            ("^", vec![]),
        ] {
            search.filter(query);
            let actual: Vec<_> = search.matches(text).into_iter().map(|r| &text[r]).collect();
            assert_eq!(actual, expected, "{query}");
        }
        search.filter("a(");
        assert_eq!(search.matches("a(b a(b"), [0..2, 4..6]);
    }

    use crate::Msg;
    use crate::harness::{Harness, line_start, mark, on};

    #[test]
    #[ignore = "manual reproducible Swiper baseline; prints timings"]
    fn prompt_and_query_baseline() {
        use crate::{Core, key::parse_keys};
        use std::time::{Duration, Instant};

        let queries = [
            "needle",
            "needle !skip",
            "NEEDLE",
            "alpha value",
            "(",
            "needle\\ value",
        ];
        let median = |mut samples: Vec<Duration>| {
            samples.sort();
            samples[samples.len() / 2].as_secs_f64() * 1000.
        };
        for lines in [1_000, 100_000] {
            let fixture: String = (0..lines)
                .map(|i| {
                    let kind = if i % 5 == 0 { "skip" } else { "keep" };
                    format!("{i:06} alpha needle value {kind} été\n")
                })
                .collect();
            let mut opening = Vec::new();
            let mut changes = vec![Vec::new(); queries.len()];
            for _ in 0..7 {
                let mut core = Core::default();
                core.open(
                    std::path::Path::new("/swiper-baseline.txt"),
                    Some((fixture.as_bytes().to_vec(), 0)),
                );
                let start = Instant::now();
                std::hint::black_box(core.handle(start, Msg::Key(parse_keys("C-s").unwrap()[0])));
                opening.push(start.elapsed());
                assert_eq!(
                    core.frame().minibuffer.unwrap().count,
                    format!("{:>6} ", format!("1/{lines}"))
                );
                let mut previous = 0;
                for (i, query) in queries.iter().enumerate() {
                    let start = Instant::now();
                    std::hint::black_box(core.handle(
                        start,
                        Msg::Insert {
                            text: (*query).into(),
                            replace: Some(0..previous),
                        },
                    ));
                    changes[i].push(start.elapsed());
                    previous = query.encode_utf16().count();
                    assert_eq!(core.text(), *query);
                    if i == 0 {
                        assert_eq!(
                            core.frame().minibuffer.unwrap().count,
                            format!("{:>6} ", format!("1/{lines}"))
                        );
                    }
                }
            }
            println!(
                "Swiper: lines={lines} bytes={} debug_assertions={} samples=7 open_median_ms={:.3}",
                fixture.len(),
                cfg!(debug_assertions),
                median(opening)
            );
            for (query, samples) in queries.iter().zip(changes) {
                println!("  query={query:?} median_ms={:.3}", median(samples));
            }
        }
    }

    #[test]
    fn snapshot_lines_keep_blank_lines_and_non_lf_separators() {
        let mut h = on("é\r\u{85}\u{2028}z\n\nlast");
        h.keys("C-s");
        assert_eq!(listed(&mut h).0, ["1 é\r\u{85}\u{2028}z", "2 ", "3 last"]);
        h.keys("z RET");
        assert_eq!(h.point(), 5);
    }

    #[test]
    fn chunk_spanning_lines_keep_unicode_matches_and_final_line_order() {
        let long = format!("{}é🙂 needle", "x".repeat(4000));
        let mut h = on(&format!("first\n{long}\nlast needle"));
        h.keys("C-s needle");
        assert_eq!(
            listed(&mut h).0,
            [format!("2 {long}"), "3 last needle".into()]
        );
        let expected = 6 + long.chars().count();
        assert_eq!(origin_point(&h), expected);
        h.keys("RET");
        assert_eq!(h.point(), expected);
        h.keys("C-s last RET");
        assert_eq!(h.point(), expected + 1 + 4);
    }

    #[test]
    fn preview_and_acceptance_follow_live_text_after_a_revert() {
        for replacement in ["prefix\n  needle\n", "gone"] {
            let mut h = on("a\nneedle\n");
            let buffer = h.core.windows.window.buffer();
            let tick = h.core.current().tick();
            let result = crate::buffer::revert(
                &h.path("a.txt"),
                h.core.current().text(),
                Ok(Some((
                    replacement.as_bytes().to_vec(),
                    crate::buffer::hash(replacement.as_bytes()),
                ))),
            );
            h.keys("C-s needle");
            h.send(Msg::Reverted {
                buffer,
                tick,
                result,
            });
            if replacement == "gone" {
                h.keys("RET");
                assert_eq!(h.core.message(), Some("No candidates"));
                assert_eq!(h.text(), "gone");
            } else {
                assert_eq!(origin_point(&h), 15);
                h.keys("RET");
                assert_eq!(h.point(), 15);
                assert_eq!(h.text(), replacement);
            }
        }
    }

    /// The listed candidates and the count before the prompt.
    fn listed(h: &mut Harness) -> (Vec<String>, String) {
        let mb = h.core.minibuffer().expect("the minibuffer is open");
        (mb.candidates, mb.count)
    }

    fn selected(h: &Harness) -> String {
        h.core
            .prompts
            .selected_candidate()
            .expect("something is selected")
    }

    /// The point of the window the search was started from.
    fn origin_point(h: &Harness) -> usize {
        h.core.windows.others[&h.core.windows.tree.leaves()[0]].point(&h.core.buffers)
    }

    #[test]
    fn c_s_lists_every_line_from_the_one_at_point() {
        let mut h = on("apple\nbanana\ncherry\n");
        h.keys("C-n C-s");
        let (candidates, count) = listed(&mut h);
        assert_eq!(candidates, ["1 apple", "2 banana", "3 cherry"]);
        assert_eq!(
            (selected(&h), count.as_str()),
            ("2 banana".into(), "   2/3 ")
        );
    }

    #[test]
    fn line_numbers_are_padded_and_not_matched() {
        let text: Vec<String> = (0..12)
            .map(|i| if i == 11 { "x1" } else { "x" }.into())
            .collect();
        let mut h = on(&text.join("\n"));
        h.keys("C-s");
        assert_eq!(listed(&mut h).0[0], "1  x");
        h.keys("1");
        assert_eq!(listed(&mut h).0, ["12 x1"]);
    }

    #[test]
    fn words_match_in_order() {
        for (text, input, expected) in [
            (
                "foo bar\nbar foo\nfoobar",
                "foo bar",
                &["1 foo bar", "3 foobar"][..],
            ),
            ("foo one\nfoo two", "foo !two", &["1 foo one"]),
            ("Foo\nfoo", "foo", &["1 Foo", "2 foo"]),
            ("Foo\nfoo", "Foo", &["1 Foo"]),
            ("a(b\nab", "a(", &["1 a(b"]),
            ("a b\nab", "a\\ b", &["1 a b"]),
            ("a!b\nab", "a\\!b", &["1 a!b"]),
            ("b1\na\nb2", "b", &["1 b1", "3 b2"]),
            ("x y\n", "(?P<n>x) (?P<n>y)", &[]),
            (
                "(?P<n>x) (?P<n>y)\n",
                "(?P<n>x) (?P<n>y)",
                &["1 (?P<n>x) (?P<n>y)"],
            ),
        ] {
            let mut h = on(text);
            let typed = Msg::Insert {
                text: input.into(),
                replace: None,
            };
            h.keys("C-s").send(typed);
            assert_eq!(listed(&mut h).0, expected, "{input:?} in {text:?}");
        }
    }

    #[test]
    fn the_selection_stays_near_its_line() {
        let mut h = on("x1\nx2\nx3\nx4\n");
        h.keys("C-n C-n C-s x");
        assert_eq!(selected(&h), "3 x3");
        let mut h = on("a\nb\nc\n");
        h.keys("C-n C-n C-s a");
        assert_eq!(selected(&h), "1 a", "no match after it selects the last");
        h.keys("z");
        assert_eq!(listed(&mut h).1, "   0/0 ");
        h.keys("DEL");
        assert_eq!(selected(&h), "1 a", "kept while nothing matches");
    }

    #[test]
    fn the_preview_follows_the_selection_and_c_g_restores() {
        let mut h = on("one\ntwo\nthree\n");
        let two = line_start(&h, 1);
        h.keys("C-f C-s");
        assert_eq!(
            origin_point(&h),
            1,
            "an empty search keeps point on its line"
        );
        h.keys("C-n");
        assert_eq!(origin_point(&h), two, "and goes to the start of another");
        h.keys("tw");
        assert_eq!(origin_point(&h), two + 2);
        assert!(
            h.core.prompts.minibuffer.is_some(),
            "the minibuffer stays selected"
        );
        h.keys("C-g");
        assert_eq!(h.point(), 1);
    }

    #[test]
    fn a_position_preview_keeps_the_scroll_while_in_view() {
        let text: Vec<String> = (0..300).map(|i| format!("l{i}")).collect();
        let mut h = on(&text.join("\n"));
        h.display(20);
        let start = line_start(&h, 102);
        h.keys("C-s l100$");
        let top = |h: &mut Harness| {
            let id = h.core.windows.tree.leaves()[0];
            h.core
                .windows
                .others
                .get_mut(&id)
                .unwrap()
                .scroll(&h.core.buffers, 20)
        };
        assert_eq!(top(&mut h), 90, "out of view centers");
        h.keys("C-a C-k l102$");
        assert_eq!(origin_point(&h), start + 4);
        assert_eq!(top(&mut h), 90, "in view keeps the scroll");
        h.keys("C-g");
        let window = &mut h.core.windows.window;
        assert_eq!(
            window.scroll(&h.core.buffers, 20),
            0,
            "C-g restores the scroll"
        );
    }

    #[test]
    fn search_move_jump() {
        let mut h = on("alpha\nbeta\nalpine\n");
        h.keys("C-s al C-n RET");
        assert_eq!(h.point(), line_start(&h, 2) + 2);
        assert_eq!(mark(&h), (Some(0), false));
        assert_eq!(h.core.message(), Some("Mark saved where search started"));
        assert!(h.core.prompts.minibuffer.is_none());
    }

    #[test]
    fn ret_with_no_text_goes_to_the_line_start() {
        // As ivy's `swiper--action`, which searches `""` from the line's start.
        let mut h = on("alpha\n");
        h.keys("C-f C-f C-s RET");
        assert_eq!((h.point(), mark(&h)), (0, (Some(2), false)));
    }

    #[test]
    fn an_active_region_keeps_its_mark() {
        let mut h = on("alpha\nbeta\n");
        h.keys("C-SPC C-f C-s beta RET");
        assert_eq!(h.point(), line_start(&h, 1) + 4);
        assert_eq!(mark(&h), (Some(0), true));
    }

    #[test]
    fn c_s_repeats_and_recalls_the_last_search() {
        let mut h = on("a1\na2\nbeta\n");
        h.keys("C-s a");
        assert_eq!(selected(&h), "1 a1");
        h.keys("C-s");
        assert_eq!(selected(&h), "2 a2");
        h.keys("C-g C-s beta RET C-s C-s");
        assert_eq!(h.core.text(), "beta");
    }

    #[test]
    fn history_keeps_typed_searches_even_when_quit() {
        let mut h = on("gamma\n");
        h.keys("C-s gam C-g C-s M-p");
        assert_eq!(h.core.text(), "gam");
    }

    #[test]
    fn an_empty_buffer_lists_nothing() {
        let mut h = on("");
        h.keys("C-s");
        assert_eq!(listed(&mut h), (vec![], "   0/0 ".into()));
        h.keys("1 RET");
        assert_eq!(h.core.message(), Some("Match required"));
    }

    #[test]
    fn ret_requires_a_match() {
        let mut h = on("abc\n");
        h.keys("C-s zzz RET");
        assert_eq!(h.core.message(), Some("Match required"));
        assert!(h.core.prompts.minibuffer.is_some());
    }
}
