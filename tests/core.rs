use std::sync::Arc;

use reagenz::World;
use reagenz::system::System;
use reagenz::value::Value;


mod common;

struct Test;

impl World for Test {
    type State<'a> = &'a ();
    type Effect = ();
    type Value = ();
}

#[test]
fn types() {
    use Value::*;
    let sys = System::<Test>::core();
    let ctx = sys.context(&());

    assert!(ctx.run("is-symbol", &[Symbol("abc".into())]).unwrap().is_success());
    assert!(ctx.run("is-symbol", &[Int(23)]).unwrap().is_failure());

    assert!(ctx.run("is-int", &[Int(23)]).unwrap().is_success());
    assert!(ctx.run("is-int", &[Float(23.0)]).unwrap().is_failure());

    assert!(ctx.run("is-float", &[Float(23.5)]).unwrap().is_success());
    assert!(ctx.run("is-float", &[Int(23)]).unwrap().is_failure());

    assert!(ctx.run("is-external", &[Ext(())]).unwrap().is_success());
    assert!(ctx.run("is-external", &[Int(23)]).unwrap().is_failure());

    assert!(ctx.run("is-list", &[List(Arc::from([]))]).unwrap().is_success());
    assert!(ctx.run("is-list", &[Int(23)]).unwrap().is_failure());
}

#[test]
fn symbols() {
    use Value::*;
    let sys = System::<Test>::core();
    let ctx = sys.context(&());

    assert!(ctx.run("symbols=", &[
        Symbol("a".into()),
        Symbol("a".into()),
    ]).unwrap().is_success());
    assert!(ctx.run("symbols=", &[
        Symbol("a".into()),
        Symbol("wrong".into()),
    ]).unwrap().is_failure());

    assert!(ctx.run("symbol-in-list", &[
        Symbol("a".into()),
        List(Arc::from([Symbol("a".into())])),
    ]).unwrap().is_success());
    assert!(ctx.run("symbol-in-list", &[
        List(Arc::from([Symbol("a".into())])),
        Symbol("a".into()),
    ]).unwrap().is_success());

    assert!(ctx.run("symbol-in-list", &[
        Symbol("a".into()),
        List(Arc::from([Symbol("wrong".into())])),
    ]).unwrap().is_failure());
    assert!(ctx.run("symbol-in-list", &[
        List(Arc::from([Symbol("wrong".into())])),
        Symbol("a".into()),
    ]).unwrap().is_failure());
}

#[test]
fn lists() {
    use Value::*;
    let sys = System::<Test>::core();
    let ctx = sys.context(&());

    let values = ctx.query("list-items", &[
        List(Arc::from([Int(2), Int(3), Int(4)])),
    ]).unwrap().collect::<Vec<_>>();
    assert_eq!(values, Vec::from([Int(2), Int(3), Int(4)]));
}
