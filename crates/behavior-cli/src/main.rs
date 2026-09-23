//! `behavior` command-line interface over the engine API (contracts/engine-api.md).
//!
//! Exit codes: 0 admitted / ALLOW / replay match; 1 DENY; 2 admission failure, INVALID_INPUT,
//! INVALID_STATE, intent rejected, or replay mismatch; 3 ERROR; 64 usage error.
#![forbid(unsafe_code)]

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "behavior", about = "Behavior IR engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Admit wire IR and print the AdmissionResult.
    Admit { wire: String },
    /// Print the behavior version of wire IR.
    Version { wire: String },
    /// Print the name → item hash table of wire IR.
    Hashes { wire: String },
    /// Evaluate a request.
    Eval { wire: String, request: String },
    /// Evaluate a structured intent with a host context.
    Intent {
        wire: String,
        intent: String,
        host: String,
    },
    /// Replay a decision record.
    Replay { wire: String, record: String },
}

const USAGE: u8 = 64;

/// Exit code for a decision result.
fn result_code(result: &str) -> u8 {
    match result {
        "ALLOW" => 0,
        "DENY" => 1,
        "ERROR" => 3,
        _ => 2,
    }
}

fn read(path: &str) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("behavior: cannot read {path}: {e}");
        ExitCode::from(USAGE)
    })
}

fn emit(text: &str, newline: bool) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    if newline {
        let _ = out.write_all(b"\n");
    }
    let _ = out.flush();
}

fn run(cli: Cli) -> Result<ExitCode, ExitCode> {
    match cli.command {
        Command::Admit { wire } => {
            let r = behavior_core::admission_report(&read(&wire)?);
            emit(&r.to_json_string(), false);
            Ok(ExitCode::from(if r.ok { 0 } else { 2 }))
        }
        Command::Version { wire } => {
            let r = behavior_core::admission_report(&read(&wire)?);
            match r.behavior_version {
                Some(v) if r.ok => {
                    emit(&v, true);
                    Ok(ExitCode::SUCCESS)
                }
                _ => {
                    emit(&r.to_json_string(), false);
                    Ok(ExitCode::from(2))
                }
            }
        }
        Command::Hashes { wire } => {
            let r = behavior_core::admission_report(&read(&wire)?);
            if r.ok {
                let items = serde_json::to_value(&r.items).unwrap_or_default();
                emit(
                    &behavior_core::canonical::to_canonical_string(&items).unwrap_or_default(),
                    false,
                );
                Ok(ExitCode::SUCCESS)
            } else {
                emit(&r.to_json_string(), false);
                Ok(ExitCode::from(2))
            }
        }
        Command::Eval { wire, request } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(ExitCode::from(2));
                }
            };
            let record = behavior_core::evaluate(&module, &read(&request)?);
            emit(&record.to_json_string(), false);
            Ok(ExitCode::from(result_code(record.result())))
        }
        Command::Replay { wire, record } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(ExitCode::from(2));
                }
            };
            let r = behavior_core::replay(&module, &read(&record)?);
            emit(&r.to_json_string(), false);
            Ok(ExitCode::from(if r.matches { 0 } else { 2 }))
        }
        Command::Intent { wire, intent, host } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(ExitCode::from(2));
                }
            };
            match behavior_core::evaluate_intent(&module, &read(&intent)?, &read(&host)?) {
                Ok(record) => {
                    emit(&record.to_json_string(), false);
                    Ok(ExitCode::from(result_code(record.result())))
                }
                Err(rejection) => {
                    emit(&rejection.to_json_string(), false);
                    Ok(ExitCode::from(2))
                }
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    ExitCode::SUCCESS
                }
                _ => ExitCode::from(USAGE),
            };
        }
    };
    run(cli).unwrap_or_else(|code| code)
}
