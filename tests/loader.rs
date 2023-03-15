use common::realign;
use reagenz::system::SymbolKind;
use reagenz::value::Value;


mod common;

#[test]
fn symbol_loading() {
    let sys = make_system!((), (), ()).load_from_str(&realign("
        action: test-action $a $b
        node: test-node $a $b
    ")).unwrap();

    let a = sys.symbol("test-action").unwrap();
    assert_eq!(a.arity, 2);
    assert_eq!(a.kind, SymbolKind::Action);

    let n = sys.symbol("test-node").unwrap();
    assert_eq!(n.arity, 2);
    assert_eq!(n.kind, SymbolKind::Node);
}

#[test]
fn values() {
    let mut sys = make_system!((), Value<Self>, ());
    sys.register_effect("emit", |_, [value]| Some(value.clone())).unwrap();
    let sys = sys.load_from_str(&realign("
        action: value $v
          effects:
            emit $v
        node: test-int
          value 23
        node: test-float
          value 0.0
        node: test-symbol
          value abc
        node: test-var $v
          value $v
        node: test-list $v
          value [23 0.0 abc $v]
    ")).unwrap();
    let ctx = sys.context(&());
    assert_eq!(
        ctx.run("test-int", &[]).unwrap().effects(),
        Some(&[Value::Int(23)][..])
    );
    assert_eq!(
        ctx.run("test-float", &[]).unwrap().effects(),
        Some(&[Value::Float(0.0)][..])
    );
    assert_eq!(
        ctx.run("test-symbol", &[]).unwrap().effects(),
        Some(&[Value::Symbol("abc".into())][..])
    );
    assert_eq!(
        ctx.run("test-var", &[23.into()]).unwrap().effects(),
        Some(&[Value::Int(23)][..])
    );
    assert_eq!(
        ctx.run("test-list", &[23.into()]).unwrap().effects(),
        Some(&[Value::from_iter([
            Value::Int(23),
            Value::Float(0.0),
            Value::Symbol("abc".into()),
            Value::Int(23),
        ])][..])
    );
}