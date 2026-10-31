//! The workspace overview, fitted around the current workspace.

use std::ops::Range;

use crate::Core;
use crate::agents::Activity;
use crate::buffer::Face;
use crate::display::Span;
use crate::settings::{FaceName, Theme};

struct Entry {
    project: String,
    workspace: String,
    current: bool,
    activity: Option<Activity>,
}

impl Entry {
    // shortcut: wide glyphs count as one column; measure glyphs when workspace labels need them.
    fn width(&self) -> usize {
        self.project.chars().count()
            + 3
            + self.workspace.chars().count()
            + usize::from(self.activity.is_some()) * 2
    }

    fn draw(&self, cols: usize, theme: &Theme) -> Vec<Span> {
        let bar = theme.face(FaceName::WorkspaceBar);
        let base = if self.current {
            bar
        } else {
            theme.face(FaceName::ModeLineDim).over(bar)
        };
        let name = if self.current {
            theme.face(FaceName::WorkspaceCurrent).over(base)
        } else {
            base
        };
        let dot = self.activity.filter(|_| cols >= 3);
        let room = cols.saturating_sub(usize::from(dot.is_some()) * 2);
        let mut spans = if room >= 5 {
            let (project, workspace) =
                crate::frame::fit_names(&self.project, &self.workspace, room);
            vec![
                span(project, Face { bold: true, ..base }),
                span(" › ", theme.face(FaceName::Separator).over(bar)),
                span(workspace, name),
            ]
        } else {
            let text = if self.workspace.chars().count() <= room {
                self.workspace.clone()
            } else {
                self.workspace
                    .chars()
                    .take(room.saturating_sub(1))
                    .chain((room > 0).then_some('…'))
                    .collect()
            };
            vec![span(text, name)]
        };
        if let Some(activity) = dot {
            spans.push(span(" ●", theme.face(activity.face()).over(bar)));
        }
        spans
    }
}

fn span(text: impl Into<String>, face: Face) -> Span {
    Span {
        text: text.into(),
        face,
        fold: false,
    }
}

impl Core {
    pub(crate) fn workspace_modeline(&self, cols: usize) -> Vec<Span> {
        let entries: Vec<_> = self
            .workspaces
            .iter()
            .map(|w| Entry {
                project: self.project_name(w.project).replace(char::is_control, " "),
                workspace: w.name.replace(char::is_control, " "),
                current: w.id == self.workspace(),
                activity: self
                    .agents
                    .values()
                    .filter(|s| {
                        self.buffers
                            .get(s.buffer)
                            .is_some_and(|b| b.workspace == Some(w.id))
                    })
                    .filter_map(|s| s.activity())
                    .max(),
            })
            .collect();
        fit(&entries, cols, &self.settings.theme)
    }
}

fn fit(entries: &[Entry], cols: usize, theme: &Theme) -> Vec<Span> {
    let current = entries
        .iter()
        .position(|e| e.current)
        .expect("the current workspace is live");
    let hidden = |range: &Range<usize>| {
        entries
            .iter()
            .enumerate()
            .filter(|(i, _)| !range.contains(i))
            .filter_map(|(_, e)| e.activity)
            .max()
    };
    let overflow = |range: &Range<usize>| {
        let count = entries.len() - range.len();
        if count == 0 {
            return Vec::new();
        }
        let base = theme.face(FaceName::WorkspaceBar);
        let mut spans = vec![span(
            format!("+{count}"),
            theme.face(FaceName::ModeLineDim).over(base),
        )];
        if let Some(activity) = hidden(range) {
            spans.push(span(" ●", theme.face(activity.face()).over(base)));
        }
        spans
    };
    let tail_width = |range: &Range<usize>| {
        let tail = overflow(range);
        if tail.is_empty() {
            0
        } else {
            3 + tail.iter().map(|s| s.text.chars().count()).sum::<usize>()
        }
    };
    let width = |range: &Range<usize>| {
        entries[range.clone()]
            .iter()
            .map(Entry::width)
            .sum::<usize>()
            + 3 * range.len().saturating_sub(1)
            + tail_width(range)
    };
    let mut range = 0..entries.len();
    if width(&range) > cols {
        range = current..current + 1;
        loop {
            let left = (range.start > 0).then(|| range.start - 1..range.end);
            let right = (range.end < entries.len()).then(|| range.start..range.end + 1);
            let candidates = if current - range.start < range.end - current {
                [left, right]
            } else {
                [right, left]
            };
            match candidates.into_iter().flatten().find(|r| width(r) <= cols) {
                Some(next) => range = next,
                None => break,
            }
        }
    }
    let mut tail = overflow(&range);
    let mut tail_cols = tail_width(&range);
    let minimum = 5 + usize::from(entries[current].activity.is_some()) * 2;
    if range.len() == 1 && cols < minimum + tail_cols {
        tail.clear();
        tail_cols = 0;
    }
    let separator = || {
        span(
            " │ ",
            theme
                .face(FaceName::Separator)
                .over(theme.face(FaceName::WorkspaceBar)),
        )
    };
    let mut spans = Vec::new();
    for (i, entry) in entries[range.clone()].iter().enumerate() {
        if i > 0 {
            spans.push(separator());
        }
        spans.extend(entry.draw(cols.saturating_sub(tail_cols), theme));
    }
    if !tail.is_empty() {
        spans.push(separator());
        spans.extend(tail);
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Harness;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    fn entries(current: usize) -> Vec<Entry> {
        ['a', 'b', 'c', 'd', 'e']
            .into_iter()
            .enumerate()
            .map(|(i, name)| Entry {
                project: "p".into(),
                workspace: name.to_string(),
                current: i == current,
                activity: match i {
                    0 => Some(Activity::Failed),
                    4 => Some(Activity::Waiting),
                    _ => None,
                },
            })
            .collect()
    }

    #[test]
    fn overflow_keeps_the_current_workspace_and_its_neighbours() {
        let theme = Theme::default();
        let entries = entries(2);
        let spans = fit(&entries, 28, &theme);
        assert_eq!(text(&spans), "p › b │ p › c │ p › d │ +2 ●");
        assert_eq!(
            spans.last().unwrap().face.fg,
            theme.face(FaceName::StatusWaiting).fg
        );
        assert_eq!(text(&fit(&entries, 20, &theme)), "p › b │ p › c │ +3 ●");
        assert_eq!(
            text(&fit(&entries, 100, &theme)),
            "p › a ● │ p › b │ p › c │ p › d │ p › e ●"
        );
        assert_eq!(
            text(&fit(&self::entries(0), 22, &theme)),
            "p › a ● │ p › b │ +3 ●"
        );
        assert_eq!(
            text(&fit(&self::entries(4), 22, &theme)),
            "p › d │ p › e ● │ +3 ●"
        );
    }

    #[test]
    fn hidden_activity_uses_the_same_priority_as_visible_activity() {
        let theme = Theme::default();
        let mut entries = entries(2);
        for (activity, face) in [
            (Some(Activity::Waiting), FaceName::StatusWaiting),
            (Some(Activity::Working), FaceName::StatusFailed),
            (None, FaceName::StatusFailed),
        ] {
            entries[4].activity = activity;
            let spans = fit(&entries, 28, &theme);
            assert_eq!(spans.last().unwrap().face.fg, theme.face(face).fg);
        }
        entries[0].activity = Some(Activity::Working);
        let spans = fit(&entries, 28, &theme);
        assert_eq!(
            spans.last().unwrap().face.fg,
            theme.face(FaceName::StatusRunning).fg
        );
        entries[0].activity = None;
        assert_eq!(
            text(&fit(&entries, 28, &theme)),
            "p › b │ p › c │ p › d │ +2"
        );
    }

    #[test]
    fn long_names_and_tiny_widths_keep_the_current_workspace_within_budget() {
        let theme = Theme::default();
        let mut entries = entries(2);
        entries[2].project = "Legacy crap".into();
        entries[2].workspace = "é".repeat(60);
        let spans = fit(&entries, 47, &theme);
        assert_eq!(
            text(&spans),
            format!("Legacy crap › {}… │ +4 ●", "é".repeat(25))
        );
        for current in 0..entries.len() {
            for (i, entry) in entries.iter_mut().enumerate() {
                entry.current = i == current;
            }
            for cols in 0..150 {
                let spans = fit(&entries, cols, &theme);
                assert!(
                    text(&spans).chars().count() <= cols,
                    "current {current}, width {cols}: {}",
                    text(&spans)
                );
                assert!(
                    spans
                        .iter()
                        .any(|s| s.face.fg == theme.face(FaceName::WorkspaceCurrent).fg)
                );
            }
        }
        entries[2].current = true;
        entries[4].current = false;
        assert_eq!(text(&fit(&entries, 1, &theme)), "…");
    }

    #[test]
    fn frame_fitting_reserves_the_clock_at_the_workspace_font_size() {
        let mut h = Harness::new(&[]);
        h.core.settings.font_size = 15.;
        h.core.settings.workspace_font_size = 15.;
        h.new_workspace("b", "p");
        h.new_workspace("c", "p");
        // 40 columns inside the insets, seven reserved for the clock.
        let normal = h.frame(344., 600.).workspaces;
        assert_eq!(text(&normal), "home › main │ p › b │ p › c");
        h.core.settings.workspace_font_size = 30.;
        let large = h.frame(344., 600.).workspaces;
        assert_eq!(text(&large), "p › c │ +2");
    }
}
