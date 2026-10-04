//! `behavior` command-line interface over the engine API (contracts/engine-api.md), as a library
//! (feature 008): the binary and the Python binding's console script run the same `run`.
//!
//! Exit codes: 0 admitted / ALLOW / VALUE / replay match / verified / authorized; 1 DENY /
//! ENTITY_ID_ALREADY_USED / LIFECYCLE_CONFLICT / not verified / refused; 2 admission failure, INVALID_INPUT, INVALID_STATE, intent rejected, replay mismatch,
//! or invalid governance input; 3 ERROR or solver unavailable; 64 usage error.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::Write;

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
    /// Print the SchemaHash of the store schema wire IR declares (feature 009).
    SchemaHash { wire: String },
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
    /// Evaluate a read in plain mode (feature 010) and print its read record.
    Read { wire: String, request: String },
    /// Replay a read record from its own facts (feature 010).
    ReadReplay { wire: String, record: String },
    /// Evaluate a read intent with a host context (feature 010): print only the response for
    /// the caller; `--record` writes the full read record for the host.
    ReadIntent {
        wire: String,
        intent: String,
        host: String,
        #[arg(long)]
        record: Option<String>,
    },
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
    /// Print the release's versions (engine, wire IR, records, store documents, verifier).
    EngineInfo,
    /// Admit, verify or apply a migration between two schemas (feature 009).
    Migration {
        #[command(subcommand)]
        command: MigrationCommand,
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
        // Lifecycle refusals (feature 006) are decisions, like DENY.
        "DENY" | "ENTITY_ID_ALREADY_USED" | "LIFECYCLE_CONFLICT" => 1,
        "ERROR" => 3,
        _ => 2,
    }
}

/// Exit code for a read result (feature 010), in the same convention as decisions.
fn read_code(result: &str) -> u8 {
    match result {
        "VALUE" => 0,
        "EVALUATION_ERROR" => 3,
        _ => 2,
    }
}

fn read(path: &str) -> Result<String, u8> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("behavior: cannot read {path}: {e}");
        USAGE
    })
}

#[derive(Subcommand)]
enum MigrationCommand {
    /// Admit a migration document against its source and target wire IR; print the resolved
    /// migration, its hash and its reviewable summary.
    Admit {
        source: String,
        target: String,
        migration: String,
    },
    /// Verify a migration with the SMT solver and print its attestation.
    Verify {
        source: String,
        target: String,
        migration: String,
        #[arg(long)]
        profile: Option<String>,
    },
    /// Apply a migration to a supplied source universe (a JSON array of {entity, value}); there
    /// is no store on the command line.
    Apply {
        source: String,
        target: String,
        migration: String,
        facts: String,
    },
}

/// The source and target modules and the admitted migration, or the exit code after printing
/// the admission errors.
fn migration_inputs(
    source: &str,
    target: &str,
    migration: &str,
) -> Result<
    (
        behavior_core::semantic::module::Module,
        behavior_core::semantic::module::Module,
        behavior_core::migration::Migration,
    ),
    u8,
> {
    let admit = |path: &str| match behavior_core::admit(&read(path)?) {
        Ok(m) => Ok(m),
        Err(r) => {
            emit(&r.to_json_string(), false);
            Err(2)
        }
    };
    let (s, t) = (admit(source)?, admit(target)?);
    match behavior_core::migration::admit_migration(&s, &t, &read(migration)?) {
        Ok(m) => Ok((s, t, m)),
        Err(r) => {
            emit(&r.to_json_string(), false);
            Err(2)
        }
    }
}

fn migration_command(command: MigrationCommand) -> Result<u8, u8> {
    let canonical = |v: &serde_json::Value| {
        behavior_core::canonical::to_canonical_string(v).unwrap_or_default()
    };
    match command {
        MigrationCommand::Admit {
            source,
            target,
            migration,
        } => {
            let (_, _, m) = match migration_inputs(&source, &target, &migration) {
                Ok(x) => x,
                Err(code) => return Ok(code),
            };
            let out = serde_json::json!({
                "ok": true, "hash": m.hash(), "summary": m.summary(), "resolved": m.resolved(),
            });
            emit(&canonical(&out), true);
            Ok(0)
        }
        MigrationCommand::Verify {
            source,
            target,
            migration,
            profile,
        } => {
            let profile = match profile {
                Some(p) => behavior_verify::Profile::from_json(&read(&p)?).map_err(|e| {
                    eprintln!("behavior: {e}");
                    USAGE
                })?,
                None => behavior_verify::Profile::default(),
            };
            let (s, t, m) = match migration_inputs(&source, &target, &migration) {
                Ok(x) => x,
                Err(code) => return Ok(code),
            };
            let solver = behavior_verify::solver::Z3Process::from_env()
                .map_err(|e| {
                    eprintln!("behavior: {e}");
                    3
                })?
                .with_guard(std::time::Duration::from_millis(
                    profile.wall_clock_guard_ms,
                ));
            if let Some(notice) = solver.version_mismatch() {
                eprintln!("behavior: warning: {notice}");
            }
            let a = behavior_verify::verify_migration(&m, &s, &t, &profile, None, &solver);
            emit(&a.to_json_string(), false);
            Ok(if a.result == "verified" { 0 } else { 1 })
        }
        MigrationCommand::Apply {
            source,
            target,
            migration,
            facts,
        } => {
            let (s, t, m) = match migration_inputs(&source, &target, &migration) {
                Ok(x) => x,
                Err(code) => return Ok(code),
            };
            let universe: Vec<serde_json::Value> =
                serde_json::from_str(&read(&facts)?).map_err(|e| {
                    eprintln!("behavior: {facts}: {e}");
                    2
                })?;
            let mut entities = Vec::new();
            for e in universe {
                let (Some(entity), Some(value)) = (e["entity"].as_str(), e.get("value")) else {
                    eprintln!("behavior: {facts}: every item needs `entity` and `value`");
                    return Ok(2);
                };
                entities.push(behavior_core::migration::SourceEntity {
                    entity: entity.to_string(),
                    value: value.clone(),
                });
            }
            let r = behavior_core::migration::apply_migration(&m, &s, &t, &entities);
            emit(
                &canonical(&behavior_core::migration::outcome_json(&r)),
                true,
            );
            Ok(if r.is_ok() { 0 } else { 1 })
        }
    }
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
) -> Result<u8, u8> {
    let profile = match profile {
        Some(p) => behavior_verify::Profile::from_json(&read(p)?).map_err(|e| {
            eprintln!("behavior: {e}");
            USAGE
        })?,
        None => behavior_verify::Profile::default(),
    };
    let module = match behavior_core::admit(&read(wire)?) {
        Ok(m) => m,
        Err(r) => {
            emit(&r.to_json_string(), false);
            return Ok(2);
        }
    };
    let solver = behavior_verify::solver::Z3Process::from_env()
        .map_err(|e| {
            eprintln!("behavior: {e}");
            3
        })?
        .with_guard(std::time::Duration::from_millis(
            profile.wall_clock_guard_ms,
        ));
    if let Some(notice) = solver.version_mismatch() {
        eprintln!("behavior: warning: {notice}");
    }
    let a = behavior_verify::verify(&module, &profile, cache.map(std::path::Path::new), &solver);
    let text = a.to_json_string();
    if let Some(path) = out {
        std::fs::write(path, &text).map_err(|e| {
            eprintln!("behavior: cannot write {path}: {e}");
            USAGE
        })?;
    }
    emit(&text, false);
    Ok(if a.result == "verified" { 0 } else { 1 })
}

fn invalid(e: behavior_verify::governance::GovernanceError) -> u8 {
    eprintln!("behavior: {e}");
    2
}

fn dispatch(cli: Cli) -> Result<u8, u8> {
    match cli.command {
        Command::Migration { command } => migration_command(command),
        Command::EngineInfo => {
            emit(
                &behavior_core::canonical::to_canonical_string(&engine_info()).unwrap_or_default(),
                true,
            );
            Ok(0)
        }
        Command::WaiverHash { waiver } => {
            let h = behavior_verify::governance::waiver_hash(&read(&waiver)?).map_err(invalid)?;
            emit(&h, true);
            Ok(0)
        }
        Command::SignWaiver { waiver, seed } => {
            let signed = behavior_verify::governance::sign_waiver(&read(&seed)?, &read(&waiver)?)
                .map_err(invalid)?;
            emit(
                &behavior_core::canonical::to_canonical_string(&signed).unwrap_or_default(),
                false,
            );
            Ok(0)
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
                    return Ok(2);
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
            Ok(if a.decision == "allow" { 0 } else { 1 })
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
            Ok(if r.ok { 0 } else { 2 })
        }
        Command::Version { wire } => {
            let r = behavior_core::admission_report(&read(&wire)?);
            match r.behavior_version {
                Some(v) if r.ok => {
                    emit(&v, true);
                    Ok(0)
                }
                _ => {
                    emit(&r.to_json_string(), false);
                    Ok(2)
                }
            }
        }
        Command::SchemaHash { wire } => match behavior_core::admit(&read(&wire)?) {
            Ok(m) => {
                emit(&behavior_core::schema(&m).hash, true);
                Ok(0)
            }
            Err(r) => {
                emit(&r.to_json_string(), false);
                Ok(2)
            }
        },
        Command::Hashes { wire } => {
            let r = behavior_core::admission_report(&read(&wire)?);
            if r.ok {
                let items = serde_json::to_value(&r.items).unwrap_or_default();
                emit(
                    &behavior_core::canonical::to_canonical_string(&items).unwrap_or_default(),
                    false,
                );
                Ok(0)
            } else {
                emit(&r.to_json_string(), false);
                Ok(2)
            }
        }
        Command::Eval { wire, request } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(2);
                }
            };
            let record = behavior_core::evaluate(&module, &read(&request)?);
            emit(&record.to_json_string(), false);
            Ok(result_code(record.result()))
        }
        Command::Replay { wire, record } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(2);
                }
            };
            let r = behavior_core::replay(&module, &read(&record)?);
            emit(&r.to_json_string(), false);
            Ok(if r.matches { 0 } else { 2 })
        }
        Command::Read { wire, request } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(2);
                }
            };
            match behavior_core::read::evaluate_read_request(&module, &read(&request)?) {
                Ok(x) => {
                    emit(&x.record.to_json_string(), false);
                    Ok(read_code(x.record.result()))
                }
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    Ok(2)
                }
            }
        }
        Command::ReadReplay { wire, record } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(2);
                }
            };
            let r = behavior_core::read::replay_read(&module, &read(&record)?);
            emit(&r.to_json_string(), false);
            Ok(if r.matches { 0 } else { 2 })
        }
        Command::ReadIntent {
            wire,
            intent,
            host,
            record,
        } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(2);
                }
            };
            match behavior_core::read::evaluate_read_intent(&module, &read(&intent)?, &read(&host)?)
            {
                Ok(x) => {
                    if let Some(path) = record {
                        std::fs::write(&path, x.record.to_json_string()).map_err(|e| {
                            eprintln!("behavior: cannot write {path}: {e}");
                            USAGE
                        })?;
                    }
                    emit(&x.response.to_json_string(), false);
                    Ok(read_code(x.response.result()))
                }
                Err(rejection) => {
                    emit(&rejection.to_json_string(), false);
                    Ok(2)
                }
            }
        }
        Command::Intent { wire, intent, host } => {
            let module = match behavior_core::admit(&read(&wire)?) {
                Ok(m) => m,
                Err(r) => {
                    emit(&r.to_json_string(), false);
                    return Ok(2);
                }
            };
            match behavior_core::evaluate_intent(&module, &read(&intent)?, &read(&host)?) {
                Ok(record) => {
                    emit(&record.to_json_string(), false);
                    Ok(result_code(record.result()))
                }
                Err(rejection) => {
                    emit(&rejection.to_json_string(), false);
                    Ok(2)
                }
            }
        }
    }
}

/// Every version of this release (feature 008, research R2): the engine and its formats, the
/// store document tags, and the verifier.
pub fn engine_info() -> serde_json::Value {
    let mut v = behavior_core::format_versions();
    if let serde_json::Value::Object(m) = &mut v {
        m.insert(
            "store_documents".into(),
            serde_json::json!(behavior_store::documents::DOCUMENT_TAGS),
        );
        m.insert(
            "verifier".into(),
            serde_json::json!(behavior_verify::VERIFIER_VERSION),
        );
    }
    v
}

/// Runs the command line `args` (the program name first) and returns the exit code. Output is
/// flushed before returning.
pub fn run(args: Vec<OsString>) -> u8 {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => USAGE,
            };
        }
    };
    let c = dispatch(cli).unwrap_or_else(|c| c);
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    c
}
