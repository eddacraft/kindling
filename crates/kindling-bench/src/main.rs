use std::error::Error;

use kindling_bench::profile::Profile;
use kindling_bench::workloads::{self, MeasurementMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut args = std::env::args().skip(1);
    let mut profile_name = "smoke".to_string();
    let mut profile_json: Option<String> = None;
    let mut pretty = false;
    let mut only_group: Option<String> = None;
    let mut mode = MeasurementMode::InProcess;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => {
                profile_name = args.next().ok_or("--profile requires a value")?;
            }
            "--profile-json" => {
                profile_json = Some(args.next().ok_or("--profile-json requires a value")?);
            }
            "--pretty" => pretty = true,
            "--only-group" => {
                only_group = Some(args.next().ok_or("--only-group requires a value")?);
            }
            "--isolated-process" => mode = MeasurementMode::IsolatedChild,
            "--measurement-scope" => {
                let scope = args.next().ok_or("--measurement-scope requires a value")?;
                mode = match scope.as_str() {
                    "in-process" => MeasurementMode::InProcess,
                    "isolated-child" => MeasurementMode::IsolatedChild,
                    other => {
                        return Err(format!(
                            "unknown measurement scope '{other}'; expected in-process|isolated-child"
                        )
                        .into());
                    }
                };
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: kindling-bench [--profile smoke|standard|stress] [--pretty]\n\
                     \n\
                     Resource modes:\n\
                       (default)              in-process sampling (groups share one process)\n\
                       --isolated-process     each group runs in a fresh child process\n\
                       --only-group NAME      run one group and emit its JSON (child entry)\n\
                       --measurement-scope S  in-process|isolated-child (label + mode)"
                );
                return Ok(());
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }
    let profile = match profile_json {
        Some(json) => serde_json::from_str(&json)?,
        None => Profile::from_name(&profile_name)
            .ok_or_else(|| format!("unknown profile: {profile_name}"))?,
    };

    if let Some(group) = only_group {
        // Child entrypoint: one group, labelled with the requested scope.
        let report = workloads::run_group(profile, &group, mode).await?;
        if pretty {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", serde_json::to_string(&report)?);
        }
        return Ok(());
    }

    let report = workloads::run_with_mode(profile, mode).await?;
    if pretty {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", serde_json::to_string(&report)?);
    }
    Ok(())
}
