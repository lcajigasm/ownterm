use cfg_aliases::cfg_aliases;

#[allow(semicolon_in_expressions_from_non_local_macros)]
fn main() {
    // This sets the same cfg aliases as the `warp` crate, used to gate crash-recovery flags.
    cfg_aliases! {
        linux_or_windows: { any(target_os = "linux", windows) },
        enable_crash_recovery: { linux_or_windows },
    }
}
