
use ramble::{Item, Node, GroupKind};
use smol_str::SmolStr;

use crate::system::ContextMode;
use crate::value::StrExt;

use super::{CompileErrorKind, CompileError};
use super::mark;


pub(super) fn require_ref_declaration(
    items: &[Item],
) -> Result<(&SmolStr, &[Item]), CompileErrorKind> {
    match_ref_declaration(items).ok_or(CompileErrorKind::InvalidNodeDeclaration)
}

pub(super) fn match_ref_declaration(items: &[Item]) -> Option<(&SmolStr, &[Item])> {
    let (name, parameters) = items.split_first()?;
    let name = match_symbol(name)?;
    parameters.iter().all(|p| match_variable(p).is_some()).then_some(())?;
    Some((name, parameters))
}

pub(super) fn match_node_ref(items: &[Item]) -> Option<(&Item, ContextMode, &[Item])> {
    let (name_item, items) = items.split_first()?;
    match_symbol(name_item)?;
    let (mode, items) =
        if items.first().map_or(false, |i| match_mark(i, mark::QUERY)) {
            (ContextMode::Inactive, &items[1..])
        } else {
            (ContextMode::Active, items)
        };
    Some((name_item, mode, items))
}

fn match_mark(item: &Item, mark: char) -> bool {
    item.punctuation().map_or(false, |p| p == mark)
}

pub(super) fn match_raw_ref(items: &[Item]) -> Option<(&Item, &[Item])> {
    let (name_item, items) = items.split_first()?;
    match_symbol(name_item)?;
    Some((name_item, items))
}

pub(super) fn match_symbol(item: &Item) -> Option<&SmolStr> {
    let word = item.word()?;
    word.is_symbol().then_some(word)
}

pub(super) fn match_variable(item: &Item) -> Option<&SmolStr> {
    let word = item.word()?;
    word.is_variable().then_some(word)
}

pub(super) fn match_list(item: &Item) -> Option<&[Item]> {
    if let Some((GroupKind::Brackets, items)) = item.group() {
        Some(items)
    } else {
        None
    }
}

pub(super) fn match_group_directive<'a>(
    node: &'a Node,
    keyword: &'static str,
) -> Result<bool, CompileError> {
    let Some((rest, items)) = match_directive(node, keyword)? else {
        return Ok(false);
    };
    if !rest.is_empty() || !items.is_empty() {
        return Err(CompileErrorKind::InvalidDirectiveSyntax(keyword.into()).at(node));
    }
    Ok(true)
}

pub(super) fn match_free_directive<'a>(
    node: &'a Node,
) -> Option<(&'a Item, &'a [Item], &'a [Item])> {
    let (first, _) = node.items.split_first()?;
    let name = first.word_str()?;
    let (signature, arguments) = match_directive(node, name).ok()??;
    Some((first, signature, arguments))
}

pub(super) fn match_directive<'a>(
    node: &'a Node,
    keyword: &str,
) -> Result<Option<(&'a [Item], &'a [Item])>, CompileError> {
    let Some((first, rest)) = node.items.split_first() else {
        panic!("nodes with empty item sets cannot be compiled");
    };
    if !match_word(first, keyword) {
        return Ok(None);
    }
    let Some(index) = rest.iter().position(|item| match_punctuation(item, mark::DECLARE)) else {
        return Err(CompileErrorKind::InvalidDirectiveForm.at(node));
    };
    Ok(Some((&rest[..index], &rest[(index + 1)..])))
}

pub(super) fn match_punctuation(item: &Item, wanted: char) -> bool {
    item.punctuation().map_or(false, |c| c == wanted)
}

pub(super) fn match_word(item: &Item, wanted: &str) -> bool {
    item.word_str().map_or(false, |s| s == wanted)
}