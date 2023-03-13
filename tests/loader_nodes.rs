use assert_matches::assert_matches;
use common::realign;
use reagenz::system::{Outcome, Action};
use reagenz::value::Value;


mod common;

#[test]
fn sequence_nodes() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("is-state-value", |ctx, [val]| {
        if val.int().unwrap() == *ctx.state() { Outcome::Success }
        else { Outcome::Failure }
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $a $b
          is-state-value? $a
          is-state-value? $b
    ")).unwrap();

    assert_matches!(
        sys.context(&23).run("test", &[0.into(), 23.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        sys.context(&23).run("test", &[23.into(), 0.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        sys.context(&23).run("test", &[0.into(), 0.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        sys.context(&23).run("test", &[23.into(), 23.into()]).unwrap(),
        Outcome::Success
    );
}

#[test]
fn selection_nodes() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("emit", |_ctx, [val]| {
        Outcome::Action(Action {
            effects: Vec::from([val.int().unwrap()]),
        })
    }).unwrap();
    sys.register_node("is-state-value", |ctx, [val]| {
        if val.int().unwrap() == *ctx.state() { Outcome::Success }
        else { Outcome::Failure }
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $a $b
          select:
            do:
              is-state-value? $a
              emit! 1
            do:
              is-state-value? $b
              emit! 2
    ")).unwrap();

    assert_matches!(
        sys.context(&23).run("test", &[23.into(), 42.into()]).unwrap().effects(),
        Some(&[1])
    );
    assert_matches!(
        sys.context(&42).run("test", &[23.into(), 42.into()]).unwrap().effects(),
        Some(&[2])
    );
    assert_matches!(
        sys.context(&0).run("test", &[23.into(), 42.into()]).unwrap(),
        Outcome::Failure
    );
}

#[test]
fn query_nodes_any() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("=", |_ctx, [a, b]| {
        (a == b).into()
    }).unwrap();
    sys.register_query("nums", |_ctx, []| {
        Box::new([1, 2, 3].into_iter().map(Value::from))
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $v
          for any $n: nums
            =? $v $n
    ")).unwrap();
    assert_matches!(
        sys.context(&0).run("test", &[3.into()]).unwrap(),
        Outcome::Success
    );
    assert_matches!(
        sys.context(&0).run("test", &[23.into()]).unwrap(),
        Outcome::Failure
    );
}

#[test]
fn query_nodes_all() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("<", |_ctx, [a, b]| {
        (a.int() < b.int()).into()
    }).unwrap();
    sys.register_query("nums", |_ctx, []| {
        Box::new([1, 2, 3].into_iter().map(Value::from))
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $v
          for all $n: nums
            <? $v $n
    ")).unwrap();
    assert_matches!(
        sys.context(&0).run("test", &[0.into()]).unwrap(),
        Outcome::Success
    );
    assert_matches!(
        sys.context(&0).run("test", &[1.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        sys.context(&0).run("test", &[23.into()]).unwrap(),
        Outcome::Failure
    );
}

#[test]
fn query_nodes_first() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("=", |_ctx, [a, b]| {
        (a.int() == b.int()).into()
    }).unwrap();
    sys.register_query("nums", |_ctx, []| {
        Box::new([1, 2, 3].into_iter().map(Value::from))
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $v
          for first $n: nums
            =? $v $n
    ")).unwrap();
    assert_matches!(
        sys.context(&0).run("test", &[1.into()]).unwrap(),
        Outcome::Success
    );
    assert_matches!(
        sys.context(&0).run("test", &[2.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        sys.context(&0).run("test", &[23.into()]).unwrap(),
        Outcome::Failure
    );
}

#[test]
fn query_nodes_last() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("=", |_ctx, [a, b]| {
        (a.int() == b.int()).into()
    }).unwrap();
    sys.register_query("nums", |_ctx, []| {
        Box::new([1, 2, 3].into_iter().map(Value::from))
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $v
          for last $n: nums
            =? $v $n
    ")).unwrap();
    assert_matches!(
        sys.context(&0).run("test", &[3.into()]).unwrap(),
        Outcome::Success
    );
    assert_matches!(
        sys.context(&0).run("test", &[2.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        sys.context(&0).run("test", &[23.into()]).unwrap(),
        Outcome::Failure
    );
}

#[test]
fn dispatchers() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_effect("emit", |_ctx, [v]| v.int()).unwrap();
    sys.register_node("<=", |_ctx, [a, b]| {
        (a.int() <= b.int()).into()
    }).unwrap();
    sys.register_dispatch("select-reverse", |_sys, signature| {
        assert_eq!(signature.len(), 1);
        assert_eq!(signature[0].int().unwrap(), 23);
        Some(Box::new(|ctx, arguments, branches| {
            assert_eq!(arguments.len(), 1);
            for branch in branches.into_iter().rev() {
                let result = branch.eval(ctx);
                if result.is_non_failure() {
                    return result;
                }
            }
            Outcome::from_effect(100 + arguments[0].int().unwrap())
        }))
    }).unwrap();
    let sys = sys.load_from_str(&realign("
        action: check $case $value
          required:
            <=? $case $value
          effects:
            emit $case
        node: test $value
          select-reverse 23: $value
            check! 1 $value
            check! 2 $value
            check! 3 $value
    ")).unwrap();
    let ctx = sys.context(&0);
    assert_eq!(ctx.run("test", &[1.into()]).unwrap(), Outcome::from_effect(1));
    assert_eq!(ctx.run("test", &[2.into()]).unwrap(), Outcome::from_effect(2));
    assert_eq!(ctx.run("test", &[3.into()]).unwrap(), Outcome::from_effect(3));
    assert_eq!(ctx.run("test", &[0.into()]).unwrap(), Outcome::from_effect(100));
}