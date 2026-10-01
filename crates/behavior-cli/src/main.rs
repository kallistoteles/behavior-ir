//! The `behavior` binary: the command-line library's `run` (see `lib.rs` for the exit codes).
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(behavior_cli::run(std::env::args_os().collect()))
}
