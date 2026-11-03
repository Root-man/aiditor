//! File chips: each one private-use char in a session buffer's text, standing for a file, an
//! image or a directory, drawn as its icon and name. Being one char, motion, deletion, kill,
//! yank and undo need nothing of their own.

use agent_client_protocol_schema::v1::{ContentBlock, ImageContent, ResourceLink};
use regex::Regex;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::buffer::{Buffer, FaceRef, Prop};
use crate::settings::FaceName;

/// Chip `i` is this char plus `i`, in Supplementary Private Use Area-A and then -B.
const FIRST: u32 = 0xF0000;

/// How Claude's adapter writes a sent `resource_link` into the transcript it replays.
/// How the Claude Code CLI writes a mention into the transcript it replays: `@` and the path.
static MENTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|\s)@(\S+)").expect("valid"));

static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[@[^\]\n]*\]\(file://([^)\n]+)\)").expect("valid"));

/// Whether `c` is a chip's char, which word motion takes as a word of its own.
pub(crate) fn is_chip(c: char) -> bool {
    c as u32 >= FIRST
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    File,
    Image,
    Directory,
}

pub(crate) struct Chip {
    /// `None` for a replayed image, which comes as data alone.
    pub(crate) path: Option<PathBuf>,
    pub(crate) kind: Kind,
    /// Its file is gone from disk.
    pub(crate) gone: bool,
}

impl Chip {
    fn name(&self) -> String {
        let path = self.path.as_deref();
        let name = path.and_then(Path::file_name).map(|n| n.to_string_lossy());
        name.map_or_else(|| "image".into(), |n| n.into_owned())
    }

    fn uri(&self, path: &Path) -> String {
        let slash = if self.kind == Kind::Directory {
            "/"
        } else {
            ""
        };
        // shortcut: not percent-encoded, as Claude's adapter slices paths out of URIs raw, and a
        // replayed link ends at the first `)`; encode if an agent rejects a path with spaces or
        // parentheses.
        format!("file://{}{slash}", path.display())
    }

    /// The text drawn in its place, a space each side for the box's padding.
    fn pieces(&self) -> Vec<(String, Vec<FaceRef>)> {
        let mut face = vec![FaceRef::Face(FaceName::Attachment)];
        if self.gone {
            face.push(FaceRef::Strike);
        }
        let mut icon = face.clone();
        if self.gone {
            icon.push(FaceRef::Face(FaceName::StatusFailed));
        }
        let glyph = match self.kind {
            Kind::File => "▤",
            Kind::Image => "▣",
            Kind::Directory => "▦",
        };
        let name = format!(" {} ", self.name());
        vec![
            (" ".into(), face.clone()),
            (glyph.into(), icon),
            (name, face),
        ]
    }
}

/// Every chip seen, interned by path so a chip's char means the same in every session.
#[derive(Default)]
pub(crate) struct Chips {
    chips: Vec<Chip>,
    /// Paths interned since the last check was sent.
    pub(crate) unchecked: Vec<PathBuf>,
}

impl Chips {
    /// The char standing for `path`, a directory when `directory`; `None` is a replayed image.
    pub(crate) fn intern(&mut self, path: Option<PathBuf>, directory: bool) -> char {
        let at = match self.chips.iter().position(|c| c.path == path) {
            Some(at) => at,
            None => {
                let image = |p: &Path| {
                    let ext = p.extension().map(|e| e.to_ascii_lowercase());
                    ext.is_some_and(|e| {
                        ["png", "jpg", "jpeg", "gif", "webp"]
                            .iter()
                            .any(|i| e == *i)
                    })
                };
                let kind = match &path {
                    _ if directory => Kind::Directory,
                    Some(p) if !image(p) => Kind::File,
                    _ => Kind::Image,
                };
                self.unchecked.extend(path.clone());
                let gone = false;
                self.chips.push(Chip { path, kind, gone });
                self.chips.len() - 1
            }
        };
        char::from_u32(FIRST + at as u32).expect("fewer chips than private-use chars")
    }

    pub(crate) fn get(&self, c: char) -> Option<&Chip> {
        self.chips.get((c as u32).checked_sub(FIRST)? as usize)
    }

    /// The paths of chips at or under `path`.
    pub(crate) fn under<'a>(&'a self, path: &'a Path) -> impl Iterator<Item = &'a PathBuf> {
        (self.chips.iter()).filter_map(move |c| c.path.as_ref().filter(|p| p.starts_with(path)))
    }

    /// Marks `path`'s chip gone or back; whether that changed it.
    pub(crate) fn set_gone(&mut self, path: &Path, gone: bool) -> bool {
        let chip = self
            .chips
            .iter_mut()
            .find(|c| c.path.as_deref() == Some(path));
        chip.is_some_and(|c| std::mem::replace(&mut c.gone, gone) != gone)
    }

    /// `text` with each `[@name](file://path)` link replaced by its chip.
    pub(crate) fn replace_links(&mut self, text: &str) -> String {
        (LINK.replace_all(text, |c: &regex::Captures| {
            let path = &c[1];
            let dir = path.len() > 1 && path.ends_with('/');
            let path = if dir { &path[..path.len() - 1] } else { path };
            self.intern(Some(path.into()), dir).to_string()
        }))
        .into_owned()
    }

    /// `text` with each `@path` mention that `file` resolves to a file replaced by its chip.
    pub(crate) fn replace_mentions(
        &mut self,
        text: &str,
        file: impl Fn(&str) -> Option<PathBuf>,
    ) -> String {
        (MENTION.replace_all(text, |c: &regex::Captures| match file(&c[2]) {
            Some(path) => format!("{}{}", &c[1], self.intern(Some(path), false)),
            None => c[0].to_string(),
        }))
        .into_owned()
    }

    /// `input` as prompt blocks: text runs split at its chips, those of only whitespace left
    /// out, a file or directory chip a `resource_link`, an image one image content whose data
    /// the agent service reads from its URI.
    pub(crate) fn blocks(&self, input: &str) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();
        let mut text = String::new();
        let flush = |text: &mut String, blocks: &mut Vec<ContentBlock>| {
            let run = std::mem::take(text);
            if !run.trim().is_empty() {
                blocks.push(run.into());
            }
        };
        for c in input.chars() {
            let Some(chip) = self.get(c) else {
                text.push(c);
                continue;
            };
            flush(&mut text, &mut blocks);
            // A replayed image has no data left to send.
            let Some(path) = &chip.path else {
                continue;
            };
            let uri = chip.uri(path);
            blocks.push(match chip.kind {
                Kind::Image => {
                    let ext = path.extension().unwrap_or_default().to_ascii_lowercase();
                    let mime = match ext.to_string_lossy().as_ref() {
                        "jpg" | "jpeg" => "image/jpeg".to_string(),
                        ext => format!("image/{ext}"),
                    };
                    ContentBlock::Image(ImageContent::new("", mime).uri(uri))
                }
                _ => ContentBlock::ResourceLink(ResourceLink::new(chip.name(), uri)),
            });
        }
        flush(&mut text, &mut blocks);
        blocks
    }

    /// Draws each chip in `range` of `buffer` as its icon and name.
    pub(crate) fn paint(&self, buffer: &mut Buffer, range: Range<usize>) {
        let chips: Vec<_> = (buffer.text().slice(range.clone()).chars().enumerate())
            .filter_map(|(i, c)| Some((range.start + i, self.get(c)?.pieces())))
            .collect();
        for (at, pieces) in chips {
            buffer.remove_properties(at..at + 1);
            (buffer.add_property(at..at + 1, Prop::Display(pieces))).expect("inside the text");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_names_and_links() {
        let mut chips = Chips::default();
        let a = chips.intern(Some("/p/a.rs".into()), false);
        assert_eq!(chips.intern(Some("/p/a.rs".into()), false), a, "interned");
        let cat = chips.intern(Some("/p/Cat.PNG".into()), false);
        assert_eq!(chips.get(cat).unwrap().kind, Kind::Image);
        assert_eq!(chips.get('a').map(|c| c.kind), None);
        let text = chips.replace_links("see [@src](file:///p/src/) and [@a.rs](file:///p/a.rs)");
        let src = text.chars().nth(4).unwrap();
        assert_eq!(chips.get(src).unwrap().kind, Kind::Directory);
        assert_eq!(chips.get(src).unwrap().name(), "src");
        assert_eq!(text, format!("see {src} and {a}"));
        assert_eq!(chips.unchecked.len(), 3);
        let file = |w: &str| (w == "a.rs").then(|| PathBuf::from("/p/a.rs"));
        let text = chips.replace_mentions("@a.rs and @b.rs, me@a.rs", file);
        assert_eq!(text, format!("{a} and @b.rs, me@a.rs"));
    }
}
