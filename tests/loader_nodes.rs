use assert_matches::assert_matches;
use common::realign;
use reagenz::World;
use reagenz::system::{System, Outcome, Action, Context};
use reagenz::value::Value;


mod common;

struct Test;

impl World for Test {
    type State = i64;
    type Effect = i64;
    type Value = ();
}

#[test]
fn sequence_nodes() {
    let mut sys = System::<Test>::default();
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
        Context::new(&23, &sys).run("test", &[0.into(), 23.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        Context::new(&23, &sys).run("test", &[23.into(), 0.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        Context::new(&23, &sys).run("test", &[0.into(), 0.into()]).unwrap(),
        Outcome::Failure
    );
    assert_matches!(
        Context::new(&23, &sys).run("test", &[23.into(), 23.into()]).unwrap(),
        Outcome::Success
    );
}

#[test]
fn selection_nodes() {
    let mut sys = System::<Test>::default();
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
        Context::new(&23, &sys).run("test", &[23.into(), 42.into()]).unwrap().effects(),
        Some(&[1])
    );
    assert_matches!(
        Context::new(&42, &sys).run("test", &[23.into(), 42.into()]).unwrap().effects(),
        Some(&[2])
    );
    assert_matches!(
        Context::new(&0, &sys).run("test", &[23.into(), 42.into()]).unwrap(),
        Outcome::Failure
    );
}

#[test]
fn query_nodes_any() {
    let mut sys = System::<Test>::default();
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
    let mut sys = System::<Test>::default();
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
    let mut sys = System::<Test>::default();
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
    let mut sys = System::<Test>::default();
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
