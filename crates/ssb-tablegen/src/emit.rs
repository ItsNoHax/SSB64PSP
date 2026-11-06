//! Shared formatting for the generated Rust.

/// The generated header every table file starts with.
pub fn header(what: &str) -> String {
    format!(
        "// Generated at build time by crates/ssb-tablegen from the user's ROM:\n\
         // {what}. Not committed; never edit.\n\n"
    )
}
