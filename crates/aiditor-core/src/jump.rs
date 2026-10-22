//! Jump history: the positions jumps left, per workspace, as Emacs's xref history.

use crate::Core;
use crate::buffer::{BufferId, Buffers, InsertionType, MarkerId};

/// How many positions the back list keeps.
const JUMPS_MAX: usize = 100;

impl Core {
    /// Records the selected window's position as one a jump leaves, emptying the forward list.
    pub(crate) fn push_jump(&mut self) {
        let (id, point) = (self.window.buffer(), self.point());
        let current = self.workspace();
        let ws = (self.workspaces.iter_mut().find(|w| w.id == current)).expect("current is live");
        let buffer = self.buffers.get_mut(id).expect("window buffer is live");
        let mut gone = std::mem::take(&mut ws.forward);
        if !(ws.back.last()).is_some_and(|&(b, m)| b == id && buffer.marker(m) == Some(point)) {
            let marker = buffer.add_marker(point, InsertionType::Stay);
            ws.back.push((id, marker.expect("point is in the buffer")));
            let dropped = ws.back.len().saturating_sub(JUMPS_MAX);
            gone.extend(ws.back.drain(..dropped));
        }
        release(&mut self.buffers, gone);
    }

    /// Goes to the newest position on the back list, or the forward list when `forward`, that is
    /// live and not where point is, putting point's position on the other list.
    pub(crate) fn xref_go(&mut self, forward: bool) -> Result<(), String> {
        let (id, point) = (self.window.buffer(), self.point());
        let current = self.workspace();
        let ws = (self.workspaces.iter_mut().find(|w| w.id == current)).expect("current is live");
        let (from, to) = match forward {
            false => (&mut ws.back, &mut ws.forward),
            true => (&mut ws.forward, &mut ws.back),
        };
        let (b, p) = loop {
            let Some((b, m)) = from.pop() else {
                let end = if forward { "end" } else { "start" };
                return Err(format!("At {end} of xref history"));
            };
            // A killed buffer took its markers with it.
            let Some(buffer) = self.buffers.get_mut(b) else {
                continue;
            };
            let p = buffer.marker(m).expect("a live buffer keeps its markers");
            buffer.remove_marker(m);
            if (b, p) != (id, point) {
                break (b, p);
            }
        };
        let buffer = self.buffers.get_mut(id).expect("window buffer is live");
        let marker = buffer.add_marker(point, InsertionType::Stay);
        to.push((id, marker.expect("point is in the buffer")));
        self.show_buffer(b);
        Ok(self.window.jump(&mut self.buffers, p)?)
    }
}

/// Removes the markers of jump positions from the buffers still live.
pub(crate) fn release(
    buffers: &mut Buffers,
    jumps: impl IntoIterator<Item = (BufferId, MarkerId)>,
) {
    for (b, m) in jumps {
        if let Some(buffer) = buffers.get_mut(b) {
            buffer.remove_marker(m);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::Msg;
    use crate::harness::{Harness, line_start, on};

    const WAIT: Duration = Duration::from_millis(200);

    /// The current workspace's back list as (buffer name, position), oldest first.
    fn back(h: &Harness) -> Vec<(String, usize)> {
        let ws = h.core.current_workspace();
        (ws.back.iter())
            .map(|&(b, m)| {
                let buffer = h.core.buffers.get(b).unwrap();
                (buffer.name.clone(), buffer.marker(m).unwrap())
            })
            .collect()
    }

    fn newest(h: &Harness) -> Option<(String, usize)> {
        back(h).pop()
    }

    /// The current buffer's name and point.
    fn at(h: &Harness) -> (String, usize) {
        (h.core.current().name.clone(), h.point())
    }

    fn pos(name: &str, p: usize) -> (String, usize) {
        (name.into(), p)
    }

    /// Files under `app`, the current project's only root, with `a.rs` visited.
    fn grepping(files: &[(&str, &str)]) -> Harness {
        let mut h = Harness::new(files);
        h.core.projects[0].project.primary.path = h.path("app");
        h.open("app/a.rs");
        h
    }

    #[test]
    fn the_back_list_skips_repeats_and_keeps_100() {
        let mut h = on(&"x".repeat(200));
        h.core.push_jump();
        h.core.push_jump();
        assert_eq!(
            back(&h),
            [pos("*scratch*<main>", 0), pos("a.txt", 0)],
            "a repeat is not recorded"
        );
        let (b, oldest) = h.core.current_workspace().back[0];
        for p in 1..=100 {
            h.core.window.set_point(&mut h.core.buffers, p).unwrap();
            h.core.push_jump();
        }
        let back = back(&h);
        assert_eq!((back.len(), back[0].1), (100, 1));
        assert_eq!(h.core.buffers.get(b).unwrap().marker(oldest), None);
    }

    #[test]
    fn back_skips_where_point_is() {
        let mut h = on("abc");
        h.core.push_jump();
        h.keys("M-,");
        assert_eq!(at(&h), pos("*scratch*<main>", 0));
    }

    #[test]
    fn a_buffer_switch_records_where_it_left() {
        let mut h = Harness::new(&[("a.txt", "abcdef"), ("b.txt", "b")]);
        h.open("a.txt").keys("C-f C-f C-f");
        h.open("b.txt");
        assert_eq!(newest(&h), Some(pos("a.txt", 3)));
        h.keys("C-x b b.txt RET");
        assert_eq!(
            newest(&h),
            Some(pos("a.txt", 3)),
            "the shown buffer is no jump"
        );
        let a = h.core.buffers.find("a.txt").unwrap();
        h.core.buffers.get_mut(a).unwrap().insert(0, "xy").unwrap();
        assert_eq!(newest(&h), Some(pos("a.txt", 5)), "positions follow edits");
        h.keys("C-x 4 b a.txt RET");
        assert_eq!(
            newest(&h),
            Some(pos("b.txt", 0)),
            "C-x 4 b leaves the other window"
        );
    }

    #[test]
    fn swiper_records_a_jump_but_not_a_preview() {
        let mut h = on("one\ntwo\nthree\n");
        h.keys("C-s thr C-g");
        assert_eq!(back(&h), [pos("*scratch*<main>", 0)]);
        h.keys("C-s thr RET");
        assert_eq!(newest(&h), Some(pos("a.txt", 0)));
    }

    #[test]
    fn back_after_a_grep_jump_returns_and_forward_goes_again() {
        let mut h = grepping(&[("app/a.rs", "needle\n"), ("app/b.rs", "x\n  needle\n")]);
        h.keys("C-f C-x p g needle").advance(WAIT).keys("C-n RET");
        let hit = line_start(&h, 1) + 2;
        assert_eq!(at(&h), pos("b.rs", hit));
        h.keys("M-,");
        assert_eq!(at(&h), pos("a.rs", 1));
        h.keys("C-M-,");
        assert_eq!(at(&h), pos("b.rs", hit));
        h.keys("C-M-,");
        assert_eq!(h.core.message(), Some("At end of xref history"));
    }

    #[test]
    fn a_grep_hit_in_the_current_buffer_records_once() {
        let mut h = grepping(&[("app/a.rs", "x\nneedle\n")]);
        let before = back(&h).len();
        h.keys("C-x p g needle").advance(WAIT).keys("RET");
        assert_eq!(h.point(), 2);
        assert_eq!(back(&h).len(), before + 1);
        assert_eq!(newest(&h), Some(pos("a.rs", 0)));
        h.keys("M-,");
        assert_eq!(at(&h), pos("a.rs", 0));
    }

    fn abc() -> Harness {
        let mut h = Harness::new(&[("a.txt", "a"), ("b.txt", "b"), ("c.txt", "c")]);
        h.open("a.txt").open("b.txt").open("c.txt");
        h
    }

    #[test]
    fn back_and_forward_go_through_several_jumps() {
        let mut h = abc();
        h.keys("M-, M-,");
        assert_eq!(at(&h).0, "a.txt");
        h.keys("C-M-, C-M-,");
        assert_eq!(at(&h).0, "c.txt");
    }

    #[test]
    fn back_skips_a_killed_buffer() {
        let mut h = abc();
        let b = h.core.buffers.find("b.txt").unwrap();
        h.core.kill_buffer(b);
        h.keys("M-,");
        assert_eq!(at(&h).0, "a.txt");
    }

    #[test]
    fn back_centers_a_far_line() {
        let text: Vec<String> = (0..40).map(|i| i.to_string()).collect();
        let mut h = on(&text.join("\n"));
        h.display(5);
        let far = line_start(&h, 30);
        h.core.window.set_point(&mut h.core.buffers, far).unwrap();
        h.core.push_jump();
        h.keys("M-<");
        h.display(5);
        h.keys("M-,");
        assert_eq!(h.point(), far);
        assert_eq!(h.core.window.scroll(&h.core.buffers, 5), 28);
    }

    #[test]
    fn empty_lists_say_so() {
        let mut h = Harness::new(&[]);
        h.keys("C-M-,");
        assert_eq!(h.core.message(), Some("At end of xref history"));
        h.keys("M-,");
        assert_eq!(h.core.message(), Some("At start of xref history"));
    }

    #[test]
    fn a_new_jump_empties_forward() {
        let mut h = Harness::new(&[("a.txt", "a"), ("b.txt", "b"), ("c.txt", "c")]);
        h.open("a.txt").open("b.txt").keys("M-,");
        h.open("c.txt").keys("C-M-,");
        assert_eq!(
            (at(&h).0, h.core.message()),
            ("c.txt".into(), Some("At end of xref history"))
        );
    }

    #[test]
    fn each_workspace_has_its_own_history() {
        let mut h = Harness::new(&[("a.txt", "a"), ("b.txt", "b")]);
        h.open("a.txt").open("b.txt");
        let one = h.core.workspace();
        h.core.new_workspace("two", "home");
        h.keys("M-,");
        assert_eq!(h.core.message(), Some("At start of xref history"));
        h.core.switch_workspace(one).unwrap();
        h.keys("M-,");
        assert_eq!(at(&h).0, "a.txt");
    }

    #[test]
    fn killing_a_workspace_drops_its_markers() {
        let mut h = Harness::new(&[("a.txt", "a")]);
        let two = h.core.new_workspace("two", "home");
        h.send(Msg::Log("hi".into())).keys("C-x b *Messages* RET");
        h.open("a.txt");
        let (b, m) = *h.core.current_workspace().back.last().unwrap();
        assert_eq!(h.core.buffers.get(b).unwrap().name, "*Messages*");
        h.core.kill_workspace(two).unwrap();
        assert_eq!(h.core.buffers.get(b).unwrap().marker(m), None);
    }
}
