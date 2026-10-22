use crate::{
    config::Config,
    engine::{self, Scope},
    ipc::Client,
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use niri_ipc::{Event, Request, Response};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub profile: String,
    pub outputs: BTreeMap<String, String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Apply(Selection),
    Status,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    pub message: String,
    pub selection: Option<Selection>,
}
fn sidecar(socket: &Path, suffix: &str) -> PathBuf {
    let mut s = socket.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}
pub fn lock(socket: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(sidecar(socket, ".niri-layout.lock"))?;
    file.try_lock_exclusive()
        .context("another niri-layout process is active; send apply to its daemon instead")?;
    Ok(file)
}
pub fn send(socket: &Path, command: &Command) -> Result<Reply> {
    let mut stream = UnixStream::connect(sidecar(socket, ".niri-layout.sock"))
        .context("niri-layout daemon is not running; start it or use apply --once")?;
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    serde_json::to_writer(&mut stream, command)?;
    stream.write_all(b"\n")?;
    let mut line = String::new();
    BufReader::new(stream).take(65536).read_line(&mut line)?;
    serde_json::from_str(&line).context("invalid companion response")
}
enum Message {
    Event(Box<Event>),
    Control(Command, mpsc::Sender<Reply>),
    Disconnected(String),
}
struct SocketCleanup(PathBuf);
impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn run(config_path: PathBuf, socket: PathBuf) -> Result<()> {
    let _lock = lock(&socket)?;
    let path = sidecar(&socket, ".niri-layout.sock");
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let _cleanup = SocketCleanup(path);
    let (tx, rx) = mpsc::channel();
    let controls = tx.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let result = (|| -> Result<()> {
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                let mut line = String::new();
                BufReader::new(stream.try_clone()?)
                    .take(65536)
                    .read_line(&mut line)?;
                let cmd = serde_json::from_str(&line)?;
                let (reply_tx, reply_rx) = mpsc::channel();
                controls.send(Message::Control(cmd, reply_tx))?;
                let reply = reply_rx.recv_timeout(Duration::from_secs(55))?;
                serde_json::to_writer(&mut stream, &reply)?;
                stream.write_all(b"\n")?;
                Ok(())
            })();
            if let Err(e) = result {
                eprintln!("control request failed: {e:#}");
            }
        }
    });
    let event_socket = socket.clone();
    thread::spawn(move || {
        let result = (|| -> Result<()> {
            let mut connection = niri_ipc::socket::Socket::connect_to(event_socket)?;
            ensure!(
                matches!(
                    connection.send(Request::EventStream)?,
                    Ok(Response::Handled)
                ),
                "event stream rejected"
            );
            let mut next = connection.read_events();
            loop {
                let event = next()?;
                if matches!(
                    event,
                    Event::WindowOpenedOrChanged { .. }
                        | Event::WindowsChanged { .. }
                        | Event::WindowClosed { .. }
                        | Event::WorkspacesChanged { .. }
                        | Event::WindowFocusChanged { id: Some(_) }
                ) {
                    if tx.send(Message::Event(Box::new(event))).is_err() {
                        return Ok(());
                    }
                }
            }
        })();
        if let Err(e) = result {
            let _ = tx.send(Message::Disconnected(format!("{e:#}")));
        }
    });
    let mut selected: Option<(Selection, Config)> = None;
    let mut managed = BTreeSet::new();
    let mut deferred_full = false;
    eprintln!("niri-layout ready; waiting for a Kanshi apply hook");
    for message in rx {
        match message {
            Message::Disconnected(e) => anyhow::bail!(
                "niri event stream disconnected: {e}; restart companion with the niri session"
            ),
            Message::Control(Command::Status, reply) => {
                let _ = reply.send(Reply {
                    ok: true,
                    message: format!(
                        "{} managed windows{}",
                        managed.len(),
                        if deferred_full {
                            "; waiting for window focus"
                        } else {
                            ""
                        }
                    ),
                    selection: selected.as_ref().map(|(s, _)| s.clone()),
                });
            }
            Message::Control(Command::Apply(selection), reply) => {
                let result = (|| -> Result<usize> {
                    let config = Config::parse(&fs::read_to_string(&config_path)?)?;
                    let profile = config.profile(&selection.profile)?;
                    let mut client = Client::connect(&socket)?;
                    let state = client.snapshot()?;
                    let (tasks, claimed) =
                        engine::tasks(&config, &profile, &selection.outputs, &state, Scope::All)?;
                    // A layer-shell panel can own focus during hotplug/login. Keep the
                    // requested profile pending until a window receives focus.
                    if !claimed.is_empty() && !state.windows.iter().any(|w| w.is_focused) {
                        selected = Some((selection, config));
                        managed.clear();
                        deferred_full = true;
                        return Ok(0);
                    }
                    deferred_full = false;
                    // Once mutation starts, a failed switch must not resume the old profile.
                    selected = None;
                    managed.clear();
                    let count = engine::apply(&mut client, &tasks)?;
                    managed = claimed;
                    selected = Some((selection, config));
                    Ok(count)
                })();
                let (ok, message) = match result {
                    Ok(n) => (
                        true,
                        if deferred_full {
                            "profile selected; waiting for window focus".into()
                        } else {
                            format!("applied {n} changes")
                        },
                    ),
                    Err(e) => (false, format!("{e:#}")),
                };
                let _ = reply.send(Reply {
                    ok,
                    message,
                    selection: selected.as_ref().map(|(s, _)| s.clone()),
                });
            }
            Message::Event(event) => {
                if let Event::WindowClosed { id } = *event {
                    managed.remove(&id);
                    continue;
                }
                let Some((selection, config)) = &selected else {
                    continue;
                };
                let result = (|| -> Result<()> {
                    if matches!(*event,Event::WindowOpenedOrChanged {ref window} if managed.contains(&window.id))
                        && !deferred_full
                    {
                        return Ok(());
                    }
                    let profile = config.profile(&selection.profile)?;
                    let mut client = Client::connect(&socket)?;
                    let state = client.snapshot()?;
                    let ids: BTreeSet<u64> = match *event {
                        Event::WindowOpenedOrChanged { ref window }
                            if !managed.contains(&window.id) =>
                        {
                            [window.id].into_iter().collect()
                        }
                        Event::WindowsChanged { ref windows } => windows
                            .iter()
                            .filter(|w| !managed.contains(&w.id))
                            .map(|w| w.id)
                            .collect(),
                        Event::WindowFocusChanged { id: Some(_) } => state
                            .windows
                            .iter()
                            .filter(|w| !managed.contains(&w.id))
                            .map(|w| w.id)
                            .collect(),
                        Event::WorkspacesChanged { .. }
                            if profile.enforce_order.unwrap_or(false) =>
                        {
                            BTreeSet::new()
                        }
                        _ => return Ok(()),
                    };
                    if deferred_full && !state.windows.iter().any(|w| w.is_focused) {
                        return Ok(());
                    }
                    if ids.is_empty()
                        && !deferred_full
                        && !matches!(*event, Event::WorkspacesChanged { .. })
                    {
                        return Ok(());
                    }
                    let scope = if deferred_full {
                        Scope::All
                    } else if ids.is_empty() {
                        Scope::OrderOnly
                    } else {
                        Scope::New(&ids)
                    };
                    let (tasks, claimed) =
                        engine::tasks(config, &profile, &selection.outputs, &state, scope)?;
                    let count = engine::apply(&mut client, &tasks)?;
                    if deferred_full {
                        managed = claimed;
                        deferred_full = false;
                    } else {
                        managed.extend(claimed.intersection(&ids));
                    }
                    if count > 0 {
                        eprintln!("{}: {count} changes", selection.profile);
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    eprintln!("layout event: {e:#}");
                }
            }
        }
    }
    Ok(())
}
