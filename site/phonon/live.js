// "Try with your mic": streams the microphone into Phonon 2.5 English running in a
// worker on WebGPU and shows the transcript as it forms. Nothing is uploaded; the
// only network traffic is the one-time model download from Hugging Face.

const SR = 16000;
const SHOWN = 280; // characters of finished text kept on screen

let worker = null;
let state = "idle"; // idle | loading | ready | failed

const TAP = `
class Tap extends AudioWorkletProcessor {
  constructor() { super(); this.buf = new Float32Array(2048); this.n = 0; }
  process(inputs) {
    const ch = inputs[0] && inputs[0][0];
    if (ch) for (let i = 0; i < ch.length; i++) {
      this.buf[this.n++] = ch[i];
      if (this.n === this.buf.length) { this.port.postMessage(this.buf.slice(0)); this.n = 0; }
    }
    return true;
  }
}
registerProcessor("fluss-tap", Tap);`;

/** Linear resampler to 16 kHz with a box pre-filter (enough for speech recognition). */
function resampler(from) {
  if (from === SR) return (x) => x;
  const ratio = from / SR;
  let pos = 0, prev = 0;
  return (x) => {
    const out = new Float32Array(Math.floor((x.length - pos) / ratio) + 1);
    let n = 0;
    for (; pos < x.length; pos += ratio) {
      const i = Math.floor(pos), f = pos - i;
      const a = i > 0 ? x[i - 1] : prev, b = x[i];
      out[n++] = a + (b - a) * f;
    }
    pos -= x.length;
    prev = x[x.length - 1];
    return out.subarray(0, n);
  };
}

function ensureWorker(ui) {
  if (worker) return;
  worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  state = "loading";
  ui.status("Preparing Phonon 2.5 English…");
  worker.onmessage = ({ data: m }) => {
    if (m.type === "progress") {
      const pct = Math.round(m.p * 100);
      if (m.stage === "download") ui.status(`Downloading Phonon 2.5 English · ${pct}% of 164 MB (once, then cached)`, m.p);
      else if (m.stage === "unpack") ui.status(`Unpacking weights · ${pct}%`, m.p);
      else ui.status("Starting the model on your GPU…", 1);
    } else if (m.type === "ready") {
      state = "ready";
      ui.status("Listening · speak and watch it write", null, "ready");
    } else if (m.type === "text") {
      ui.text(m.committed, m.live, m.ms);
    } else if (m.type === "error") {
      if (m.fatal) {
        state = "failed";
        worker.terminate();
        worker = null;
        ui.status(`Live transcription unavailable: ${m.message}. It needs WebGPU with fp16 (desktop Chrome or Edge). The aperture still follows your voice.`, null, "error");
      } else console.warn("phonon:", m.message);
    }
  };
  worker.onerror = (e) => {
    state = "failed";
    ui.status(`Live transcription failed to start (${e.message || "worker error"}).`, null, "error");
  };
  worker.postMessage({ type: "load" });
}

/** Start transcribing `src` (a MediaStreamAudioSourceNode). Returns a stop function. */
export async function startLive(ctx, src, ui) {
  if (navigator.deviceMemory && navigator.deviceMemory < 4) {
    ui.status("Live transcription needs a desktop browser with more memory. The aperture still follows your voice.", null, "error");
    return () => {};
  }
  ensureWorker(ui);
  if (state === "ready") ui.status("Listening · speak and watch it write", null, "ready");
  worker?.postMessage({ type: "reset" });
  ui.text("", "", null);
  const url = URL.createObjectURL(new Blob([TAP], { type: "text/javascript" }));
  await ctx.audioWorklet.addModule(url);
  URL.revokeObjectURL(url);
  const tap = new AudioWorkletNode(ctx, "fluss-tap");
  const toSR = resampler(ctx.sampleRate);
  tap.port.onmessage = ({ data }) => {
    if (!worker) return;
    const pcm = toSR(data).slice();
    worker.postMessage({ type: "audio", pcm }, [pcm.buffer]);
  };
  src.connect(tap);
  return () => {
    try {
      src.disconnect(tap);
    } catch {}
    tap.port.onmessage = null;
  };
}

/** Keep only the last `SHOWN` characters of finished text, cut at a word. */
export function clip(committed) {
  if (committed.length <= SHOWN) return committed;
  const cut = committed.slice(-SHOWN);
  const sp = cut.indexOf(" ");
  return "… " + (sp > 0 ? cut.slice(sp + 1) : cut);
}
