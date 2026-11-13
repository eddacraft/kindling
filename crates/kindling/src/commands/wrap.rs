//! `wrap -- <command...>` — run a command and capture it as a `command`
//! observation, then propagate the child's exit code.
//!
//! Mirrors the `log` verb's write path (in-process service or `--via-daemon`
//! client) but sources the observation from a spawned subprocess rather than an
//! explicit string. The command line is the observation content; the exit code
//! and truncated stdout/stderr are recorded in provenance.

use std::process::{Command, ExitStatus, Stdio};

use kindling_service::AppendObservationOptions;
use kindling_types::{ObservationInput, ObservationKind};

use crate::cli::WrapArgs;
use crate::commands::{resolve_repo, scope_from};
use crate::output::format_json;
use crate::{build_client, open_service, runtime, CliError};

/// Maximum captured stdout/stderr length (chars) per stream, recorded in
/// provenance. Keeps observations bounded for long-running or chatty commands.
const MAX_CAPTURE: usize = 2_000;

pub fn run(args: WrapArgs, via_daemon: bool) -> Result<i32, CliError> {
    let (code, stdout, stderr) = spawn_and_capture(&args.command)?;

    // Default the repo scope to the enclosing git root so `search` finds it.
    let repo = resolve_repo(args.repo.as_deref(), false);
    let scope = scope_from(args.session.as_deref(), repo.as_deref());
    let command_line = args.command.join(" ");

    let input = ObservationInput {
        id: None,
        kind: ObservationKind::Command,
        content: command_line.clone(),
        provenance: Some(wrap_provenance(&command_line, code, &stdout, &stderr)),
        ts: None,
        scope_ids: scope,
        redacted: Some(false),
    };

    let observation = if via_daemon {
        let client = build_client(args.common.db.as_deref())?;
        runtime()?
            .block_on(async {
                client
                    .append_observation(input, args.capsule.clone(), None)
                    .await
            })?
            .observation
    } else {
        let (service, _db) = open_service(args.common.db.as_deref())?;
        service
            .append_observation(
                input,
                AppendObservationOptions {
                    capsule_id: args.capsule.clone(),
                    validate: true,
                },
            )?
            .observation
    };

    if args.common.json {
        // JSON mode reports the stored observation; the child's output is
        // already captured in provenance.
        println!("{}", format_json(&observation, true)?);
    } else {
        // Transparent mode: relay the child's output unchanged so `wrap`
        // behaves like the wrapped command.
        use std::io::Write;
        let mut out = std::io::stdout();
        let _ = out.write_all(stdout.as_bytes());
        let _ = out.flush();
        let mut err = std::io::stderr();
        let _ = err.write_all(stderr.as_bytes());
        let _ = err.flush();
    }

    Ok(code)
}

/// Spawn `argv`, capturing stdout/stderr and the exit status.
fn spawn_and_capture(argv: &[String]) -> Result<(i32, String, String), CliError> {
    let (program, args) = argv.split_first().ok_or_else(|| {
        CliError::Invalid(
            "wrap requires a command to run (e.g. `kindling wrap -- echo hi`)".to_string(),
        )
    })?;

    let output = Command::new(program.as_str())
        .args(args.iter().map(String::as_str))
        .stdin(Stdio::inherit())
        .output()
        .map_err(|e| CliError::Invalid(format!("failed to run '{program}': {e}")))?;

    let code = exit_status_code(output.status);
    let stdout = truncate_capture(String::from_utf8_lossy(&output.stdout).into_owned());
    let stderr = truncate_capture(String::from_utf8_lossy(&output.stderr).into_owned());
    Ok((code, stdout, stderr))
}

fn wrap_provenance(
    command: &str,
    code: i32,
    stdout: &str,
    stderr: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert(
        "source".to_string(),
        serde_json::Value::String("cli".to_string()),
    );
    map.insert(
        "command".to_string(),
        serde_json::Value::String(command.to_string()),
    );
    map.insert("exitCode".to_string(), serde_json::Value::from(code));
    map.insert(
        "stdout".to_string(),
        serde_json::Value::String(stdout.to_string()),
    );
    map.insert(
        "stderr".to_string(),
        serde_json::Value::String(stderr.to_string()),
    );
    map
}

fn truncate_capture(s: String) -> String {
    if s.chars().count() <= MAX_CAPTURE {
        s
    } else {
        let keep = MAX_CAPTURE.saturating_sub(3);
        let prefix: String = s.chars().take(keep).collect();
        format!("{prefix}...")
    }
}

#[cfg(unix)]
fn exit_status_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

#[cfg(not(unix))]
fn exit_status_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}
