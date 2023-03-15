use common::realign;
use reagenz::value::Value;

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
            test-action 1
            test-action 2
            test-action wrong
            test-action 3
    ")).unwrap();
    let ctx = sys.context(&());
    let mut actions = Vec::new();
    assert!(ctx.collect_into(&mut actions, "test", &[]).unwrap().is_success());
    assert_eq!(actions.len(), 3);
    assert_eq!(&actions[0].effects, &[1]);
    assert_eq!(&actions[1].effects, &[2]);
    assert_eq!(&actions[2].effects, &[3]);
}

#[test]
fn discovery() {
    let mut sys = make_system!((), i64, ());
    sys.register_effect("emit", |_, [v]| v.int()).unwrap();
    sys.register_query("nums", |_, []| {
        Box::new([1, 2, 3].into_iter().map(Value::from))
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        action: test $value
          discover:
            for complete $n: nums
              test $n
          effects:
            emit $value
    ")).unwrap();

    let ctx = sys.context(&());
    let mut actions = Vec::new();
    ctx.discover_all(&mut actions);

    assert_eq!(actions.len(), 3);
    assert!(actions.iter().any(|a| &a.effects == &[1]));
    assert!(actions.iter().any(|a| &a.effects == &[2]));
    assert!(actions.iter().any(|a| &a.effects == &[3]));
    assert!(actions.iter().all(|a| &a.name == "test"));
    assert!(actions.iter().any(|a| &a.signature.as_slice() == &[Value::Int(1)]));
    assert!(actions.iter().any(|a| &a.signature.as_slice() == &[Value::Int(2)]));
    assert!(actions.iter().any(|a| &a.signature.as_slice() == &[Value::Int(3)]));
}