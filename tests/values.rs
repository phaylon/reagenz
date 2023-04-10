use std::sync::Arc;

use ordered_float::OrderedFloat;
use reagenz::{Value, ExtValue, IntoValues, TryFromValues};
use smol_str::SmolStr;


#[derive(Debug, Clone, PartialEq)]
struct TestEntity(u8);

type TestValue = Value<TestEntity>;

#[test]
fn into_value() {
    use Value::*;

    assert_eq!(TestValue::from(23), Int(23));
    assert_eq!(TestValue::from(23i32), Int(23));

    assert_eq!(TestValue::from(0.0), Float(OrderedFloat(0.0)));
    assert_eq!(TestValue::from(0.0f32), Float(OrderedFloat(0.0)));

    assert_eq!(TestValue::from("abc"), Symbol("abc".into()));
    assert_eq!(TestValue::from(SmolStr::from("abc")), Symbol("abc".into()));
    assert_eq!(TestValue::from(&SmolStr::from("abc")), Symbol("abc".into()));

    assert_eq!(TestValue::from(ExtValue(TestEntity(23))), Ext(TestEntity(23)));

    assert_eq!(TestValue::from(Vec::from([2, 3, 4])), List(Arc::new([Int(2), Int(3), Int(4)])));
    assert_eq!(TestValue::from([2, 3, 4]), List(Arc::new([Int(2), Int(3), Int(4)])));
}

#[test]
fn into_values() {
    use Value::*;

    assert_eq!(().into_values::<Vec<_>>(), Vec::<TestValue>::new());
    assert_eq!(
        (23, "abc").into_values::<Vec<_>>(),
        Vec::<TestValue>::from([Int(23), Symbol("abc".into())])
    );
    assert_eq!(
        [2, 3, 4].into_values::<Vec<_>>(),
        Vec::<TestValue>::from([Int(2), Int(3), Int(4)])
    );
}

#[test]
fn try_from_values() {
    use Value::*;

    assert_eq!(
        <()>::try_from_values({ const NONE: [TestValue; 0] = []; NONE }),
        Some(())
    );
    assert_eq!(
        <(i32, SmolStr)>::try_from_values([TestValue::Int(23), Symbol("abc".into())]),
        Some((23, "abc".into()))
    );
    assert_eq!(
        <[i32; 3]>::try_from_values([TestValue::Int(2), Int(3), Int(4)]),
        Some([2, 3, 4])
    );

    assert_eq!(
        <()>::try_from_values([TestValue::Int(23)]),
        None
    );
    assert_eq!(
        <(i32, i32)>::try_from_values([TestValue::Int(23), Symbol("abc".into())]),
        None
    );
    assert_eq!(
        <(i32, SmolStr)>::try_from_values([TestValue::Int(23), Symbol("abc".into()), Int(42)]),
        None
    );
}