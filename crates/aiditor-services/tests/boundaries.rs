mod support;

use std::fs;

use aiditor_core::Msg;
use aiditor_core::buffer::hash;
use aiditor_core::key::parse_keys;
use aiditor_services::projects;
use support::Driver;

#[test]
fn production_visit_save_and_index_replies_can_be_held() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.txt"), "original").unwrap();
    let loaded = projects::write(
        &dir.path().join("p.toml"),
        None,
        format!("name = 'p'\n[primary]\npath = {root:?}\n"),
    )
    .unwrap();
    let mut d = Driver::default();
    d.send(Msg::ProjectsLoaded {
        dir: None,
        projects: vec![Ok(loaded)],
    });
    d.core.launch(&root, &root);
    d.send(Msg::Tick);
    assert!(
        d.replies
            .iter()
            .any(|msg| matches!(msg, Msg::Indexed { .. }))
    );
    d.settle();
    assert_eq!(d.watched.roots, vec![(root.clone(), vec![])]);

    d.keys(&format!("C-x C-f C-a C-k {}", root.join("a.txt").display()));
    d.send(Msg::Key(parse_keys("M-RET").unwrap()[0]));
    assert!(
        d.replies
            .iter()
            .any(|msg| matches!(msg, Msg::Opened { .. }))
    );
    d.settle();
    assert_eq!(d.core.text(), "original");
    let origin = d.core.workspace();
    d.keys("saved SPC C-x");
    d.send(Msg::Key(parse_keys("C-s").unwrap()[0]));
    assert_eq!(
        fs::read_to_string(root.join("a.txt")).unwrap(),
        "saved original\n"
    );
    assert!(d.core.current().modified());
    assert!(d.replies.iter().any(|msg| matches!(msg, Msg::Saved { .. })));
    let second = d
        .core
        .new_workspace("second", d.core.project_id("p").unwrap());
    d.settle();
    assert_eq!(d.core.workspace(), second);
    assert_eq!(d.core.text(), "");
    d.core.switch_workspace(origin).unwrap();
    assert_eq!(d.core.current().disk, Some(hash(b"saved original\n")));
    assert_eq!(d.core.text(), "saved original\n");
    assert!(!d.core.current().modified());
}
