#![allow(dead_code)]

#[macro_export]
macro_rules! make_system {
    ($state:ty, $effect:ty, $value:ty $(,)?) => {
        {
            struct TestSystem;
            impl reagenz::World for TestSystem {
                type State = $state;
                type Effect = $effect;
                type Value = $value;
            }
            reagenz::system::System::<TestSystem>::default()
        }
    };
}

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

