//! Local IPC transport (blocking, line-delimited JSON).

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use interprocess::local_socket::{prelude::*, GenericFilePath, GenericNamespaced, ListenerOptions, Name, Stream};
use serde_json::Value;

use crate::protocol::{Message, Request};

pub fn endpoint_name() -> Result<Name<'static>> {
    let base = crate::paths::ipc_name();
    if GenericNamespaced::is_supported() {
        Ok(format!("{base}.sock").to_ns_name::<GenericNamespaced>()?.into_owned())
    } else {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        Ok(dir.join(format!("{base}.sock")).to_fs_name::<GenericFilePath>()?.into_owned())
    }
}

pub fn listen() -> Result<interprocess::local_socket::Listener> {
    let name = endpoint_name()?;
    // A stale socket file (unix) from a crash would block the bind.
    if !GenericNamespaced::is_supported() {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let p = dir.join(format!("{}.sock", crate::paths::ipc_name()));
        if Stream::connect(name.borrow()).is_err() {
            let _ = std::fs::remove_file(p);
        }
    }
    ListenerOptions::new().name(name).create_sync().context("another Fluss daemon is already running")
}

/// A connection to the daemon.
pub struct Client {
    reader: BufReader<Stream>,
    next_id: u64,
}

impl Client {
    pub fn connect() -> Result<Self> {
        let s = Stream::connect(endpoint_name()?).map_err(|_| anyhow!("Fluss is not running"))?;
        Ok(Self { reader: BufReader::new(s), next_id: 1 })
    }

    /// Connect, starting nothing. Retries for up to `wait` (daemon just launched).
    pub fn connect_wait(wait: Duration) -> Result<Self> {
        let deadline = std::time::Instant::now() + wait;
        loop {
            match Self::connect() {
                Ok(c) => return Ok(c),
                Err(e) if std::time::Instant::now() >= deadline => return Err(e),
                Err(_) => std::thread::sleep(Duration::from_millis(60)),
            }
        }
    }

    fn send(&mut self, method: &str, params: Value) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&Request { id, method: method.into(), params })?;
        line.push(b'\n');
        self.reader.get_mut().write_all(&line)?;
        self.reader.get_mut().flush()?;
        Ok(id)
    }

    pub fn read_message(&mut self) -> Result<Message> {
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            bail!("Fluss closed the connection");
        }
        Ok(serde_json::from_str(&line)?)
    }

    /// Call a method and wait for its response (events received meanwhile are dropped).
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.send(method, params)?;
        loop {
            if let Message::Response { id: rid, result, error } = self.read_message()? {
                if rid != id {
                    continue;
                }
                return match error {
                    Some(e) => Err(anyhow!(e)),
                    None => Ok(result),
                };
            }
        }
    }

    /// [`Client::call`] that gives up after `timeout` (a hung daemon must not hang the caller).
    pub fn call_timeout(mut self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        let (tx, rx) = std::sync::mpsc::channel();
        let method = method.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(self.call(&method, params));
        });
        rx.recv_timeout(timeout).map_err(|_| anyhow!("Fluss did not answer within {} s", timeout.as_secs()))?
    }

    /// Subscribe to events; then use [`Client::read_message`] in a loop.
    pub fn subscribe(&mut self) -> Result<()> {
        self.call("subscribe", Value::Null).map(|_| ())
    }
}

/// Serialize one message as a line.
pub fn encode(msg: &Message) -> Vec<u8> {
    let mut v = serde_json::to_vec(msg).unwrap_or_default();
    v.push(b'\n');
    v
}
