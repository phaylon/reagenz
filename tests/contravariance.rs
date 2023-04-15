use reagenz::{BehaviorTree, BehaviorTreeBuilder, effect_fn, query_fn, Outcome};
use src_ctx::normalize;
use treelang::{Indent};


type Tree<'a> = BehaviorTree<Context<'a>, (), i32>;

const INDENT: Indent = Indent::spaces(2);

struct Context<'a> {
    values: &'a [i32],
}

fn make_tree<'a>() -> Tree<'a> {
    let mut tree: BehaviorTreeBuilder<Context<'_>, (), i32> = BehaviorTreeBuilder::default();
    tree.register_effect("emit-value", effect_fn!(_, v: i32 => Some(v)));
    tree.register_query("values", query_fn!(ctx => ctx.values.iter().copied().map(Into::into)));
    tree.compile_str(INDENT, "test", &normalize("
        |action: emit $v
        |  effects:
        |    emit-value $v
        |node: test
        |  with-first $v: values
        |    emit $v
    ")).unwrap()
}

fn eval<'tree, 'ctx: 'tree>(tree: &Tree<'tree>, ctx: &Context<'ctx>) -> Outcome<(), i32> {
    tree.evaluate(ctx, "test", ()).unwrap()
}

fn erase<R>(tree: &Tree<'_>, ctx: &Context<'_>, scope: impl FnOnce(&Tree<'_>, &Context<'_>) -> R) -> R {
    scope(tree, ctx)
}


#[test]
fn contravariant_context() {
    /*
    let orig_tree = make_tree();

    for _ in 0..3 {
        let tree = orig_tree.clone();
        let values = Vec::from([3, 4, 5]);
        let ctx = Context { values: &values };
        //eval(&tree, &ctx);
        //erase(&tree, &ctx, |tree, ctx| {
        tree.evaluate(&ctx, "test", ()).unwrap();
        //});
    };
    */
}
