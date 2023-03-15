use assert_matches::assert_matches;
use common::realign;
use reagenz::system::{Outcome, Action};
use reagenz::value::{Value, Args};


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
            name: "emit".into(),
            signature: Args::from_iter([val.clone()]),
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
fn none_nodes() {
    let mut sys = make_system!((), (), ());
    sys.register_node("<", |_ctx, [a, b]| (a < b).into()).unwrap();
    sys.register_node(">", |_ctx, [a, b]| (a > b).into()).unwrap();
    let sys = sys.load_from_str(&realign("
        node: test $v
          none:
            >? $v 5
            <? $v -5
    ")).unwrap();
    assert!(sys.context(&()).run("test", &[Value::Int(-10)]).unwrap().is_failure());
    assert!(sys.context(&()).run("test", &[Value::Int(0)]).unwrap().is_success());
    assert!(sys.context(&()).run("test", &[Value::Int(10)]).unwrap().is_failure());
}

#[test]
fn match_nodes() {
    use Value::*;
    let mut sys = make_system!((), i64, ());
    sys.register_effect("emit", |_, [v]| v.int()).unwrap();
    let sys = sys.load_from_str(&realign("
        action: output $v
          effects:
            emit $v
        node: test $value $lex
          match $value: abc $v? $lex $v
            output! $v
    ")).unwrap();
    let ctx = sys.context(&());

    assert_matches!(
        ctx.run("test", &[
            [Symbol("abc".into()), Int(23), Int(42), Int(23)].into(),
            42.into(),
        ]).unwrap().effects().unwrap(),
        &[23]
    );

    assert!(ctx.run("test", &[
        [Symbol("abc".into()), Int(23), Int(42), Int(0)].into(),
        42.into(),
    ]).unwrap().is_failure());
    assert!(ctx.run("test", &[
        [Symbol("abc".into()), Int(23), Int(0), Int(23)].into(),
        42.into(),
    ]).unwrap().is_failure());
    assert!(ctx.run("test", &[
        [Symbol("def".into()), Int(23), Int(42), Int(23)].into(),
        42.into(),
    ]).unwrap().is_failure());
    assert!(ctx.run("test", &[
        [Symbol("abc".into()), Int(23), Int(42), Int(23), Int(0)].into(),
        42.into(),
    ]).unwrap().is_failure());
}

#[test]
fn query_nodes_any() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_node("=", |_ctx, [a, b]| (a == b).into()).unwrap();
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
    sys.register_node("<", |_ctx, [a, b]| (a < b).into()).unwrap();
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
    sys.register_node("=", |_ctx, [a, b]| (a == b).into()).unwrap();
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
    sys.register_node("=", |_ctx, [a, b]| (a == b).into()).unwrap();
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
    sys.register_node("<=", |_ctx, [a, b]| (a <= b).into()).unwrap();
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
            Outcome::Action(Action {
                name: "fallback".into(),
                signature: Args::new(),
                effects: Vec::from([100 + arguments[0].int().unwrap()]),
            })
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
    assert_eq!(ctx.run("test", &[1.into()]).unwrap().effects().unwrap(), &[1]);
    assert_eq!(ctx.run("test", &[2.into()]).unwrap().effects().unwrap(), &[2]);
    assert_eq!(ctx.run("test", &[3.into()]).unwrap().effects().unwrap(), &[3]);
    assert_eq!(ctx.run("test", &[0.into()]).unwrap().effects().unwrap(), &[100]);
}