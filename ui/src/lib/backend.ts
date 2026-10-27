// One API for the whole UI. Inside Tauri it proxies to the daemon over IPC;
// in a plain browser (`npm run dev`) it runs against an in-memory mock.

import type { DaemonEvent } from "./types";

type EventCb = (e: DaemonEvent) => void;
type ConnCb = (connected: boolean) => void;

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

interface Backend {
  call<T = any>(method: string, params?: unknown): Promise<T>;
  onEvent(cb: EventCb): () => void;
  onConnection(cb: ConnCb): () => void;
  invoke<T = any>(cmd: string, args?: Record<string, unknown>): Promise<T>;
}

function tauriBackend(): Backend {
  const core = import("@tauri-apps/api/core");
  const ev = import("@tauri-apps/api/event");
  return {
    async call(method, params = null) {
      const { invoke } = await core;
      return invoke("daemon_call", { method, params });
    },
    onEvent(cb) {
      let un: (() => void) | undefined;
      let dead = false;
      ev.then(({ listen }) =>
        listen<DaemonEvent>("daemon-event", (e) => cb(e.payload)).then((u) => (dead ? u() : (un = u))),
      );
      return () => {
        dead = true;
        un?.();
      };
    },
    onConnection(cb) {
      let un: (() => void) | undefined;
      let dead = false;
      ev.then(({ listen }) =>
        listen<{ connected: boolean }>("daemon-connection", (e) => cb(e.payload.connected)).then((u) =>
          dead ? u() : (un = u),
        ),
      );
      return () => {
        dead = true;
        un?.();
      };
    },
    async invoke(cmd, args) {
      const { invoke } = await core;
      return invoke(cmd, args);
    },
  };
}

// Cache the promise, not the value: concurrent first calls must share one instance.
let impl: Promise<Backend> | null = null;
function backend(): Promise<Backend> {
  impl ??= inTauri ? Promise.resolve(tauriBackend()) : import("./mock").then((m) => m.mockBackend());
  return impl;
}

export async function call<T = any>(method: string, params: unknown = null): Promise<T> {
  return (await backend()).call<T>(method, params);
}

export function onEvent(cb: EventCb): () => void {
  let un: (() => void) | undefined;
  let dead = false;
  backend().then((b) => {
    if (!dead) un = b.onEvent(cb);
  });
  return () => {
    dead = true;
    un?.();
  };
}

export function onConnection(cb: ConnCb): () => void {
  let un: (() => void) | undefined;
  let dead = false;
  backend().then((b) => {
    if (!dead) un = b.onConnection(cb);
  });
  return () => {
    dead = true;
    un?.();
  };
}

/** Tauri-only commands (window/focus/paths). No-ops in the browser. */
export async function invoke<T = any>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  const b = await backend();
  return b.invoke<T>(cmd, args);
}
