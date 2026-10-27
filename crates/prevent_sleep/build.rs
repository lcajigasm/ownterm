use cfg_aliases::cfg_aliases;

#[allow(semicolon_in_expressions_from_non_local_macros)]
fn main() {
    cfg_aliases! {
        macos: { target_os = "macos" },
        noop: { not(any(macos, windows)) },
    }
}
