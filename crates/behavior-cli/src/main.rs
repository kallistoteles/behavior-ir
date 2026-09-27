//! `behavior` command-line interface over the engine API (contracts/engine-api.md).
//!
//! Exit codes: 0 admitted / ALLOW / replay match / verified / authorized; 1 DENY / not verified /
//! refused; 2 admission failure, INVALID_INPUT, INVALID_STATE, intent rejected, replay mismatch,
//! or invalid governance input; 3 ERROR or solver unavailable; 64 usage error.
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
    /// Verify wire IR with the SMT solver and print the verification attestation.
    Verify {
        wire: String,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        cache: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
    /// Print the content hash of a waiver.
    WaiverHash { waiver: String },
    /// Sign a waiver with an Ed25519 key (a file holding the 32-byte seed as hex).
    SignWaiver {
        waiver: String,
        #[arg(long)]
        seed: String,
    },
    /// Decide whether a decision record's transition may be committed under a policy.
    Authorize {
        wire: String,
        record: String,
        #[arg(long)]
        policy: String,
        #[arg(long)]
        attestation: Option<String>,
        #[arg(long = "waiver")]
        waivers: Vec<String>,
        #[arg(long = "signature")]
        signatures: Vec<String>,
        #[arg(long)]
        now: String,
    },
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

fn verify(
    wire: &str,
    profile: Option<&str>,
    cache: Option<&str>,
    out: Option<&str>,
) -> Result<ExitCode, ExitCode> {
    let profile = match profile {
        Some(p) => behavior_verify::Profile::from_json(&read(p)?).map_err(|e| {
            eprintln!("behavior: {e}");
            ExitCode::from(USAGE)
        })?,
        None => behavior_verify::Profile::default(),
    };
    let module = match behavior_core::admit(&read(wire)?) {
        Ok(m) => m,
        Err(r) => {
            emit(&r.to_json_string(), false);
            return Ok(ExitCode::from(2));
        }
    };
    let solver = behavior_verify::solver::Z3Process::from_env()
        .map_err(|e| {
            eprintln!("behavior: {e}");
            ExitCode::from(3)
        })?
        .with_guard(std::time::Duration::from_millis(
            profile.wall_clock_guard_ms,
        ));
    let a = behavior_verify::verify(&module, &profile, cache.map(std::path::Path::new), &solver);
    let text = a.to_json_string();
    if let Some(path) = out {
        std::fs::write(path, &text).map_err(|e| {
            eprintln!("behavior: cannot write {path}: {e}");
            ExitCode::from(USAGE)
        })?;
    }
    emit(&text, false);
    Ok(ExitCode::from(if a.result == "verified" { 0 } else { 1 }))
}

fn invalid(e: behavior_verify::governance::GovernanceError) -> ExitCode {
    eprintln!("behavior: {e}");
    ExitCode::from(2)
}

fn run(cli: Cli) -> Result<ExitCode, ExitCode> {
    match cli.command {
        Command::WaiverHash { waiver } => {
            let h = behavior_verify::governance::waiver_hash(&read(&waiver)?).map_err(invalid)?;
            emit(&h, true);
            Ok(ExitCode::SUCCESS)
        }
        Command::SignWaiver { waiver, seed } => {
            let signed = behavior_verify::governance::sign_waiver(&read(&seed)?, &read(&waiver)?)
                .map_err(invalid)?;
            emit(
                &behavior_core::canonical::to_canonical_string(&signed).unwrap_or_default(),
                false,
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Authorize {
            wire,
            record,
            policy,
            attestation,
            waivers,
            signatures,
            now,
        } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(ExitCode::from(2));
                }
            };
            let attestation = attestation.as_deref().map(read).transpose()?;
            let waivers = waivers
                .iter()
                .map(|w| read(w))
                .collect::<Result<Vec<_>, _>>()?;
            let signatures = signatures
                .iter()
                .map(|s| read(s))
                .collect::<Result<Vec<_>, _>>()?;
            let a = behavior_verify::governance::authorize(
                &read(&policy)?,
                &module,
                &read(&record)?,
                attestation.as_deref(),
                &waivers,
                &signatures,
                &now,
            )
            .map_err(invalid)?;
            emit(&a.to_json_string(), false);
            Ok(ExitCode::from(if a.decision == "allow" { 0 } else { 1 }))
        }
        Command::Verify {
            wire,
            profile,
            cache,
            out,
        } => verify(&wire, profile.as_deref(), cache.as_deref(), out.as_deref()),
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
