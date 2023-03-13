use assert_matches::assert_matches;
use reagenz::system::{SymbolKind, SystemSymbolError, SymbolInfo};


mod common;

#[test]
fn node_symbols() {
    let mut sys = make_system!((), (), ());
    sys.register_node("test", |_ctx, [_, _]| panic!("test node")).unwrap();
    let info = sys.symbol("test").unwrap();
    assert_eq!(info.arity, 2);
    assert_eq!(info.kind, SymbolKind::Node);
}

#[test]
fn effect_symbols() {
    let mut sys = make_system!((), (), ());
    sys.register_effect("test", |_ctx, [_, _]| panic!("test effect")).unwrap();
    let info = sys.symbol("test").unwrap();
    assert_eq!(info.arity, 2);
    assert_eq!(info.kind, SymbolKind::Effect);
}

#[test]
fn query_symbols() {
    let mut sys = make_system!((), (), ());
    sys.register_query("test", |_ctx, [_, _]| panic!("test query")).unwrap();
    let info = sys.symbol("test").unwrap();
    assert_eq!(info.arity, 2);
    assert_eq!(info.kind, SymbolKind::Query);
}

#[test]
fn getter_symbols() {
    let mut sys = make_system!((), (), ());
    sys.register_getter("test", |_ctx, [_, _]| panic!("test getter")).unwrap();
    let info = sys.symbol("test").unwrap();
    assert_eq!(info.arity, 2);
    assert_eq!(info.kind, SymbolKind::Getter);
}

#[test]
fn multiple() {
    let mut sys = make_system!((), (), ());
    sys.register_node("a", |_ctx, [_, _]| panic!("test node a")).unwrap();
    sys.register_node("b", |_ctx, [_, _]| panic!("test node b")).unwrap();
    sys.register_node("c", |_ctx, [_, _]| panic!("test node c")).unwrap();

    let mut symbols = sys.symbols().map(|s| s.as_str()).collect::<Vec<_>>();
    symbols.sort();
    assert_eq!(&symbols, &["a", "b", "c"]);

    assert_matches!(
        sys.register_query("a", |_ctx, []| panic!("conflict node")),
        Err(SystemSymbolError::Conflict {
            previous: SymbolInfo { kind: SymbolKind::Node, .. },
            current: SymbolInfo { kind: SymbolKind::Query, .. },
        })
    );

    assert_matches!(
        sys.register_query(" ", |_ctx, []| panic!("invalid name node")),
        Err(SystemSymbolError::Invalid)
    );
}