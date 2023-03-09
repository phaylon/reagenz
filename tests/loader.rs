use common::realign;
use reagenz::World;
use reagenz::system::{System, SymbolKind};


mod common;

struct Test;

impl World for Test {
    type State = ();
    type Effect = ();
    type Value = ();
}

#[test]
fn symbol_loading() {
    let sys = System::<Test>::default().load_from_str(&realign("
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