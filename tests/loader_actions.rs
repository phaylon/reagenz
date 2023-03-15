use assert_matches::assert_matches;
use reagenz::system::{Outcome};
use common::realign;


mod common;

#[derive(Debug)]
enum Effect { A(i64), B(i64) }

#[test]
fn action_nodes() {
    let mut sys = make_system!((), Effect, ());
    sys.register_effect("emit-a", |_ctx, [v]| Some(Effect::A(v.int().unwrap()))).unwrap();
    sys.register_effect("emit-b", |_ctx, [v]| Some(Effect::B(v.int().unwrap()))).unwrap();
    sys.register_node("<", |_ctx, [a, b]| (a < b).into()).unwrap();
    let sys = sys.load_from_str(&realign("
        action: test $a $b
          required:
            < $a $b
          effects:
            emit-a $a
            emit-b $b
    ")).unwrap();
    let ctx = sys.context(&());
    assert_matches!(
        ctx.run("test", &[23.into(), 42.into()]).unwrap().effects().unwrap(),
        &[Effect::A(23), Effect::B(42)]
    );
    assert_matches!(
        ctx.run("test", &[42.into(), 23.into()]).unwrap(),
        Outcome::Failure
    );
}