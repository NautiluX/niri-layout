mod config;
mod daemon;
mod engine;
mod ipc;
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Defaults to $XDG_CONFIG_HOME/niri-layout/config.kdl.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Defaults to $NIRI_SOCKET.
    #[arg(long, global = true)]
    socket: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Validate all profiles without contacting niri.
    Check,
    /// Preview changes, without moving or focusing anything.
    Plan {
        profile: String,
        #[arg(long="output",value_parser=parse_output)]
        outputs: Vec<(String, String)>,
    },
    /// Select and apply a profile through the daemon (suitable for Kanshi exec).
    Apply {
        profile: String,
        #[arg(long="output",value_parser=parse_output)]
        outputs: Vec<(String, String)>,
        #[arg(long)]
        once: bool,
    },
    /// Wait for profile selections and newly opened windows; no polling.
    Daemon,
    /// Show the daemon's current profile.
    Status,
    /// Reapply the currently selected profile (for a layout shortcut).
    Reapply,
}
fn parse_output(s: &str) -> std::result::Result<(String, String), String> {
    s.split_once('=')
        .filter(|(a, b)| !a.is_empty() && !b.is_empty())
        .map(|(a, b)| (a.into(), b.into()))
        .ok_or_else(|| "expected role=connector-or-description".into())
}
fn mappings(pairs: Vec<(String, String)>) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for (k, v) in pairs {
        ensure!(
            map.insert(k.clone(), v).is_none(),
            "duplicate output role {k}"
        );
    }
    Ok(map)
}
fn config_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let base = if let Some(p) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(p)
    } else {
        PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?).join(".config")
    };
    Ok(base.join("niri-layout/config.kdl"))
}
fn run() -> Result<()> {
    let cli = Cli::parse();
    let path = config_path(cli.config)?;
    if matches!(cli.command, Command::Check) {
        let c = config::Config::parse(&std::fs::read_to_string(path)?)?;
        println!("Valid: {} profiles", c.profiles.len());
        return Ok(());
    }
    let socket = cli
        .socket
        .or_else(|| std::env::var_os("NIRI_SOCKET").map(PathBuf::from))
        .context("NIRI_SOCKET is not set; run inside niri or pass --socket")?;
    match cli.command {
        Command::Check => unreachable!(),
        Command::Daemon => {
            config::Config::parse(&std::fs::read_to_string(&path)?)?;
            daemon::run(path, socket)?;
        }
        Command::Reapply => {
            let status = daemon::send(&socket, &daemon::Command::Status)?;
            let selection = status.selection.context("no profile selected yet")?;
            let reply = daemon::send(&socket, &daemon::Command::Apply(selection))?;
            ensure!(reply.ok, "{}", reply.message);
            println!("{}", reply.message);
        }
        Command::Status => {
            let reply = daemon::send(&socket, &daemon::Command::Status)?;
            println!("{}", serde_json::to_string_pretty(&reply)?);
            ensure!(reply.ok, "{}", reply.message);
        }
        Command::Apply {
            profile,
            outputs,
            once: false,
        } => {
            let reply = daemon::send(
                &socket,
                &daemon::Command::Apply(daemon::Selection {
                    profile,
                    outputs: mappings(outputs)?,
                }),
            )?;
            ensure!(reply.ok, "{}", reply.message);
            println!("{}", reply.message);
        }
        command => {
            let (profile, outputs, apply) = match command {
                Command::Plan { profile, outputs } => (profile, outputs, false),
                Command::Apply {
                    profile,
                    outputs,
                    once: true,
                } => (profile, outputs, true),
                _ => unreachable!(),
            };
            let _lock = if apply {
                Some(daemon::lock(&socket)?)
            } else {
                None
            };
            let config = config::Config::parse(&std::fs::read_to_string(path)?)?;
            let mut client = ipc::Client::connect(&socket)?;
            let state = client.snapshot()?;
            let (tasks, _) = engine::tasks(
                &config,
                &config.profile(&profile)?,
                &mappings(outputs)?,
                &state,
                engine::Scope::All,
            )?;
            if apply {
                println!("Applied {} changes", engine::apply(&mut client, &tasks)?)
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&engine::preview(&tasks, &state))?
                )
            }
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("niri-layout: {e:#}");
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests;
