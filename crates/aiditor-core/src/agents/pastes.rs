//! Pasted text stays in the rope; markers bound its chip and preview.

use super::*;

const PREVIEW: usize = 4;

pub(super) struct Paste {
    start: MarkerId,
    end: MarkerId,
    open: bool,
}

impl Paste {
    fn range(&self, buffer: &Buffer) -> Range<usize> {
        buffer.marker(self.start).expect("session marker")
            ..buffer.marker(self.end).expect("session marker")
    }

    fn paint(&self, buffer: &mut Buffer) {
        let range = self.range(buffer);
        if range.is_empty() {
            return;
        }
        let text = buffer.text().slice(range.clone()).to_string();
        let lines = text.lines().count();
        let label = format!(" ≡ Pasted {lines} lines ");
        let attachment = vec![FaceRef::Face(FaceName::Attachment)];
        buffer.remove_properties(range.clone());
        if !self.open {
            buffer
                .add_property(range, Prop::Display(vec![(label, attachment)]))
                .expect("paste range");
            return;
        }
        let block = vec![FaceRef::Face(FaceName::SessionBlock)];
        let first = text.chars().next().expect("nonempty paste");
        buffer
            .add_property(
                range.start..range.start + 1,
                Prop::Display(vec![
                    (label, attachment),
                    (format!("\n{first}"), block.clone()),
                ]),
            )
            .expect("paste header");
        let preview = text
            .char_indices()
            .filter(|(_, c)| *c == '\n')
            .nth(PREVIEW - 1)
            .map_or(text.chars().count(), |(at, _)| text[..=at].chars().count());
        let end = range.start + preview;
        buffer
            .add_property(range.start..end, Prop::Named(block))
            .expect("preview range");
        if lines > PREVIEW {
            buffer
                .add_property(
                    end..range.end,
                    Prop::Display(vec![(
                        format!("… {} more lines", lines - PREVIEW),
                        vec![FaceRef::Face(FaceName::Annotation)],
                    )]),
                )
                .expect("paste tail");
        }
    }
}

impl Session {
    pub(super) fn paint_pastes(&mut self, buffer: &mut Buffer, from: usize) {
        // shortcut: undo restores text without fold markers; include folds in history if their restoration is needed.
        self.pastes.retain(|paste| {
            let range = paste.range(buffer);
            if range.start < range.end {
                true
            } else {
                buffer.remove_marker(paste.start);
                buffer.remove_marker(paste.end);
                false
            }
        });
        for paste in &self.pastes {
            if paste.range(buffer).start >= from {
                paste.paint(buffer);
            }
        }
    }

    pub(super) fn fold_pastes(&mut self, buffer: &mut Buffer, from: usize) {
        for paste in &mut self.pastes {
            if paste.range(buffer).start >= from {
                paste.open = false;
            }
        }
        self.paint_pastes(buffer, from);
    }

    /// The paste `point` is in.
    pub(super) fn paste_at(&self, buffer: &Buffer, point: usize) -> Option<usize> {
        (self.pastes.iter()).position(|p| p.range(buffer).contains(&point))
    }

    /// Opens or folds paste `i`, returning where it starts.
    pub(super) fn toggle_paste(&mut self, buffer: &mut Buffer, i: usize) -> usize {
        let paste = &mut self.pastes[i];
        paste.open = !paste.open;
        paste.paint(buffer);
        paste.range(buffer).start
    }
}

impl Core {
    pub(crate) fn fold_yank(&mut self, start: usize, text: &str) {
        let buffer = self.windows.window.buffer();
        let Some(session) = self.agents.values_mut().find(|s| s.buffer == buffer) else {
            return;
        };
        let buffer = self.buffers.get_mut(buffer).expect("current buffer");
        session.paint_pastes(buffer, buffer.prompt_start());
        if start < buffer.prompt_start() || text.lines().count() <= self.settings.paste_fold_lines {
            return;
        }
        let end = start + text.chars().count();
        // A yank inside an existing paste becomes its text, rather than a nested fold.
        if session
            .pastes
            .iter()
            .any(|p| p.range(buffer).contains(&start))
        {
            return;
        }
        session.pastes.push(Paste {
            start: buffer
                .add_marker(start, InsertionType::Advance)
                .expect("yanked text"),
            end: buffer
                .add_marker(end, InsertionType::Stay)
                .expect("yanked text"),
            open: false,
        });
        session.paint_pastes(buffer, buffer.prompt_start());
    }
}
