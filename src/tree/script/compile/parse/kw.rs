
pub mod def {
    pub const ACTION: &str = "action";
    pub const NODE: &str = "node";

    pub mod action {
        pub const CONDITIONS: &str = "conditions";
        pub const EFFECTS: &str = "effects";
        pub const DISCOVERY: &str = "discovery";
        pub const INHERIT: &str = "inherit";
    }
}

pub mod dir {
    pub const SELECT: &str = "select";
    pub const SEQUENCE: &str = "do";
    pub const NONE: &str = "none";
    pub const VISIT: &str = "visit";
    pub const MATCH: &str = "match";
    pub const RANDOM: &str = "random";
    pub const RANDOM_ANY: &str = "any-random";

    pub mod query {
        pub const SELECT: &str = "for-any";
        pub const SEQUENCE: &str = "for-every";
        pub const FIRST: &str = "with-first";
        pub const LAST: &str = "with-last";
        pub const VISIT: &str = "visit-every";
    }

    pub mod switch {
        pub const SWITCH: &str = "switch";
        pub const CASE: &str = "case";
    }

    pub mod cond {
        pub const COND: &str = "cond";
        pub const CASE: &str = "when";
        pub const BODY: &str = "do";
        pub const ELSE: &str = "else";
    }
}
