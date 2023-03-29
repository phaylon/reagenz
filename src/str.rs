
pub fn is_symbol(value: &str) -> bool {
    !value.is_empty()
    && !value.starts_with(|c: char| c == '-' || c.is_ascii_digit())
    && !value.chars().any(|c| c.is_whitespace() || "$?:;()[]{}".contains(c))
}

pub fn is_variable(value: &str) -> bool {
    value.starts_with('$') && is_symbol(&value[1..])
}