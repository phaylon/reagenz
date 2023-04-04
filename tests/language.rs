use reagenz::{BehaviorTreeBuilder, Outcome, effect_fn, cond_fn, query_fn};
use treelang::{normalize_source, Indent};
use assert_matches::assert_matches;


const INDENT: Indent = Indent::spaces(2);

fn normalize(source: &str) -> String {
    normalize_source('|', source).unwrap()
}

#[test]
fn globals() {
    let mut tree = BehaviorTreeBuilder::<i32, (), i32>::default();
    tree.register_effect("emit-value", 1, effect_fn!(_, value: i32 => Some(value)));
    tree.register_global("$global", |ctx| (*ctx).into());
    let tree = tree.compile_str(INDENT, "test", &normalize("
        |action: test
        |  effects:
        |    emit-value $global
    ")).unwrap();
    assert_matches!(tree.evaluate(&23, "test", ()), Ok(Outcome::Action(action)) => {
        assert_matches!(action.effects(), [23]);
    });
}

#[test]
fn effects() {
    let mut tree = BehaviorTreeBuilder::<i32, (), i32>::default();
    tree.register_effect("emit-value", 1, effect_fn!(ctx, value: i32 => {
        (*ctx != value).then_some(*ctx + value)
    }));
    let tree = tree.compile_str(INDENT, "test", &normalize("
        |action: test $value
        |  effects:
        |    emit-value $value
    ")).unwrap();
    assert_matches!(tree.evaluate(&23, "test", [42]), Ok(Outcome::Action(action)) => {
        assert_matches!(action.effects(), [65]);
    });
    assert_matches!(tree.evaluate(&23, "test", [23]), Ok(Outcome::Failure));
}

#[test]
fn conditions() {
    let mut tree = BehaviorTreeBuilder::<(), (), ()>::default();
    tree.register_condition("test", 1, cond_fn!(_, value: i32 => value == 23));
    let tree = tree.compile_str(INDENT, "test", "").unwrap();
    assert_eq!(tree.evaluate(&(), "test", [23]), Ok(Outcome::Success));
    assert_eq!(tree.evaluate(&(), "test", [42]), Ok(Outcome::Failure));
}

#[test]
fn queries() {
    let mut tree = BehaviorTreeBuilder::<&[i32], (), ()>::default();
    tree.register_condition("check", 1, cond_fn!(_, value: i32 => value != 0));
    tree.register_query("values", 0, query_fn!(ctx => ctx.iter().copied().map(Into::into)));
    let tree = tree.compile_str(INDENT, "test", &normalize("
        |node: test-every
        |  for-every $value: values
        |    check $value
        |node: test-any
        |  for-any $value: values
        |    check $value
        |node: test-visit
        |  visit-every $value: values
        |    check $value
        |node: test-first
        |  with-first $value: values
        |    check $value
        |node: test-last
        |  with-last $value: values
        |    check $value
    ")).unwrap();
    let eval = |name, values| tree.evaluate(&values, name, ()).map(|o| o.is_success());

    assert!(eval("test-every", &[1, 1, 1]).unwrap());
    assert!(! eval("test-every", &[1, 0, 1]).unwrap());

    assert!(eval("test-any", &[0, 1, 0]).unwrap());
    assert!(! eval("test-any", &[0, 0, 0]).unwrap());

    assert!(eval("test-visit", &[0, 0, 0]).unwrap());

    assert!(eval("test-first", &[1, 0, 0]).unwrap());
    assert!(! eval("test-first", &[0, 1, 1]).unwrap());

    assert!(eval("test-last", &[0, 0, 1]).unwrap());
    assert!(! eval("test-last", &[1, 1, 0]).unwrap());
}

#[test]
fn patterns() {
    let mut tree = BehaviorTreeBuilder::<&[[i32; 2]], (), (i32, i32)>::default();
    tree.register_global("$global", |_| 123.into());
    tree.register_effect("emit-value", 2, effect_fn!(_, a: i32, b: i32 => Some((a, b))));
    tree.register_query("values", 0, query_fn!(ctx => ctx.iter().copied().map(Into::into)));
    let tree = tree.compile_str(INDENT, "test", &normalize("
        |action: emit $a $b
        |  effects:
        |    emit-value $a $b
        |node: test-for-every
        |  for-every [$a $b]: values
        |    emit $a $b
        |node: test-for-any
        |  for-any [$a $b]: values
        |    emit $a $b
        |node: test-visit-every
        |  visit-every [$a $b]: values
        |    emit $a $b
        |node: test-with-first
        |  with-first [$a $b]: values
        |    emit $a $b
        |node: test-with-last
        |  with-last [$a $b]: values
        |    emit $a $b
        |node: test-match
        |  with-first $value: values
        |    match [$a $b]: $value
        |      emit $a $b
        |node: test-match-symbol $value
        |  match abc: $value
        |node: test-match-int $value
        |  match 23: $value
        |node: test-match-multi $value
        |  match [$x $x]: $value
        |node: test-match-global $value
        |  match $global: $value
    ")).unwrap();

    assert_matches!(
        tree.evaluate(&&[[2, 3]][..], "test-for-every", ()),
        Ok(Outcome::Action(action)) => {
            assert_matches!(action.effects(), [(2, 3)]);
        }
    );
    assert_matches!(
        tree.evaluate(&&[[2, 3]][..], "test-for-any", ()),
        Ok(Outcome::Action(action)) => {
            assert_matches!(action.effects(), [(2, 3)]);
        }
    );
    assert_matches!(
        tree.evaluate(&&[[2, 3]][..], "test-visit-every", ()),
        Ok(Outcome::Success)
    );
    assert_matches!(
        tree.evaluate(&&[[2, 3]][..], "test-with-first", ()),
        Ok(Outcome::Action(action)) => {
            assert_matches!(action.effects(), [(2, 3)]);
        }
    );
    assert_matches!(
        tree.evaluate(&&[[2, 3]][..], "test-with-last", ()),
        Ok(Outcome::Action(action)) => {
            assert_matches!(action.effects(), [(2, 3)]);
        }
    );
    assert_matches!(
        tree.evaluate(&&[[2, 3]][..], "test-match", ()),
        Ok(Outcome::Action(action)) => {
            assert_matches!(action.effects(), [(2, 3)]);
        }
    );

    assert_matches!(
        tree.evaluate(&&[][..], "test-match-symbol", ("abc",)),
        Ok(Outcome::Success)
    );
    assert_matches!(
        tree.evaluate(&&[][..], "test-match-symbol", ("xyz",)),
        Ok(Outcome::Failure)
    );

    assert_matches!(
        tree.evaluate(&&[][..], "test-match-int", (23,)),
        Ok(Outcome::Success)
    );
    assert_matches!(
        tree.evaluate(&&[][..], "test-match-int", (42,)),
        Ok(Outcome::Failure)
    );

    assert_matches!(
        tree.evaluate(&&[][..], "test-match-global", (123,)),
        Ok(Outcome::Success)
    );
    assert_matches!(
        tree.evaluate(&&[][..], "test-match-global", (142,)),
        Ok(Outcome::Failure)
    );

    assert_matches!(
        tree.evaluate(&&[][..], "test-match-multi", ([23, 23],)),
        Ok(Outcome::Success)
    );
    assert_matches!(
        tree.evaluate(&&[][..], "test-match-multi", ([23, 42],)),
        Ok(Outcome::Failure)
    );
}