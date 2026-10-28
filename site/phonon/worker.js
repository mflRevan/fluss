// Phonon 2.5 English in the browser: download, unpack, run on WebGPU, stream.
//
// Messages in:  {type:"load"} · {type:"audio", pcm: Float32Array (16 kHz)} · {type:"reset"}
// Messages out: {type:"progress", stage, p} · {type:"ready", device} · {type:"error", message}
//               {type:"text", committed, live, ms}
import * as ort from "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.30.0/dist/ort.webgpu.min.mjs";
import { Decompress } from "https://cdn.jsdelivr.net/npm/fzstd@0.1.1/esm/index.mjs";
import { untar, unpack } from "./weights.js";

const MODEL_URL = "https://huggingface.co/FermionResearch/Phonon-2.5-English/resolve/main/phonon-2.5-en.bps.tar.zst";
const MODEL_BYTES = 163693770;
const CACHE = "fluss-phonon-2.5-en-v1";
const SR = 16000;
const FRAME = 1280; // samples per encoder frame (10 ms hop x 8 subsampling)
const MAX_WINDOW = 12 * SR; // re-decoded audio never grows past this
const KEEP_TAIL = 3 * SR; // when the window is full, keep the last seconds open
const STEP = 0.35 * SR; // decode again after this much new audio

let enc, dec, joint, pre, vocab, BLANK;
const post = (m) => self.postMessage(m);

async function fetchModel() {
  let res;
  try {
    const cache = await caches.open(CACHE);
    res = await cache.match(MODEL_URL);
    if (!res) {
      const r = await fetch(MODEL_URL);
      if (!r.ok) throw new Error(`download failed (${r.status})`);
      await cache.put(MODEL_URL, r.clone()).catch(() => {});
      res = r;
    }
  } catch {
    res = await fetch(MODEL_URL);
    if (!res.ok) throw new Error(`download failed (${res.status})`);
  }
  // Stream-decompress while downloading.
  const chunks = [];
  let outBytes = 0, inBytes = 0;
  const z = new Decompress((c) => {
    chunks.push(c);
    outBytes += c.length;
  });
  const reader = res.body.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    inBytes += value.length;
    z.push(value);
    post({ type: "progress", stage: "download", p: Math.min(1, inBytes / MODEL_BYTES) });
  }
  z.push(new Uint8Array(0), true);
  const tar = new Uint8Array(outBytes);
  let o = 0;
  for (const c of chunks) {
    tar.set(c, o);
    o += c.length;
  }
  return untar(tar, "model.fermion");
}

async function load() {
  if (!navigator.gpu) throw new Error("WebGPU is not available in this browser");
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("no WebGPU adapter");
  if (!adapter.features.has("shader-f16")) throw new Error("this GPU has no fp16 support in WebGPU");

  const base = new URL("./", import.meta.url);
  const get = (f) => fetch(new URL(f, base)).then((r) => (r.ok ? r : Promise.reject(new Error(`${f}: ${r.status}`))));
  const [manifest, vocabJson, encG, decG, jointG, preG] = await Promise.all([
    get("manifest.json").then((r) => r.json()),
    get("vocab.json").then((r) => r.json()),
    ...["encoder.onnx", "decoder.onnx", "joint.onnx", "nemo128.onnx"].map((f) => get(f).then((r) => r.arrayBuffer()).then((b) => new Uint8Array(b))),
  ]);
  vocab = vocabJson;
  BLANK = vocab.length;

  const container = await fetchModel();
  post({ type: "progress", stage: "unpack", p: 0 });
  const bufs = {};
  for (const [k, g] of Object.entries(manifest.graphs)) bufs[k] = new Uint8Array(g.bytes);
  unpack(container, manifest, bufs, (p) => post({ type: "progress", stage: "unpack", p }));

  post({ type: "progress", stage: "gpu", p: 0 });
  ort.env.wasm.numThreads = 1;
  const cpu = { executionProviders: ["wasm"] };
  pre = await ort.InferenceSession.create(preG, cpu);
  dec = await ort.InferenceSession.create(decG, { ...cpu, externalData: [{ path: "decoder.weights", data: bufs.decoder }] });
  joint = await ort.InferenceSession.create(jointG, { ...cpu, externalData: [{ path: "joint.weights", data: bufs.joint }] });
  enc = await ort.InferenceSession.create(encG, {
    // A few shape computations (e.g. the attention scale) have no WebGPU kernel; they run on the CPU.
    executionProviders: ["webgpu", "wasm"],
    externalData: [{ path: "encoder.weights", data: bufs.encoder }],
  });
  for (const k of Object.keys(bufs)) bufs[k] = null;
  // Warm up the shaders so the first real decode is fast.
  await transcribe(new Float32Array(SR));
  post({ type: "ready", device: "WebGPU" });
}

// ---------------------------------------------------------------- inference

async function transcribe(pcm) {
  const [feats, flen] = Object.values(
    await pre.run({ waveforms: new ort.Tensor("float32", pcm, [1, pcm.length]), waveforms_lens: new ort.Tensor("int64", BigInt64Array.from([BigInt(pcm.length)]), [1]) }),
  );
  const T = Number(flen.data[0]), M = 128, F = feats.dims[2];
  const x = new Float32Array(T * M); // [1,128,F] -> [1,T,128]
  for (let m = 0; m < M; m++) for (let t = 0; t < T; t++) x[t * M + m] = feats.data[m * F + t];
  const { enc: e } = await enc.run({ features: new ort.Tensor("float32", x, [1, T, M]) });
  return greedy(e.data, e.dims[1]);
}

/** TDT greedy decoding; returns tokens with the encoder frame they were emitted at. */
async function greedy(e, frames) {
  const H = 640;
  let h = new ort.Tensor("float32", new Float32Array(2 * H), [2, 1, H]);
  let c = new ort.Tensor("float32", new Float32Array(2 * H), [2, 1, H]);
  const step = (tok, h0, c0) => dec.run({ token: new ort.Tensor("int64", BigInt64Array.from([BigInt(tok)]), [1, 1]), h: h0, c: c0 });
  let out = await step(BLANK, h, c);
  const tokens = [];
  let f = 0, sym = 0;
  while (f < frames) {
    const { logits } = await joint.run({ enc: new ort.Tensor("float32", e.subarray(f * H, (f + 1) * H), [1, H]), dec: out.dec });
    const l = logits.data;
    let k = 0;
    for (let i = 1; i <= BLANK; i++) if (l[i] > l[k]) k = i;
    let d = BLANK + 1;
    for (let i = BLANK + 2; i < l.length; i++) if (l[i] > l[d]) d = i;
    const dur = d - BLANK - 1;
    if (k !== BLANK) {
      tokens.push({ id: k, frame: f });
      h = out.h_out;
      c = out.c_out;
      out = await step(k, h, c);
      sym++;
    }
    if (dur > 0) {
      f += dur;
      sym = 0;
    } else if (k === BLANK || sym >= 10) {
      f++;
      sym = 0;
    }
  }
  return tokens;
}

const text = (tokens) =>
  tokens
    .map((t) => vocab[t.id])
    .filter((p) => !/^<.*>$/.test(p))
    .join("")
    .replace(/▁/g, " ")
    .trim();

// ---------------------------------------------------------------- streaming

let audio = new Float32Array(0); // the open window
let committed = "";
let sinceDecode = 0;
let busy = false;
let quietFor = 0;

function append(pcm) {
  const next = new Float32Array(audio.length + pcm.length);
  next.set(audio);
  next.set(pcm, audio.length);
  audio = next;
  sinceDecode += pcm.length;
  let s = 0;
  for (const v of pcm) s += v * v;
  const rms = Math.sqrt(s / Math.max(1, pcm.length));
  quietFor = rms < 0.012 ? quietFor + pcm.length : 0;
}

function commit(words) {
  if (!words) return;
  committed = ((committed ? committed + " " : "") + words).slice(-4000);
}

async function pump() {
  if (busy || !enc || sinceDecode < STEP) return;
  busy = true;
  sinceDecode = 0;
  try {
    const t0 = performance.now();
    const tokens = await transcribe(audio);
    const ms = Math.round(performance.now() - t0);
    let live = text(tokens);
    if (quietFor > 0.7 * SR) {
      // A pause: everything heard so far is final.
      commit(live);
      live = "";
      audio = audio.slice(Math.max(0, audio.length - 0.3 * SR));
    } else if (audio.length > MAX_WINDOW) {
      // Window full: finalize the words that end well before "now", cut at a word start.
      const cutFrame = Math.floor((audio.length - KEEP_TAIL) / FRAME);
      let i = tokens.findIndex((t) => t.frame >= cutFrame);
      if (i < 0) i = tokens.length;
      while (i > 0 && i < tokens.length && !vocab[tokens[i].id].startsWith("▁")) i--;
      let cutAt = i < tokens.length ? tokens[i].frame * FRAME : audio.length - KEEP_TAIL;
      if (cutAt < SR) {
        // No word boundary to cut at (one very long word): cut at the frame instead.
        i = tokens.findIndex((t) => t.frame >= cutFrame);
        if (i < 0) i = tokens.length;
        cutAt = audio.length - KEEP_TAIL;
      }
      commit(text(tokens.slice(0, i)));
      live = text(tokens.slice(i));
      audio = audio.slice(cutAt);
    } else if (!live && audio.length > 2 * SR) {
      audio = audio.slice(audio.length - SR); // silence: don't let the window grow
    }
    post({ type: "text", committed, live, ms });
  } catch (e) {
    post({ type: "error", message: String(e?.message || e) });
  } finally {
    busy = false;
  }
}

self.onmessage = async ({ data }) => {
  if (data.type === "load") {
    try {
      await load();
    } catch (e) {
      post({ type: "error", message: String(e?.message || e), fatal: true });
    }
  } else if (data.type === "audio") {
    append(data.pcm);
    pump();
  } else if (data.type === "reset") {
    audio = new Float32Array(0);
    committed = "";
    sinceDecode = 0;
    quietFor = 0;
  }
};
