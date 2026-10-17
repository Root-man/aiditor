//! Headless core driven by key scripts, for tests.

use std::path::PathBuf;

use tempfile::TempDir;

use crate::{Core, Msg, key::parse_keys};

pub struct Harness {
    pub core: Core,
    pub dir: TempDir,
}

impl Harness {
    /// A core plus a temp dir holding `files` as (relative path, contents).
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        Harness {
            core: Core::default(),
            dir,
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Feeds a `kbd`-style script such as `"hello C-x C-s"`.
    pub fn keys(&mut self, script: &str) -> &mut Self {
        for key in parse_keys(script).unwrap() {
            self.core.handle(Msg::Key(key));
        }
        self
    }

    pub fn text(&self) -> String {
        self.core.text()
    }

    pub fn point(&self) -> usize {
        self.core.point()
    }
}

mod tests {
    use super::*;

    #[test]
    fn types_into_a_buffer() {
        let mut h = Harness::new(&[("src/a.txt", "on disk")]);
        assert_eq!(
            std::fs::read_to_string(h.path("src/a.txt")).unwrap(),
            "on disk"
        );
        h.keys("hello RET wörld C-x C-s");
        assert_eq!(h.text(), "hello\nwörld");
        assert_eq!(h.point(), 11);
        assert!(h.core.current().modified());
        assert_eq!(h.core.current().name, "*scratch*");
        assert!(h.core.current().file.is_none());
    }
}
