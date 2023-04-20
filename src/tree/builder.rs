
use derivative::Derivative;
use smol_str::SmolStr;
use treelang::Indent;

use crate::str::{is_variable, is_symbol};
use crate::tree::SeedIdx;
use crate::tree::id_space::{QueryIdx, CondIdx};

use super::{BehaviorTree, GlobalFn, EffectFn, QueryFn, CondFn, SeedFn};
use super::id_space::{IdSpace, GlobalIdx, EffectIdx};
use super::script::{ScriptSource, Compiler, CompileResult};


#[derive(Derivative)]
#[derivative(Clone(bound=""), Default(bound=""))]
pub struct BehaviorTreeBuilder<Ctx, Ext, Eff> {
    ids: IdSpace<Ctx, Ext, Eff>
}

impl<Ctx, Ext, Eff> BehaviorTreeBuilder<Ctx, Ext, Eff> {
    #[track_caller]
    pub fn register_global<N>(&mut self, id: N, handler: GlobalFn<Ctx, Ext>)
    where
        N: Into<SmolStr>,
    {
        let id = id.into();
        assert!(is_variable(&id), "global id `{id}` is not a valid variable");
        let prev = self.ids.set::<GlobalIdx>(id.clone(), handler, 0).err();
        if let Some(kind) = prev {
            panic!("global id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_seed<N>(&mut self, id: N, handler: SeedFn<Ctx>)
    where
        N: Into<SmolStr>,
    {
        let id = id.into();
        assert!(is_symbol(&id), "seed id `{id}` is not a valid symbol");
        let prev = self.ids.set::<SeedIdx>(id.clone(), handler, 0).err();
        if let Some(kind) = prev {
            panic!("seed id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_effect<N>(&mut self, id: N, (arity, handler): (usize, EffectFn<Ctx, Ext, Eff>))
    where
        N: Into<SmolStr>,
        Ext: Clone,
    {
        let id = id.into();
        assert!(is_symbol(&id), "effect id `{id}` is not a valid symbol");
        let prev = self.ids.set::<EffectIdx>(id.clone(), handler, arity).err();
        if let Some(kind) = prev {
            panic!("effect id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_query<N>(&mut self, id: N, (arity, handler): (usize, QueryFn<Ctx, Ext, Eff>))
    where
        N: Into<SmolStr>,
        Ext: Clone,
    {
        let id = id.into();
        assert!(is_symbol(&id), "query id `{id}` is not a valid symbol");
        let prev = self.ids.set::<QueryIdx>(id.clone(), handler, arity).err();
        if let Some(kind) = prev {
            panic!("query id `{id}` was already used for {kind}");
        }
    }

    #[track_caller]
    pub fn register_condition<N>(&mut self, id: N, (arity, handler): (usize, CondFn<Ctx, Ext>))
    where
        N: Into<SmolStr>,
        Ext: Clone,
    {
        let id = id.into();
        assert!(is_symbol(&id), "condition id `{id}` is not a valid symbol");
        let prev = self.ids.set::<CondIdx>(id.clone(), handler, arity).err();
        if let Some(kind) = prev {
            panic!("condition id `{id}` was already used for {kind}");
        }
    }

    pub fn compile_str(
        self,
        indent: Indent,
        name: &str,
        content: &str,
    ) -> CompileResult<BehaviorTree<Ctx, Ext, Eff>> {
        self.compile(indent, [
            ScriptSource::Str { name: name.into(), content: content.into() },
        ])
    }

    pub fn compile<'a, T>(
        self,
        indent: Indent,
        sources: T,
    ) -> CompileResult<BehaviorTree<Ctx, Ext, Eff>>
    where
        T: IntoIterator<Item = ScriptSource>,
    {
        let mut compiler = Compiler::new(self.ids, indent);
        for source in sources {
            compiler.load(source)?;
        }
        let compiled_ids = compiler.compile()?;
        Ok(BehaviorTree { ids: compiled_ids })
    }
}
