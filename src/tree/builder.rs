use std::sync::Arc;

use derivative::Derivative;
use smol_str::SmolStr;
use treelang::Indent;

use crate::Outcome;
use crate::str::{is_variable, is_symbol};
use crate::tree::id_space::{QueryIdx, CondIdx};
use crate::value::{Value, TryFromValues};

use super::BehaviorTree;
use super::id_space::{IdSpace, GlobalIdx, EffectIdx};
use super::script::{ScriptSource, Compiler, ScriptResult};


#[derive(Derivative)]
#[derivative(Clone(bound=""), Default(bound=""))]
pub struct BehaviorTreeBuilder<Ctx, Ext, Eff> {
    ids: IdSpace<Ctx, Ext, Eff>
}

impl<Ctx, Ext, Eff> BehaviorTreeBuilder<Ctx, Ext, Eff> {
    #[track_caller]
    pub fn register_global<N, R, F>(&mut self, id: N, handler: F)
    where
        N: Into<SmolStr>,
        F: Fn(&Ctx) -> R + 'static,
        R: Into<Value<Ext>>,
    {
        let id = id.into();
        assert!(is_variable(&id), "global id `{id}` is not a valid variable");
        let prev = self.ids.set::<GlobalIdx>(id.clone(), Arc::new(move |ctx| {
            handler(ctx).into()
        }), 0).err();
        if let Some(kind) = prev {
            panic!("global id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_effect<N, F, S>(&mut self, id: N, handler: F)
    where
        N: Into<SmolStr>,
        F: Fn(&Ctx, S) -> Option<Eff> + 'static,
        S: TryFromValues<Ext>,
        Ext: Clone,
    {
        let id = id.into();
        assert!(is_symbol(&id), "effect id `{id}` is not a valid symbol");
        let prev = self.ids.set::<EffectIdx>(id.clone(), Arc::new(move |ctx, args| {
            S::try_from_values(args.iter().cloned())
                .and_then(|args| handler(ctx, args))
        }), S::ARITY).err();
        if let Some(kind) = prev {
            panic!("effect id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_query<N, F, S, I>(&mut self, id: N, handler: F)
    where
        N: Into<SmolStr>,
        F: Fn(&Ctx, S) -> I + 'static,
        I: IntoIterator<Item = Value<Ext>>,
        S: TryFromValues<Ext>,
        Ext: Clone,
    {
        let id = id.into();
        assert!(is_symbol(&id), "query id `{id}` is not a valid symbol");
        let prev = self.ids.set::<QueryIdx>(id.clone(), Arc::new(move |ctx, args, iter_fn| {
            if let Some(args) = S::try_from_values(args.iter().cloned()) {
                let iter = handler.run(ctx, args);
                let mut iter = iter.into_iter();
                iter_fn(&mut iter)
            } else {
                Outcome::Failure
            }
        }), S::ARITY).err();
        if let Some(kind) = prev {
            panic!("query id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_condition<N, F, S>(&mut self, id: N, handler: F)
    where
        N: Into<SmolStr>,
        F: Fn(&Ctx, S) -> bool + 'static,
        S: TryFromValues<Ext>,
        Ext: Clone,
    {
        let id = id.into();
        assert!(is_symbol(&id), "condition id `{id}` is not a valid symbol");
        let prev = self.ids.set::<CondIdx>(id.clone(), Arc::new(move |ctx, args| {
            S::try_from_values(args.iter().cloned())
                .map(|args| handler(ctx, args))
                .unwrap_or(false)
        }), S::ARITY).err();
        if let Some(kind) = prev {
            panic!("condition id `{id}` was already used for {kind}");
        }
    }

    pub fn compile<'a, T>(
        self,
        indent: Indent,
        sources: T,
    ) -> ScriptResult<BehaviorTree<Ctx, Ext, Eff>>
    where
        T: IntoIterator<Item = ScriptSource<'a>>,
    {
        let mut compiler = Compiler::new(self.ids, indent);
        for source in sources {
            compiler.load(source)?;
        }
        let compiled_ids = compiler.compile()?;
        Ok(BehaviorTree { ids: compiled_ids })
    }
}

pub trait QueryCallback<'ctx, S, Ctx, Ext> {
    type Iter: IntoIterator<Item = Value<Ext>> + 'ctx;

    fn run(&self, ctx: &'ctx Ctx, args: S) -> Self::Iter;
}

impl<'ctx, F, I, S, Ctx, Ext> QueryCallback<'ctx, S, Ctx, Ext> for F
where
    F: Fn(&'ctx Ctx, S) -> I + 'static,
    I: IntoIterator<Item = Value<Ext>> + 'ctx,
    Ctx: 'ctx,
{
    type Iter = I;

    fn run(&self, ctx: &'ctx Ctx, args: S) -> Self::Iter {
        self(ctx, args)
    }
}