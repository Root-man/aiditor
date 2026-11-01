//! Named commands, the arguments they declare and the arguments they get.

use std::ops::Range;
use std::path::PathBuf;

use crate::buffer::BufferId;
use crate::{Core, Ctx};

pub type Handler = fn(&mut Ctx, Args) -> Result<(), String>;

/// Given the text and every candidate, the indices of those that match, in the order to list them.
#[derive(Clone, Copy, Debug)]
pub enum Filter {
    Items(fn(&str, &[String]) -> Vec<usize>),
    Swiper,
}

#[derive(Clone, Copy)]
pub struct Command {
    pub name: &'static str,
    pub spec: ArgSpec,
    pub handler: Handler,
}

/// What a command needs besides the prefix argument, which every command gets.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArgSpec {
    /// Takes the region as a char range.
    pub region: bool,
    pub project: bool,
    pub unselected: bool,
    pub project_candidates: bool,
    pub grep_hits: bool,
    /// Reads the system clipboard, which the app then sends before the key that runs it.
    pub clipboard: bool,
    /// One string per prompt, in order.
    pub prompts: &'static [&'static str],
    /// The candidates each declared prompt completes over, given the args so far; a handler's
    /// own questions pass their choices instead.
    pub completion: Option<fn(&Core, &Args) -> Vec<String>>,
    /// The prompt at this index completes file names, starting at the current buffer's
    /// directory, from listings the app reads with `FileOp::ListDir`.
    pub files: Option<usize>,
    /// The file-name prompt lists directories only, the typed one first as `./`, and `RET`
    /// returns the selected one instead of entering it.
    pub directories: bool,
    /// Text shown after a candidate in the list and not matched, worked out as the prompt opens.
    pub annotation: Option<fn(&Core, &str) -> Option<String>>,
    /// What to show for the selected candidate in the window the prompt was opened from.
    pub preview: Option<fn(&Core, &str) -> Option<Preview>>,
    /// Matches the candidates itself in place of the fuzzy matcher.
    pub filter: Option<Filter>,
    /// The index of the candidate selected when the list opens, ivy's `:preselect`.
    pub preselect: Option<fn(&Core, &Args) -> usize>,
}

/// What a preview shows for a candidate.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    Buffer(BufferId),
    /// A file, shown in the buffer visiting it, else read into a preview buffer.
    File(PathBuf),
    /// A char position in the buffer of the window the minibuffer was opened from.
    Point(usize),
    /// A file, as `File`, at a 0-based line and char column.
    FileAt(PathBuf, usize, usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// The prefix argument, `None` when none was given.
    pub prefix: Option<Prefix>,
    /// The region, lower end first; `Ctx::call` fills it from point and mark when the spec asks.
    pub region: Option<Range<usize>>,
    pub strings: Vec<String>,
    pub project: Option<crate::project::ProjectId>,
}

impl Args {
    /// The prefix argument's value, 1 without one, like Emacs's `prefix-numeric-value`.
    pub fn count(&self) -> i64 {
        self.prefix.map_or(1, Prefix::value)
    }
}

/// A prefix argument in Emacs's raw form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefix {
    /// `C-u` pressed with no digits, holding 4 to the power of the presses.
    Universal(i64),
    /// A lone minus, worth -1.
    Minus,
    Number(i64),
}

impl Prefix {
    pub fn value(self) -> i64 {
        match self {
            Prefix::Universal(n) | Prefix::Number(n) => n,
            Prefix::Minus => -1,
        }
    }
}
