use anyhow::{Context, Result, bail};
use niri_ipc::{Action, Output, Reply, Request, Response, Window, Workspace};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

pub struct Client {
    reader: BufReader<UnixStream>,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub outputs: HashMap<String, Output>,
    pub workspaces: Vec<Workspace>,
    pub windows: Vec<Window>,
}
impl Client {
    pub fn connect(path: &Path) -> Result<Self> {
        let socket = UnixStream::connect(path)
            .with_context(|| format!("cannot connect to niri at {}", path.display()))?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self {
            reader: BufReader::new(socket),
        })
    }
    pub fn request(&mut self, request: Request) -> Result<Response> {
        serde_json::to_writer(self.reader.get_mut(), &request)?;
        self.reader.get_mut().write_all(b"\n")?;
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .context("reading niri reply")?;
        let reply: Reply = serde_json::from_str(&line).context("invalid niri reply")?;
        reply.map_err(|e| anyhow::anyhow!("niri rejected request: {e}"))
    }
    pub fn action(&mut self, a: Action) -> Result<()> {
        match self.request(Request::Action(a))? {
            Response::Handled => Ok(()),
            _ => bail!("unexpected niri action response"),
        }
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let Response::Outputs(outputs) = self.request(Request::Outputs)? else {
            bail!("expected Outputs")
        };
        let Response::Workspaces(workspaces) = self.request(Request::Workspaces)? else {
            bail!("expected Workspaces")
        };
        let Response::Windows(windows) = self.request(Request::Windows)? else {
            bail!("expected Windows")
        };
        Ok(Snapshot {
            outputs,
            workspaces,
            windows,
        })
    }
}
