use smol_str::SmolStr;
use treelang::{Node as ScriptNode, Item, Directive};

use crate::gen::smol_str_wrapper;
use crate::str::{is_symbol, is_variable};
use crate::tree::ArityError;

use super::{CompileErrorKind, CompileResult, RefClass, Root, Decl};


pub mod kw;

pub(super) fn parse_root_declaration(
    node: &ScriptNode,
) -> CompileResult<Root<Decl>> {
    if let Some(ref_signature) = try_parse_keyword_directive(node, kw::def::NODE)? {
        let (name, parameters) = parse_ref_declaration(ref_signature, node)?;
        Ok(Root::Node(Decl { name, parameters, node: node.clone() }))
    } else if let Some(ref_signature) = try_parse_keyword_directive(node, kw::def::ACTION)? {
        let (name, parameters) = parse_ref_declaration(ref_signature, node)?;
        Ok(Root::Action(Decl { name, parameters, node: node.clone() }))
    } else {
        Err(CompileErrorKind::InvalidRootDeclaration { offset: node.location })
    }
}

pub(super) fn match_directive<'a>(
    node: &'a ScriptNode,
    keyword: &'static str,
) -> Option<(&'a [Item], &'a [Item])> {
    let Directive { signature, arguments, .. } = node.kind.directive()?;
    let (key, signature) = signature.split_first()?;
    let key = key.word_str()?;
    (key == keyword).then_some((signature, arguments))
}

pub(super) fn try_parse_label_directive(
    node: &ScriptNode,
    keyword: &'static str,
) -> CompileResult<bool> {
    let Some(arguments) = try_parse_keyword_directive(node, keyword)? else {
        return Ok(false);
    };
    if arguments.is_empty() {
        Ok(true)
    } else {
        Err(CompileErrorKind::DirectiveArgumentArity {
            keyword,
            offset: node.location,
            error: ArityError { expected: 0, given: arguments.len() }
        })
    }
}

fn try_parse_keyword_directive<'a>(
    node: &'a ScriptNode,
    keyword: &'static str,
) -> CompileResult<Option<&'a [Item]>> {
    let Some((signature, arguments)) = match_directive(node, keyword) else {
        return Ok(None);
    };
    if signature.is_empty() {
        Ok(Some(arguments))
    } else {
        Err(CompileErrorKind::DirectiveSignatureArity {
            keyword,
            offset: node.location,
            error: ArityError { expected: 0, given: signature.len() },
        })
    }
}

pub(super) fn match_ref(items: &[Item]) -> Option<(RefClass<ItemValue<Sym>>, &[Item])> {
    let (first, items) = items.split_first()?;
    if let Some(word) = first.word() {
        if word.ends_with('?') {
            let word = &word[..(word.len() - 1)];
            is_symbol(word).then(|| (
                RefClass::Query(ItemValue { value: Sym(word.into()), item: first.clone() }),
                items,
            ))
        } else {
            is_symbol(word).then(|| (
                RefClass::Raw(ItemValue { value: Sym(word.clone()), item: first.clone() }),
                items,
            ))
        }
    } else {
        None
    }
}

fn parse_ref_declaration(
    items: &[Item],
    node: &ScriptNode,
) -> CompileResult<(ItemValue<Sym>, Vec<ItemValue<Var>>)> {
    let Some((RefClass::Raw(ref_name), parameter_items)) = match_ref(items) else {
        return Err(CompileErrorKind::InvalidRefDeclaration { offset: node.location });
    };
    let mut parameters = Vec::new();
    for item in parameter_items {
        let Some(var) = match_var(item) else {
            return Err(CompileErrorKind::InvalidRefDeclaration { offset: item.location.offset() });
        };
        parameters.push(var);
    }
    Ok((ref_name, parameters))
}

smol_str_wrapper!(pub Sym);
smol_str_wrapper!(pub Var);

pub(super) fn match_sym(item: &Item) -> Option<ItemValue<Sym>> {
    let word = item.word()?;
    if is_symbol(&word) {
        Some(ItemValue { value: Sym(word.clone()), item: item.clone() })
    } else {
        None
    }
}

pub(super) fn match_var(item: &Item) -> Option<ItemValue<Var>> {
    let word = item.word()?;
    if is_variable(&word) {
        Some(ItemValue { value: Var(word.clone()), item: item.clone() })
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ItemValue<T> {
    pub value: T,
    pub item: Item,
}

impl<T> std::ops::Deref for ItemValue<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
