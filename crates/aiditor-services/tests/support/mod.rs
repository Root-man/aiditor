use std::collections::VecDeque;
use std::time::Instant;

use aiditor_core::key::{Key, KeyCode, parse_keys};
use aiditor_core::{Core, Effect, GrepOp, Msg, WatchSet};
use aiditor_services::{files, grep};

#[derive(Default)]
pub struct Driver {
    pub core: Core,
    pub replies: VecDeque<Msg>,
    pub watched: WatchSet,
}

impl Driver {
    pub fn send(&mut self, msg: Msg) {
        for effect in self.core.handle(Instant::now(), msg) {
            match effect {
                Effect::File(op) => self.replies.push_back(files::run(op)),
                Effect::Grep(GrepOp::Search(op)) => {
                    grep::search(&op, || false, |msg| self.replies.push_back(msg));
                }
                Effect::Grep(GrepOp::Cancel) => {}
                Effect::Watch(set) => self.watched = set,
                Effect::Recovery(_) | Effect::SetClipboard(_) => {}
                Effect::LoadSettings | Effect::Quit => panic!("unexpected effect in service test"),
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
