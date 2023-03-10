use assert_matches::assert_matches;
use reagenz::World;
use reagenz::system::{System, Context, Outcome};
use common::realign;


mod common;

struct Test;

#[derive(Debug)]
enum Effect { A(i64), B(i64) }

impl World for Test {
    type State = ();
    type Effect = Effect;
    type Value = ();
}

#[test]
fn action_nodes() {
    let mut sys = System::<Test>::default();
    sys.register_effect("emit-a", |_ctx, [v]| Some(Effect::A(v.int().unwrap()))).unwrap();
    sys.register_effect("emit-b", |_ctx, [v]| Some(Effect::B(v.int().unwrap()))).unwrap();
    sys.register_node("lt", |_ctx, [a, b]| (a.int().unwrap() < b.int().unwrap()).into()).unwrap();
    let sys = sys.load_from_str(&realign("
        action: test $a $b
          required:
            lt? $a $b
          effects:
            emit-a $a
            emit-b $b
    ")).unwrap();
    let ctx = Context::new(&(), &sys);
    assert_matches!(
        ctx.run("test", &[23.into(), 42.into()]).unwrap().effects().unwrap(),
        &[Effect::A(23), Effect::B(42)]
    );
    assert_matches!(
        ctx.run("test", &[42.into(), 23.into()]).unwrap(),
        Outcome::Failure
    );
}