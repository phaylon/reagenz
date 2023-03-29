use crate::tree::id_space::{IdSpace, GlobalIdx};
use crate::tree::script::{CompileErrorKind, Pattern, ProtoValue};
use crate::tree::script::compile::CompileResult;
use crate::tree::script::compile::parse::{Var, ItemValue};


pub struct Env<'a, Ctx, Ext, Eff> {
    ids: &'a IdSpace<Ctx, Ext, Eff>,
    vars: Vec<Var>,
    max_vars: usize,
}

impl<'a, Ctx, Ext, Eff> Env<'a, Ctx, Ext, Eff> {
    pub fn new(ids: &'a IdSpace<Ctx, Ext, Eff>) -> Self {
        Self {
            ids,
            vars: Vec::new(),
            max_vars: 0,
        }
    }

    pub fn declare(&mut self, var: &ItemValue<Var>) -> CompileResult<usize> {
        let name = var.as_smol_str();
        let span = var.item.location;
        if self.vars.contains(&var.value) {
            Err(CompileErrorKind::ShadowedLexical { span, name: name.clone() })
        } else if self.ids.contains::<GlobalIdx>(name) {
            Err(CompileErrorKind::ShadowedGlobal { span, name: name.clone() })
        } else {
            let index = self.vars.len();
            self.vars.push(var.value.clone());
            self.max_vars = self.max_vars.max(self.vars.len());
            Ok(index)
        }
    }

    pub fn scope<'i, I, F, R>(&mut self, vars: I, scope: F) -> CompileResult<R>
    where
        I: IntoIterator<Item = &'i ItemValue<Var>>,
        F: FnOnce(&mut Self) -> CompileResult<R>,
    {
        let len = self.vars.len();
        let mut env = scopeguard::guard(self, |env| env.vars.truncate(len));
        for var in vars {
            env.declare(var)?;
        }
        scope(&mut env)
    }

    pub fn resolve_pattern(&mut self, var: &ItemValue<Var>) -> Pattern<Ext> {
        let name = var.value.as_smol_str().as_str();
        if let Some(index) = self.vars.iter().position(|prev_var| *prev_var == var.value) {
            Pattern::Lexical(index)
        } else if let Ok(index) = self.ids.resolve::<GlobalIdx>(name, 0) {
            Pattern::Global(index)
        } else {
            self.declare(var).unwrap();
            Pattern::Bind
        }
    }

    pub fn resolve(&self, var: &ItemValue<Var>) -> CompileResult<ProtoValue<Ext>> {
        let name = var.value.as_smol_str();
        let span = var.item.location;
        if let Some(index) = self.vars.iter().position(|prev_var| *prev_var == var.value) {
            Ok(ProtoValue::Lexical(index))
        } else if let Ok(index) = self.ids.resolve::<GlobalIdx>(name, 0) {
            Ok(ProtoValue::Global(index))
        } else {
            Err(CompileErrorKind::UnboundVariable { span, name: name.clone() })
        }
    }

    pub fn max_vars(&self) -> usize {
        self.max_vars
    }

    pub fn ids(&self) -> &IdSpace<Ctx, Ext, Eff> {
        self.ids
    }
}
