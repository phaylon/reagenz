use assert_matches::assert_matches;
use reagenz::system::{SystemGlobalError};
use common::realign;
use reagenz::value::Value;


mod common;

#[test]
fn globals() {
    let mut sys = make_system!(i64, i64, ());
    sys.register_global("$X", |ctx| ctx.state().clone().into()).unwrap();
    sys.register_effect("emit", |_, [v]| v.int()).unwrap();
    let sys = sys.load_from_str(&realign("
        action: test
          effects:
            emit $X
    ")).unwrap();

    assert_matches!(sys.context(&23).run("test", &[]).unwrap().effects().unwrap(), &[23]);
    assert_matches!(sys.context(&23).global("$X"), Some(Value::Int(23)));

    let globals = sys.globals().map(|s| s.as_str()).collect::<Vec<_>>();
    assert_eq!(&globals, &["$X"]);

    let mut sys = sys;
    assert_eq!(
        sys.register_global("$X", |_| panic!("test global")),
        Err(SystemGlobalError::Conflict)
    );
    assert_eq!(
        sys.register_global("X", |_| panic!("test global")),
        Err(SystemGlobalError::Invalid)
    );
}