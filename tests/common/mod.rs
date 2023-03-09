#![allow(dead_code)]

pub fn realign(content: &str) -> String {
    let common = content.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim().len())
        .min()
        .unwrap_or(0);
    content.lines()
        .map(|line| if line.trim().is_empty() { "" } else { &line[common..] })
        .fold(String::new(), |s, line| s + line + "\n")
}
