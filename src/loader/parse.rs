
use ramble::{Item, Node};
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
    let (mark, items) = items.split_first()?;
    let is_active = match mark.punctuation()? {
        mark::GOAL => Some(ContextMode::Active),
        mark::QUERY => Some(ContextMode::Inactive),
        _ => None,
    }?;
    Some((name_item, is_active, items))
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

pub(super) fn match_group_directive<'a>(
    node: &'a Node,
    keyword: &'static str,
) -> Result<bool, CompileError> {
    let Some((rest, items)) = match_directive(node, keyword)? else {
        return Ok(false);
    };
    if !rest.is_empty() || !items.is_empty() {
        return Err(CompileErrorKind::InvalidDirectiveSyntax(keyword).at(node));
    }
    Ok(true)
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