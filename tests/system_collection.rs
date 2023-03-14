use common::realign;

mod common;

#[test]
fn collection() {
    let mut sys = make_system!((), i64, ());
    sys.register_effect("emit", |_, [v]| v.int()).unwrap();
    let sys = sys.load_from_str(&realign("
        action: test-action $value
          effects:
            emit $value
        node: test
          complete:
            test-action! 1
            test-action! 2
            test-action! wrong
            test-action! 3
    ")).unwrap();
    let ctx = sys.context(&());
    let mut actions = Vec::new();
    assert!(ctx.collect_into(&mut actions, "test", &[]).unwrap().is_success());
    assert_eq!(actions.len(), 3);
    assert_eq!(&actions[0].effects, &[1]);
    assert_eq!(&actions[1].effects, &[2]);
    assert_eq!(&actions[2].effects, &[3]);
}