//! Named commands, the arguments they declare and the arguments they get.

use std::ops::Range;

use crate::Ctx;

pub type Handler = fn(&mut Ctx, Args) -> Result<(), String>;

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
    /// One string per prompt, in order.
    pub prompts: &'static [&'static str],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// The prefix argument, `None` when none was given.
    pub prefix: Option<i64>,
    /// The region, lower end first; `Ctx::call` fills it from point and mark when the spec asks.
    pub region: Option<Range<usize>>,
    pub strings: Vec<String>,
}
