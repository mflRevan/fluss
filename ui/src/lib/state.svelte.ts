// Global reactive state shared by every screen.

import { call, onConnection, onEvent } from "./backend";
import type { Config, InstallProgress, Status } from "./types";

export type Page = "home" | "dictionary" | "shortcuts" | "model" | "audio" | "text" | "output" | "advanced" | "about";

interface Toast {
  id: number;
  kind: "info" | "success" | "error";
  text: string;
}

export const app = $state({
  connected: false,
  status: null as Status | null,
  config: null as Config | null,
  install: null as InstallProgress | null,
  partial: "",
  level: { rms: 0, peak: 0 },
  page: "home" as Page,
  toasts: [] as Toast[],
  rulesVersion: 0,
  historyVersion: 0,
  /** Prefill for the in-app add-rule sheet (from history). */
  ruleDraft: null as { heard: string; target: string } | null,
});

let toastId = 0;
export function toast(text: string, kind: Toast["kind"] = "info", ms = 3200) {
  const id = ++toastId;
  app.toasts.push({ id, kind, text });
  setTimeout(() => {
    const i = app.toasts.findIndex((t) => t.id === id);
    if (i >= 0) app.toasts.splice(i, 1);
  }, ms);
}

function getPath(obj: any, path: string): any {
  return path.split(".").reduce((o, k) => o?.[k], obj);
}

function setPathLocal(obj: any, path: string, value: unknown) {
  const keys = path.split(".");
  let o = obj;
  for (const k of keys.slice(0, -1)) o = o[k];
  o[keys[keys.length - 1]] = value;
}

/** Optimistically change a setting; revert and explain on failure. */
export async function setCfg(path: string, value: unknown) {
  if (!app.config) return;
  const before = getPath(app.config, path);
  if (before === value) return;
  setPathLocal(app.config, path, value);
  try {
    await call("config.set", { path, value });
  } catch (e) {
    setPathLocal(app.config, path, before);
    toast(String(e instanceof Error ? e.message : e), "error");
  }
}

export function cfg<T = any>(path: string): T {
  return getPath(app.config, path);
}

let started = false;
export function startState() {
  if (started) return;
  started = true;
  onConnection(async (c) => {
    app.connected = c;
    if (c) await refresh();
  });
  onEvent((e) => {
    switch (e.event) {
      case "status":
        app.status = e.data;
        break;
      case "config":
        app.config = e.data;
        break;
      case "install":
        app.install = e.data;
        if (e.data?.error) toast(e.data.error, "error", 6000);
        break;
      case "partial":
        app.partial = e.data?.text ?? "";
        break;
      case "level":
        app.level = e.data;
        break;
      case "dictation":
        app.partial = "";
        app.historyVersion++;
        break;
      case "rules":
        app.rulesVersion++;
        break;
      case "notice":
        toast(e.data?.text ?? "", e.data?.kind ?? "info");
        break;
    }
  });
  refresh();
}

export async function refresh() {
  try {
    const [status, config] = await Promise.all([call<Status>("status"), call<Config>("config.get")]);
    app.status = status;
    app.config = config;
    app.connected = true;
  } catch {
    app.connected = false;
  }
}

/** Resolved theme for the document. */
export function applyTheme() {
  const t = app.config?.ui.theme ?? "system";
  const dark = t === "dark" || (t === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.documentElement.dataset.motion = app.config?.ui.reduce_motion ? "reduced" : "full";
}
