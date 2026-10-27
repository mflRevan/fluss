//! Local IPC server. Each connection gets a reader thread (requests -> the
//! controller) and a writer thread fed by a bounded queue, so a slow or stuck
//! client can never block the controller: its events are simply dropped.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Sender, TrySendError};
use fluss_core::ipc::encode;
use fluss_core::protocol::{Message, Request};
use interprocess::local_socket::{prelude::*, Listener, Stream};
use parking_lot::Mutex;
use serde_json::Value;

use crate::app::Ev;

struct Conn {
    tx: Sender<Vec<u8>>,
    subscribed: bool,
    levels: bool,
}

#[derive(Default)]
pub struct Hub {
    conns: Mutex<HashMap<u64, Conn>>,
    level_subs: AtomicUsize,
}

impl Hub {
    fn send_to(&self, conn: u64, msg: &Message) {
        let bytes = encode(msg);
        if let Some(c) = self.conns.lock().get(&conn) {
            let _ = c.tx.try_send(bytes);
        }
    }

    pub fn broadcast(&self, event: &str, data: Value) {
        let bytes = encode(&Message::Event { event: event.into(), data });
        for c in self.conns.lock().values() {
            if c.subscribed {
                if let Err(TrySendError::Full(_)) = c.tx.try_send(bytes.clone()) {
                    log::debug!("ipc client is slow; dropped `{event}`");
                }
            }
        }
    }

    pub fn broadcast_levels(&self, data: Value) {
        let bytes = encode(&Message::Event { event: "level".into(), data });
        for c in self.conns.lock().values() {
            if c.levels {
                let _ = c.tx.try_send(bytes.clone());
            }
        }
    }

    pub fn wants_levels(&self) -> bool {
        self.level_subs.load(Ordering::Relaxed) > 0
    }

    fn recount(&self) {
        let n = self.conns.lock().values().filter(|c| c.levels).count();
        self.level_subs.store(n, Ordering::Relaxed);
    }
}

/// Answers one request; can be moved to another thread for slow work.
pub struct Replier {
    hub: Arc<Hub>,
    conn: u64,
    id: u64,
}

impl Replier {
    pub fn ok(self, result: Value) {
        self.hub.send_to(self.conn, &Message::Response { id: self.id, result, error: None });
    }

    pub fn err(self, msg: &str) {
        self.hub.send_to(self.conn, &Message::Response { id: self.id, result: Value::Null, error: Some(msg.to_string()) });
    }

    pub fn subscribe(&self, levels: bool) {
        if let Some(c) = self.hub.conns.lock().get_mut(&self.conn) {
            c.subscribed = true;
            c.levels = levels;
        }
        self.hub.recount();
    }
}

pub struct IpcCall {
    pub method: String,
    pub params: Value,
    pub reply: Replier,
}

pub fn serve(listener: Listener, hub: Arc<Hub>, ev: Sender<Ev>) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    std::thread::Builder::new()
        .name("fluss-ipc".into())
        .spawn(move || {
            for conn in listener.incoming() {
                let stream = match conn {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("ipc accept: {e}");
                        continue;
                    }
                };
                let id = NEXT.fetch_add(1, Ordering::Relaxed);
                handle(stream, id, hub.clone(), ev.clone());
            }
        })
        .expect("spawn ipc thread");
}

fn handle(stream: Stream, id: u64, hub: Arc<Hub>, ev: Sender<Ev>) {
    let stream = Arc::new(stream);
    let recv = stream.clone();
    let (tx, rx) = crossbeam_channel::bounded::<Vec<u8>>(512);
    hub.conns.lock().insert(id, Conn { tx, subscribed: false, levels: false });
    std::thread::spawn(move || {
        let mut send: &Stream = &stream;
        while let Ok(b) = rx.recv() {
            if send.write_all(&b).and_then(|_| send.flush()).is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        let mut reader = BufReader::new(&*recv);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let req: Request = match serde_json::from_str(line.trim()) {
                Ok(r) => r,
                Err(e) => {
                    hub.send_to(id, &Message::Response { id: 0, result: Value::Null, error: Some(format!("bad request: {e}")) });
                    continue;
                }
            };
            let reply = Replier { hub: hub.clone(), conn: id, id: req.id };
            if ev.send(Ev::Ipc(IpcCall { method: req.method, params: req.params, reply })).is_err() {
                break;
            }
        }
        hub.conns.lock().remove(&id);
        hub.recount();
    });
}
