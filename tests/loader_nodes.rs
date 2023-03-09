use assert_matches::assert_matches;
use common::realign;
use reagenz::World;
use reagenz::system::{System, Outcome, Action, Context};


mod common;

struct Test;

impl World for Test {
    type State = i64;
    type Effect = i64;
    type Value = ();
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