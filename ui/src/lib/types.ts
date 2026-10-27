// Mirrors crates/fluss-core (protocol.rs, config.rs, models.rs, rules.rs, chunk.rs).

export type DictationPhase = "idle" | "listening" | "hands-free" | "finishing";
export type ModelPhase = "not-installed" | "installing" | "unloaded" | "loading" | "ready" | "error";

export interface ModelStatus {
  id: string;
  name: string;
  phase: ModelPhase;
  progress: number | null;
  detail: string;
  device: string;
  memory_mb: number | null;
  unload_in_s: number | null;
  error: string | null;
}

export interface Status {
  version: string;
  pid: number;
  uptime_s: number;
  phase: DictationPhase;
  model: ModelStatus;
  mic: { device: string; open: boolean; sample_rate: number; error: string | null };
  stats: {
    dictations: number;
    words: number;
    audio_seconds: number;
    median_latency_ms: number | null;
    last_latency_ms: number | null;
  };
  onboarded: boolean;
  config_path: string;
  data_path: string;
  hotkeys: { push_to_talk: string; hands_free: string; add_rule: string; paused: boolean };
}

export interface Config {
  general: {
    autostart: boolean;
    autostart_delay_s: number;
    onboarded: boolean;
    sounds: boolean;
    sound_volume: number;
    history: boolean;
  };
  model: {
    id: string;
    device: "auto" | "gpu" | "cpu";
    residency: "always" | "idle-unload" | "on-demand";
    idle_unload_min: number;
    load_priority: "low" | "normal";
    threads: number;
    language: string;
    hotwords: boolean;
  };
  custom: {
    kind: "openai-transcriptions";
    base_url: string;
    api_key: string;
    model: string;
    live_preview: boolean;
    word_timestamps: boolean;
    timeout_s: number;
  };
  audio: {
    device: string;
    preroll_ms: number;
    postroll_ms: number;
    keep_mic_open: boolean;
    pause_ms: number;
    max_phrase_s: number;
    preview_interval_ms: number;
    gain: number;
  };
  hotkeys: {
    push_to_talk: string;
    hands_free: string;
    add_rule: string;
    cancel: string;
    double_tap_lock: boolean;
    double_tap_ms: number;
  };
  output: {
    method: "type" | "paste";
    newline: "shift-enter" | "enter";
    continuation: boolean;
    continuation_window_s: number;
    seam_repair: boolean;
    smart_spacing: boolean;
  };
  text: {
    casing: "natural" | "lowercase";
    spoken_punctuation: boolean;
    remove_fillers: boolean;
    drop_final_period: boolean;
    apply_rules: boolean;
  };
  ui: {
    theme: "system" | "light" | "dark";
    overlay: boolean;
    overlay_position: "bottom" | "top";
    overlay_preview: boolean;
    keep_warm: boolean;
    reduce_motion: boolean;
  };
}

export interface ModelInfo {
  id: string;
  name: string;
  vendor: string;
  engine: "phonon" | "parakeet" | "custom";
  tagline: string;
  languages: string;
  multilingual: boolean;
  download_mb: number;
  memory_mb: number;
  gpu: boolean;
  recommended: boolean;
  source: string;
  installed: boolean;
}

export interface Rule {
  target: string;
  variants: string[];
  created: number;
  uses: number;
}

export interface SearchHit {
  rule: Rule;
  score: number;
  matched_variant: string | null;
}

export interface UpsertOutcome {
  rule: Rule;
  created: boolean;
  moved: [string, string][];
  existing: string[];
}

export interface ChunkGuess {
  context: string[];
  start: number;
  end: number;
  text: string;
}

export interface PopupContext {
  chunk: ChunkGuess;
  return_hwnd: number;
  from_dictation: boolean;
  can_apply_last: boolean;
}

export interface HistoryItem {
  id: number;
  at: number;
  text: string;
  raw: string;
  app: string;
  duration_s: number;
  latency_ms: number;
}

export interface InstallProgress {
  id: string;
  step: string;
  progress: number | null;
  done: boolean;
  error: string | null;
}

export interface DeviceInfo {
  name: string;
  default: boolean;
}

export interface DaemonEvent {
  event: string;
  data: any;
}
