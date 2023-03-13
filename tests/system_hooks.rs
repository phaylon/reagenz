use reagenz::World;
use reagenz::system::{System, Outcome};
use reagenz::value::Value;


struct Test;

impl World for Test {
    type State = i64;
    type Effect = i64;
    type Value = ();
}

#[test]
fn effect_hooks() {
    let mut sys = System::<Test>::default();
    sys.register_effect("test", |ctx, [val]| Some(*ctx.state() + val.int().unwrap())).unwrap();
    let ctx = sys.context(&23);
    assert_eq!(ctx.effect("test", &[42.into()]).unwrap(), Some(65));
}

#[test]
fn node_hooks() {
    let mut sys = System::<Test>::default();
    sys.register_node("test", |ctx, [val]| (*ctx.state() == val.int().unwrap()).into()).unwrap();
    let ctx = sys.context(&23);
    assert_eq!(ctx.run("test", &[42.into()]).unwrap(), Outcome::Failure);
    assert_eq!(ctx.run("test", &[23.into()]).unwrap(), Outcome::Success);
}

#[test]
fn query_hooks() {
    let mut sys = System::<Test>::default();
    sys.register_query("test", |ctx, [val]| {
        Box::new((*ctx.state()..val.int().unwrap()).map(Value::from))
    }).unwrap();
    let values = sys.context(&3)
        .query("test", &[6.into()]).unwrap()
        .map(|v| v.int().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(&values, &[3, 4, 5]);
}

#[test]
fn getter_hooks() {
    let mut sys = System::<Test>::default();
    sys.register_getter("test", |ctx, [val]| {
        Some((*ctx.state() + val.int().unwrap()).into())
    }).unwrap();
    let values = sys.context(&23)
        .query("test", &[42.into()]).unwrap()
        .map(|v| v.int().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(&values, &[65]);
}
