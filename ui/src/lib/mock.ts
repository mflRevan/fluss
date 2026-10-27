// In-memory stand-in for the daemon so the UI runs in a plain browser.
// Mirrors the real methods closely enough to design and test every screen.

import type {
  Config,
  DaemonEvent,
  HistoryItem,
  ModelInfo,
  PopupContext,
  Rule,
  SearchHit,
  Status,
} from "./types";

const defaults: Config = {
  general: { autostart: true, autostart_delay_s: 4, onboarded: true, sounds: true, sound_volume: 45, history: true },
  model: {
    id: "phonon-2.5-en",
    device: "auto",
    residency: "always",
    idle_unload_min: 20,
    load_priority: "low",
    threads: 0,
    language: "auto",
    hotwords: true,
  },
  custom: {
    kind: "openai-transcriptions",
    base_url: "",
    api_key: "",
    model: "",
    live_preview: false,
    word_timestamps: true,
    timeout_s: 30,
  },
  audio: {
    device: "",
    preroll_ms: 500,
    postroll_ms: 500,
    keep_mic_open: true,
    pause_ms: 700,
    max_phrase_s: 28,
    preview_interval_ms: 450,
    gain: 1,
  },
  hotkeys: {
    push_to_talk: "Ctrl+Win",
    hands_free: "Ctrl+Win+Space",
    add_rule: "Ctrl+Win+R",
    cancel: "Escape",
    double_tap_lock: true,
    double_tap_ms: 320,
  },
  output: {
    method: "type",
    newline: "shift-enter",
    continuation: true,
    continuation_window_s: 600,
    seam_repair: true,
    smart_spacing: true,
  },
  text: {
    casing: "natural",
    spoken_punctuation: true,
    remove_fillers: true,
    drop_final_period: false,
    apply_rules: true,
  },
  ui: {
    theme: "system",
    overlay: true,
    overlay_position: "bottom",
    overlay_preview: true,
    keep_warm: false,
    reduce_motion: false,
  },
};

const catalog: ModelInfo[] = [
  {
    id: "phonon-2.5-en",
    name: "Phonon 2.5 English",
    vendor: "Fermion Research",
    engine: "phonon",
    tagline: "Tiny, fast and accurate. The best default for English.",
    languages: "English",
    multilingual: false,
    download_mb: 158,
    memory_mb: 1500,
    gpu: false,
    recommended: true,
    source: "phonon-2.5-en",
    installed: true,
  },
  {
    id: "phonon-2.5",
    name: "Phonon 2.5 Multilingual",
    vendor: "Fermion Research",
    engine: "phonon",
    tagline: "25 European languages in the same tiny package.",
    languages: "25 languages (bg, cs, da, de, el, en, es, et, fi, fr, hr, hu, it, lt, lv, mt, nl, pl, pt, ro, ru, sk, sl, sv, uk)",
    multilingual: true,
    download_mb: 170,
    memory_mb: 1600,
    gpu: false,
    recommended: false,
    source: "phonon-2.5",
    installed: false,
  },
  {
    id: "parakeet-v3",
    name: "Parakeet TDT 0.6B v3",
    vendor: "NVIDIA",
    engine: "parakeet",
    tagline: "The full-size original. Runs on your GPU and stays in VRAM.",
    languages: "25 languages",
    multilingual: true,
    download_mb: 2560,
    memory_mb: 2600,
    gpu: true,
    recommended: false,
    source: "istupakov/parakeet-tdt-0.6b-v3-onnx",
    installed: false,
  },
  {
    id: "custom",
    name: "Custom endpoint",
    vendor: "You",
    engine: "custom",
    tagline: "Any OpenAI-compatible /audio/transcriptions server, local or online.",
    languages: "Depends on the server",
    multilingual: true,
    download_mb: 0,
    memory_mb: 0,
    gpu: false,
    recommended: false,
    source: "",
    installed: true,
  },
];

const now = Math.floor(Date.now() / 1000);
let rules: Rule[] = [
  { target: "Omarchy", variants: ["on march 9", "oh marchy"], created: now - 3600, uses: 14 },
  { target: "Hyprland", variants: ["hyper land", "hyperland"], created: now - 7200, uses: 31 },
  { target: "Kubernetes", variants: ["cooper netties", "cube or nettys"], created: now - 86400, uses: 9 },
  { target: "Fluss", variants: ["floss", "fluff"], created: now - 600, uses: 52 },
  { target: "GitHub", variants: [], created: now - 90000, uses: 77 },
  { target: "Tauri", variants: ["tory", "towery"], created: now - 50000, uses: 6 },
  { target: "PostgreSQL", variants: ["postgres q l", "post gress"], created: now - 40000, uses: 12 },
  { target: "Svelte", variants: ["svelt", "swelled"], created: now - 30000, uses: 18 },
  { target: "Anthropic", variants: ["and tropic"], created: now - 20000, uses: 4 },
  { target: "nginx", variants: ["engine x", "engine ex"], created: now - 10000, uses: 3 },
  { target: "Wispr", variants: ["whisper flow"], created: now - 5000, uses: 2 },
  { target: "mflRevan", variants: ["m f l revan", "mfl reven"], created: now - 1000, uses: 1 },
];

const history: HistoryItem[] = [
  {
    id: 6,
    at: Date.now() - 40_000,
    text: "Hey team, quick update on the release. The build passed this morning, so we can ship on Thursday.",
    raw: "Hey team, quick update on the release. The build passed this morning, so we can ship on Thursday.",
    app: "Slack",
    duration_s: 6.2,
    latency_ms: 84,
  },
  {
    id: 5,
    at: Date.now() - 4 * 60_000,
    text: "I switched my laptop to Omarchy last week and Hyprland feels great.",
    raw: "I switched my laptop to on March 9 last week and hyper land feels great.",
    app: "Discord",
    duration_s: 4.1,
    latency_ms: 77,
  },
  {
    id: 4,
    at: Date.now() - 25 * 60_000,
    text: "Can you review the Kubernetes manifests before lunch?",
    raw: "Can you review the cooper netties manifests before lunch?",
    app: "Outlook",
    duration_s: 3.3,
    latency_ms: 69,
  },
  {
    id: 3,
    at: Date.now() - 2 * 3600_000,
    text: "Remember to update the PostgreSQL connection string in the staging config.",
    raw: "Remember to update the post gress connection string in the staging config.",
    app: "Visual Studio Code",
    duration_s: 4.8,
    latency_ms: 91,
  },
];

function deepClone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v));
}

function norm(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s'-]/gu, "")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
}

function score(s: string, q: string): number | null {
  if (s === q) return 100;
  if (s.startsWith(q)) return 80;
  if (s.split(/\s+/).some((w) => w.startsWith(q))) return 65;
  if (s.includes(q)) return 50;
  let pos = 0;
  let first = -1;
  for (const c of q.replace(/\s/g, "")) {
    const f = s.indexOf(c, pos);
    if (f < 0) return null;
    if (first < 0) first = f;
    pos = f + 1;
  }
  return 5 + Math.min(30, Math.floor((q.length * 30) / Math.max(1, pos - first)));
}

function search(query: string, limit = 50): SearchHit[] {
  const q = query.trim().toLowerCase();
  const hits: SearchHit[] = [];
  for (const r of rules) {
    if (!q) {
      hits.push({ rule: deepClone(r), score: 0, matched_variant: null });
      continue;
    }
    let best = score(r.target.toLowerCase(), q);
    if (best !== null) best += 20;
    let mv: string | null = null;
    for (const v of r.variants) {
      const s = score(v.toLowerCase(), q);
      if (s !== null && (best === null || s > best)) {
        best = s;
        mv = v;
      }
    }
    if (best !== null) hits.push({ rule: deepClone(r), score: best, matched_variant: mv });
  }
  if (!q) hits.sort((a, b) => b.rule.created - a.rule.created);
  else hits.sort((a, b) => b.score - a.score || a.rule.target.length - b.rule.target.length);
  return hits.slice(0, limit);
}

function preview(text: string, cfg: Config): string {
  let t = " " + text + " ";
  if (cfg.text.apply_rules) {
    const pairs: [string, string][] = [];
    for (const r of rules) for (const v of [...r.variants, r.target]) pairs.push([norm(v), r.target]);
    pairs.sort((a, b) => b[0].length - a[0].length);
    for (const [v, target] of pairs) {
      if (!v) continue;
      const re = new RegExp(`(?<=\\s|^)${v.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replace(/ /g, "[\\s,]+")}(?=[\\s.,!?;:]|$)`, "gi");
      t = t.replace(re, target);
    }
  }
  if (cfg.text.spoken_punctuation) {
    const map: [RegExp, string][] = [
      [/[,.]?\s+new paragraph[,.]?\s*/gi, "\n\n"],
      [/[,.]?\s+new line[,.]?\s*/gi, "\n"],
      [/[,.]?\s+question mark[,.]?/gi, "?"],
      [/[,.]?\s+exclamation (mark|point)[,.]?/gi, "!"],
      [/[,.]?\s+comma[,.]?/gi, ","],
      [/[,.]?\s+(full stop|period)[,.]?/gi, "."],
      [/[,.]?\s+colon[,.]?/gi, ":"],
      [/[,.]?\s+semicolon[,.]?/gi, ";"],
    ];
    for (const [re, s] of map) t = t.replace(re, s);
  }
  if (cfg.text.remove_fillers) t = t.replace(/\b(um+|uh+|uhm|erm|hmm)\b[,.]?\s*/gi, "");
  t = t.replace(/\s+([,.!?;:])/g, "$1").replace(/[ \t]+/g, " ").trim();
  t = t.replace(/(^|[.!?]\s+|\n)(\p{Ll})/gu, (_m, a, b) => a + b.toUpperCase());
  if (cfg.text.casing === "lowercase")
    t = t
      .split(" ")
      .map((w) => (rules.some((r) => r.target === w.replace(/[^\p{L}\p{N}]/gu, "")) || /^[A-Z]{2,}\W*$/.test(w) ? w : w.toLowerCase()))
      .join(" ");
  if (cfg.text.drop_final_period) t = t.replace(/\.$/, "");
  return t;
}

export function mockBackend() {
  const listeners = new Set<(e: DaemonEvent) => void>();
  const connListeners = new Set<(c: boolean) => void>();
  const cfg = deepClone(defaults);
  const params = new URLSearchParams(location.search);
  if (params.has("onboarding")) cfg.general.onboarded = false;
  const start = Date.now();
  const status: Status = {
    version: "0.1.0",
    pid: 4242,
    uptime_s: 0,
    phase: "idle",
    model: {
      id: "phonon-2.5-en",
      name: "Phonon 2.5 English",
      phase: "ready",
      progress: null,
      detail: "",
      device: "CPU · AVX-512",
      memory_mb: 1488,
      unload_in_s: null,
      error: null,
    },
    mic: { device: "SteelSeries Sonar - Microphone", open: true, sample_rate: 48000, error: null },
    stats: { dictations: 1284, words: 48211, audio_seconds: 15230, median_latency_ms: 81, last_latency_ms: 77 },
    onboarded: cfg.general.onboarded,
    config_path: "C:\\Users\\you\\AppData\\Roaming\\Fluss\\config.toml",
    data_path: "C:\\Users\\you\\AppData\\Local\\Fluss",
    hotkeys: { push_to_talk: "Ctrl+Win", hands_free: "Ctrl+Win+Space", add_rule: "Ctrl+Win+R", paused: false },
  };
  if (params.has("phase")) status.model.phase = params.get("phase") as any;
  if (params.has("listening")) {
    status.phase = "listening";
    setTimeout(() => emit("partial", { text: "Quick note for later, the new build looks really good and I think" }), 300);
  }

  const emit = (event: string, data: unknown) => listeners.forEach((l) => l({ event, data }));
  const emitStatus = () => {
    status.uptime_s = Math.floor((Date.now() - start) / 1000);
    status.onboarded = cfg.general.onboarded;
    status.hotkeys.push_to_talk = cfg.hotkeys.push_to_talk;
    status.hotkeys.hands_free = cfg.hotkeys.hands_free;
    status.hotkeys.add_rule = cfg.hotkeys.add_rule;
    emit("status", deepClone(status));
  };

  // Fake mic levels: speech-like bursts.
  let t = 0;
  setInterval(() => {
    t += 0.04;
    const speech = Math.max(0, Math.sin(t * 1.3) * Math.sin(t * 3.7 + 1) + 0.15 * Math.sin(t * 17));
    const rms = Math.min(1, 0.02 + speech * 0.35 + Math.random() * 0.04);
    emit("level", { rms, peak: Math.min(1, rms * 1.6) });
  }, 40);

  // A dictation cycle every ~14 s so the hero animates.
  if (!params.has("still")) {
    setInterval(() => {
      if (status.model.phase !== "ready") return;
      status.phase = "listening";
      emitStatus();
      const words = "Quick note for later, the new build looks really good".split(" ");
      words.forEach((_, i) => setTimeout(() => emit("partial", { text: words.slice(0, i + 1).join(" ") }), 250 * (i + 1)));
      setTimeout(() => {
        status.phase = "finishing";
        emitStatus();
      }, 250 * words.length + 300);
      setTimeout(() => {
        status.phase = "idle";
        status.stats.dictations++;
        status.stats.words += words.length;
        const item: HistoryItem = {
          id: Date.now(),
          at: Date.now(),
          text: "Quick note for later, the new build looks really good.",
          raw: "Quick note for later, the new build looks really good.",
          app: "Notepad",
          duration_s: 3.1,
          latency_ms: 70 + Math.round(Math.random() * 30),
        };
        if (cfg.general.history) history.unshift(item);
        status.stats.last_latency_ms = item.latency_ms;
        emit("dictation", item);
        emitStatus();
      }, 250 * words.length + 700);
    }, 14000);
  }

  function install(id: string) {
    const m = catalog.find((x) => x.id === id);
    if (!m) throw new Error(`unknown model ${id}`);
    const steps = ["Preparing runtime", "Downloading model", "Verifying", "Warming up"];
    let p = 0;
    status.model.phase = "installing";
    status.model.id = id;
    status.model.name = m.name;
    const timer = setInterval(() => {
      p += 0.025;
      const step = steps[Math.min(steps.length - 1, Math.floor(p * steps.length))];
      status.model.progress = Math.min(1, p);
      status.model.detail = step;
      emit("install", { id, step, progress: Math.min(1, p), done: p >= 1, error: null });
      if (p >= 1) {
        clearInterval(timer);
        m.installed = true;
        status.model.phase = "loading";
        status.model.detail = "Loading into memory";
        status.model.progress = null;
        emitStatus();
        setTimeout(() => {
          status.model.phase = "ready";
          status.model.detail = "";
          status.model.device = m.engine === "parakeet" ? "GPU · DirectML" : m.engine === "custom" ? "Remote" : "CPU · AVX-512";
          emitStatus();
        }, 1400);
      } else emitStatus();
    }, 120);
  }

  const handlers: Record<string, (p: any) => any> = {
    status: () => (emitStatus(), deepClone(status)),
    subscribe: () => true,
    "config.get": (p) => {
      if (!p?.path) return deepClone(cfg);
      return p.path.split(".").reduce((o: any, k: string) => o?.[k], cfg);
    },
    "config.set": (p) => {
      const keys = p.path.split(".");
      let o: any = cfg;
      for (const k of keys.slice(0, -1)) o = o[k];
      const last = keys[keys.length - 1];
      if (!(last in o)) throw new Error(`unknown setting \`${p.path}\``);
      if (p.path.startsWith("hotkeys.") && typeof p.value === "string" && !/^[\w+\s]+$/.test(p.value))
        throw new Error(`invalid value for \`${p.path}\``);
      o[last] = p.value;
      emit("config", deepClone(cfg));
      emitStatus();
      return deepClone(cfg);
    },
    "config.merge": (p) => {
      const merge = (d: any, s: any) => {
        for (const k in s) typeof s[k] === "object" && s[k] && !Array.isArray(s[k]) ? merge(d[k], s[k]) : (d[k] = s[k]);
      };
      merge(cfg, p.patch);
      emit("config", deepClone(cfg));
      emitStatus();
      return deepClone(cfg);
    },
    "config.reset": () => {
      const onboarded = cfg.general.onboarded;
      Object.assign(cfg, deepClone(defaults));
      cfg.general.onboarded = onboarded;
      emit("config", deepClone(cfg));
      return deepClone(cfg);
    },
    "rules.list": (p) => search(p?.query ?? "", p?.limit ?? 200),
    "rules.upsert": (p) => {
      const target = String(p.target).trim();
      const moved: [string, string][] = [];
      const existing: string[] = [];
      const clean = (p.variants as string[]).map((v) => v.trim()).filter((v) => norm(v) && norm(v) !== norm(target));
      for (const r of rules) {
        if (r.target.toLowerCase() === target.toLowerCase()) continue;
        r.variants = r.variants.filter((v) => {
          const hit = clean.some((c) => norm(c) === norm(v));
          if (hit) moved.push([v, r.target]);
          return !hit;
        });
      }
      let rule = rules.find((r) => r.target.toLowerCase() === target.toLowerCase());
      const created = !rule;
      if (!rule) {
        rule = { target, variants: [], created: Math.floor(Date.now() / 1000), uses: 0 };
        rules.push(rule);
      }
      rule.target = target;
      for (const v of clean) {
        if (rule.variants.some((x) => norm(x) === norm(v))) existing.push(v);
        else rule.variants.push(v);
      }
      emit("rules", { count: rules.length });
      if (p.apply_last) emit("notice", { kind: "success", text: `Fixed last dictation → ${target}` });
      return { rule: deepClone(rule), created, moved, existing };
    },
    "rules.replace": (p) => {
      rules = rules.filter((r) => r.target.toLowerCase() !== String(p.old_target).toLowerCase());
      rules.push({ ...p.rule, created: p.rule.created || Math.floor(Date.now() / 1000) });
      emit("rules", { count: rules.length });
      return true;
    },
    "rules.delete": (p) => {
      const n = rules.length;
      rules = rules.filter((r) => r.target.toLowerCase() !== String(p.target).toLowerCase());
      emit("rules", { count: rules.length });
      return rules.length !== n;
    },
    "rules.delete_variant": (p) => {
      const r = rules.find((r) => r.target === p.target);
      if (r) r.variants = r.variants.filter((v) => norm(v) !== norm(p.variant));
      emit("rules", { count: rules.length });
      return true;
    },
    "rules.preview": (p) => ({ text: preview(p.text ?? "", cfg) }),
    "model.catalog": () => deepClone(catalog),
    "model.use": (p) => {
      cfg.model.id = p.id;
      const m = catalog.find((x) => x.id === p.id)!;
      if (m.installed) {
        status.model = { ...status.model, id: m.id, name: m.name, phase: "loading", detail: "Loading into memory", progress: null };
        emitStatus();
        setTimeout(() => {
          status.model.phase = "ready";
          status.model.detail = "";
          status.model.device = m.engine === "parakeet" ? "GPU · DirectML" : m.engine === "custom" ? "Remote" : "CPU · AVX-512";
          emitStatus();
        }, 1200);
      } else {
        status.model = { ...status.model, id: m.id, name: m.name, phase: "not-installed", detail: "", progress: null };
        emitStatus();
      }
      emit("config", deepClone(cfg));
      return true;
    },
    "model.install": (p) => (install(p?.id ?? cfg.model.id), true),
    "model.load": () => {
      status.model.phase = "loading";
      emitStatus();
      setTimeout(() => ((status.model.phase = "ready"), emitStatus()), 1200);
      return true;
    },
    "model.unload": () => ((status.model.phase = "unloaded"), (status.model.memory_mb = null), emitStatus(), true),
    "devices.list": () => [
      { name: "SteelSeries Sonar - Microphone", default: true },
      { name: "Microphone (Elgato Wave:3)", default: false },
      { name: "Headset Microphone (Arctis Nova Pro)", default: false },
      { name: "Microphone Array (Realtek Audio)", default: false },
    ],
    "history.list": () => (cfg.general.history ? deepClone(history) : []),
    "history.clear": () => ((history.length = 0), true),
    "popup.context": (): PopupContext => ({
      chunk: { context: ["I", "really", "like", "using", "on", "March", "9"], start: 4, end: 7, text: "on March 9" },
      return_hwnd: 0,
      from_dictation: true,
      can_apply_last: true,
    }),
    "popup.closed": () => true,
    "hotkeys.record": () =>
      new Promise((res) => setTimeout(() => res({ hotkey: ["Ctrl+Shift+Space", "Ctrl+Alt+D", "F9", "RightAlt"][Math.floor(Math.random() * 4)] }), 1800)),
    "hotkeys.pause": () => true,
    "endpoint.test": () =>
      new Promise((res, rej) =>
        setTimeout(
          () => (cfg.custom.base_url ? res({ ok: true, latency_ms: 312, text: "Hello from Fluss." }) : rej(new Error("Set a base URL first"))),
          900,
        ),
      ),
    "autostart.set": (p) => ((cfg.general.autostart = !!p.enabled), true),
    "logs.path": () => "C:\\Users\\you\\AppData\\Local\\Fluss\\logs",
    "daemon.quit": () => true,
    "dictation.start": () => true,
    "dictation.stop": () => true,
    "dictation.cancel": () => true,
  };

  setTimeout(() => connListeners.forEach((c) => c(true)), 50);

  return {
    async call(method: string, params: unknown = null): Promise<any> {
      await new Promise((r) => setTimeout(r, 12));
      const h = handlers[method];
      if (!h) throw new Error(`unknown method ${method}`);
      return await h(params);
    },
    onEvent(cb: (e: DaemonEvent) => void) {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    onConnection(cb: (c: boolean) => void) {
      connListeners.add(cb);
      cb(true);
      return () => connListeners.delete(cb);
    },
    async invoke(cmd: string, args?: Record<string, unknown>): Promise<any> {
      console.debug("[mock invoke]", cmd, args);
      return undefined;
    },
  };
}
