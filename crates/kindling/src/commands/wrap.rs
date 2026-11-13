//! `wrap -- <command...>` — run a command and capture it as a `command`
//! observation, then propagate the child's exit code.
//!
//! Mirrors the `log` verb's write path (in-process service or `--via-daemon`
//! client) but sources the observation from a spawned subprocess rather than an
//! explicit string.
//!
//! Relay and capture are kept separate on purpose:
//!
//! * **Relay** (transparent mode) forwards the child's raw bytes to the parent
//!   stdout/stderr live and unbounded, so `wrap` behaves like the wrapped
//!   command and never corrupts or truncates its output.
//! * **Capture** keeps only a bounded prefix per stream, decoded lossily and
//!   truncated at a whitespace boundary so a secret token is never split (a
//!   split token could dodge the service sanitizer's minimum-length rule).
//!   The service still masks secrets in the stored provenance.
//!
//! A recording failure is reported on stderr and does not replace the child's
//! exit code, so a script never retries a command whose side effects already
//! completed.

use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use kindling_service::AppendObservationOptions;
use kindling_types::{ObservationInput, ObservationKind};

use crate::cli::WrapArgs;
use crate::commands::{resolve_repo, scope_from};
use crate::output::format_json;
use crate::{build_client, open_service, runtime, CliError};

/// Maximum captured stdout/stderr length (chars) recorded in provenance.
const MAX_CAPTURE: usize = 2_000;

/// Byte budget kept per stream before the char-level, token-safe truncation.
/// Generous enough that the char truncation (not the byte cap) decides the
/// boundary for any realistic output, while still bounding memory for chatty
/// commands.
const CAPTURE_BYTES: usize = 16 * 1024;

pub fn run(args: WrapArgs, via_daemon: bool) -> Result<i32, CliError> {
    // Default the repo scope to the enclosing git root so `search` finds it.
    let repo = resolve_repo(args.repo.as_deref(), false);
    let scope = scope_from(args.session.as_deref(), repo.as_deref());

    // Open the write target before running the child, so `--via-daemon --db`
    // (or a DB open failure) is rejected up front — consistent with `log`.
    let backend = if via_daemon {
        WriteBackend::Daemon(build_client(args.common.db.as_deref())?)
    } else {
        WriteBackend::Embedded(open_service(args.common.db.as_deref())?.0)
    };

    // In JSON mode we suppress relay (the observation JSON owns stdout); in
    // transparent mode we relay the child's raw bytes live.
    let (code, stdout, stderr) = spawn_and_capture(&args.command, !args.common.json)?;
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

    // A recording failure must not replace the child's exit status: report it
    // on stderr and still return the child's code. Relay already happened.
    let observation = match backend {
        WriteBackend::Daemon(client) => match runtime()?.block_on(async move {
            client
                .append_observation(input, args.capsule.clone(), None)
                .await
        }) {
            Ok(outcome) => outcome.observation,
            Err(err) => {
                eprintln!("warning: failed to record wrap observation: {err}");
                return Ok(code);
            }
        },
        WriteBackend::Embedded(service) => {
            match service.append_observation(
                input,
                AppendObservationOptions {
                    capsule_id: args.capsule.clone(),
                    validate: true,
                },
            ) {
                Ok(outcome) => outcome.observation,
                Err(err) => {
                    eprintln!("warning: failed to record wrap observation: {err}");
                    return Ok(code);
                }
            }
        }
    };

    if args.common.json {
        println!("{}", format_json(&observation, true)?);
    }

    Ok(code)
}

/// The write path selected by `--via-daemon`, opened before the child runs.
enum WriteBackend {
    Daemon(kindling_client::Client),
    Embedded(kindling_service::KindlingService),
}

/// Spawn `argv`, streaming its stdout/stderr to the parent (when `relay`) while
/// keeping a bounded capture of each stream. Returns the exit code and the
/// token-safe truncated captures.
fn spawn_and_capture(argv: &[String], relay: bool) -> Result<(i32, String, String), CliError> {
    let (program, args) = argv.split_first().ok_or_else(|| {
        CliError::Invalid(
            "wrap requires a command to run (e.g. `kindling wrap -- echo hi`)".to_string(),
        )
    })?;

    let mut child = Command::new(program.as_str())
        .args(args.iter().map(String::as_str))
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CliError::Invalid(format!("failed to run '{program}': {e}")))?;

    let child_out = child
        .stdout
        .take()
        .ok_or_else(|| CliError::Invalid(format!("failed to capture '{program}' stdout")))?;
    let child_err = child
        .stderr
        .take()
        .ok_or_else(|| CliError::Invalid(format!("failed to capture '{program}' stderr")))?;

    let out_target: Box<dyn Write + Send> = if relay {
        Box::new(std::io::stdout())
    } else {
        Box::new(std::io::sink())
    };
    let err_target: Box<dyn Write + Send> = if relay {
        Box::new(std::io::stderr())
    } else {
        Box::new(std::io::sink())
    };

    let cap_out = Arc::new(Mutex::new(Capture::new()));
    let cap_err = Arc::new(Mutex::new(Capture::new()));

    let t_out = thread::spawn({
        let cap = Arc::clone(&cap_out);
        move || drain(child_out, out_target, cap)
    });
    let t_err = thread::spawn({
        let cap = Arc::clone(&cap_err);
        move || drain(child_err, err_target, cap)
    });

    let status = child.wait()?;
    let _ = t_out.join();
    let _ = t_err.join();

    let code = exit_status_code(status);
    let out = cap_out.lock().unwrap().finish();
    let err = cap_err.lock().unwrap().finish();
    Ok((code, out, err))
}

/// Drain a child stream: relay every chunk to `writer` and append it to the
/// bounded `capture`.
fn drain<R: Read, W: Write + Send>(mut reader: R, mut writer: W, capture: Arc<Mutex<Capture>>) {
    let mut buf = [0u8; 8192];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        let _ = writer.write_all(&buf[..n]);
        let _ = writer.flush();
        capture.lock().unwrap().push(&buf[..n]);
    }
}

/// A bounded raw-byte buffer per stream, decoded lossily and token-safe
/// truncated on finish.
struct Capture {
    bytes: Vec<u8>,
    cap: usize,
}

impl Capture {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            cap: CAPTURE_BYTES,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        let remaining = self.cap.saturating_sub(self.bytes.len());
        if remaining > 0 {
            self.bytes
                .extend_from_slice(&chunk[..remaining.min(chunk.len())]);
        }
    }

    fn finish(&self) -> String {
        token_safe_truncate(&String::from_utf8_lossy(&self.bytes), MAX_CAPTURE)
    }
}

/// Truncate `s` to at most `max_chars`, cutting at the last whitespace boundary
/// so a token (a secret included) is never split across the cut.
fn token_safe_truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let head: String = s.chars().take(max_chars).collect();
    match head.rfind(char::is_whitespace) {
        Some(i) if i > 0 => format!("{} ...", &head[..i]),
        _ => format!("{head}..."),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_safe_truncate_keeps_short_strings_whole() {
        assert_eq!(token_safe_truncate("short", 100), "short");
        assert_eq!(token_safe_truncate("", 100), "");
    }

    #[test]
    fn token_safe_truncate_cuts_at_whitespace() {
        let s = "one two three four five six";
        let out = token_safe_truncate(s, 8);
        // "one two " is 8 chars; the cut lands on the trailing whitespace, so
        // the result is "one two ..." (no partial token).
        assert_eq!(out, "one two ...");
    }

    #[test]
    fn token_safe_truncate_never_splits_a_token() {
        let s = "ghp_abcdefghijklmnopqrstuvwxyz0123456789 tail";
        let out = token_safe_truncate(s, 20);
        // The 20-char head ends mid-token; the cut backs up to the space.
        assert!(!out.ends_with("ghp_..."), "must not split the token: {out}");
        assert!(out.ends_with("..."));
    }
}
