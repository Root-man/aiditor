use std::collections::VecDeque;
use std::time::Instant;

use aiditor_core::key::{Key, KeyCode, parse_keys};
use aiditor_core::{Core, Effect, FileOp, GrepOp, Msg, WatchSet};
use aiditor_services::{files, grep};

#[derive(Default)]
pub struct Driver {
    pub core: Core,
    pub replies: VecDeque<Msg>,
    pub watched: WatchSet,
    pub hold_files: bool,
    pub files: VecDeque<FileOp>,
    pub quit: bool,
}

impl Driver {
    pub fn send(&mut self, msg: Msg) {
        let effects = self.core.handle(Instant::now(), msg);
        self.run(effects);
    }

    pub fn call(&mut self, command: &str, strings: &[&str]) {
        let mut ctx = aiditor_core::Ctx {
            core: &mut self.core,
            now: Instant::now(),
            effects: Vec::new(),
        };
        ctx.call(
            command,
            aiditor_core::command::Args {
                strings: strings.iter().map(|s| (*s).into()).collect(),
                ..Default::default()
            },
        );
        let effects = ctx.effects;
        self.run(effects);
        self.settle();
    }

    fn run(&mut self, effects: aiditor_core::Effects) {
        for effect in effects {
            match effect {
                Effect::Terminal(_) => {}
                Effect::File(op) if self.hold_files => self.files.push_back(op),
                Effect::File(op) => self.replies.push_back(files::run(op)),
                Effect::Grep(GrepOp::Search(op)) => {
                    grep::search(&op, || false, |msg| self.replies.push_back(msg));
                }
                Effect::Grep(GrepOp::Cancel) => {}
                Effect::Watch(set) => self.watched = set,
                Effect::Recovery(_) | Effect::SetClipboard(_) => {}
                Effect::Quit => self.quit = true,
                Effect::LoadConfiguration { .. } => panic!("unexpected effect in service test"),
            }
        }
    }

    pub fn settle(&mut self) {
        while let Some(reply) = self.replies.pop_front() {
            self.send(reply);
        }
    }

    pub fn keys(&mut self, script: &str) {
        for key in parse_keys(script).unwrap() {
            self.send(match key.code {
                KeyCode::Char(c) if key == Key::plain(key.code) => Msg::Insert {
                    text: c.into(),
                    replace: None,
                },
                _ => Msg::Key(key),
            });
            self.settle();
        }
    }
}
