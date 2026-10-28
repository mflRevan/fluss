// Rebuilds the ONNX weight files from Phonon's own download.
//
// Phonon 2.5 English ships as a `fermion-five-value-parakeet-v1` container (inside
// phonon-2.5-en.bps.tar.zst on Hugging Face): an 8-byte header length, a JSON index,
// then one record per tensor, named like Hugging Face's ParakeetForTDT. Records are
//   five_value  rows of {0, ±lo, ±hi}: base-3 codes (5 per byte), one bit per non-zero
//               (lo or hi), then fp16 lo[rows] and hi[rows]
//   int6/int8   per-row symmetric integers with fp16 row scales
//   fp16        plain half floats
// manifest.json (made by scripts/phonon-web/build.py) says where each tensor goes in
// encoder.weights (fp16), decoder.weights and joint.weights (fp32).

const DIGITS = new Uint8Array(243 * 5);
for (let b = 0; b < 243; b++) for (let k = 0, x = b; k < 5; k++, x = (x / 3) | 0) DIGITS[b * 5 + k] = x % 3;

const f32 = new Float32Array(1);
const u32 = new Uint32Array(f32.buffer);
function toHalf(v) {
  f32[0] = v;
  const x = u32[0];
  const sign = (x >>> 16) & 0x8000;
  let e = ((x >>> 23) & 0xff) - 127 + 15;
  let m = x & 0x7fffff;
  if (e >= 31) return sign | 0x7c00;
  if (e <= 0) {
    if (e < -10) return sign;
    const full = m | 0x800000, shift = 14 - e;
    const h = full >>> shift, rest = full & ((1 << shift) - 1), halfway = 1 << (shift - 1);
    return sign | (h + (rest > halfway || (rest === halfway && h & 1) ? 1 : 0));
  }
  // Round to nearest, ties to even (what numpy does).
  const h = sign | (e << 10) | (m >>> 13), rest = m & 0x1fff;
  return h + (rest > 0x1000 || (rest === 0x1000 && h & 1) ? 1 : 0);
}
function fromHalf(h) {
  const s = h & 0x8000 ? -1 : 1, e = (h >>> 10) & 0x1f, m = h & 0x3ff;
  if (e === 0) return s * m * 2 ** -24;
  if (e === 31) return m ? NaN : s * Infinity;
  return s * (1 + m / 1024) * 2 ** (e - 15);
}

/** Find a file in an (uncompressed) tar archive. */
export function untar(tar, suffix) {
  let p = 0;
  const dec = new TextDecoder();
  while (p + 512 <= tar.length && tar[p] !== 0) {
    const name = dec.decode(tar.subarray(p, p + 100)).replace(/\0.*$/s, "");
    const size = parseInt(dec.decode(tar.subarray(p + 124, p + 136)).replace(/\0.*$/s, "").trim() || "0", 8);
    if (name.endsWith(suffix)) return tar.subarray(p + 512, p + 512 + size);
    p += 512 + Math.ceil(size / 512) * 512;
  }
  throw new Error(`${suffix} not found in the download`);
}

/**
 * Write every container tensor into the graph weight buffers.
 * `bufs` maps graph name -> Uint8Array of manifest.graphs[name].bytes.
 */
export function unpack(container, manifest, bufs, onProgress) {
  const where = new Map();
  for (const [graph, g] of Object.entries(manifest.graphs)) for (const t of g.tensors) where.set(t.n, { graph, ...t });
  const dv = new DataView(container.buffer, container.byteOffset, container.byteLength);
  const n = Number(dv.getBigUint64(0, true));
  const header = JSON.parse(new TextDecoder().decode(container.subarray(8, 8 + n)));
  if (header.format !== manifest.format) throw new Error(`unexpected model format ${header.format}`);
  let p = 8 + n, filled = 0;
  for (let r = 0; r < header.index.length; r++) {
    const e = header.index[r];
    const blob = container.subarray(p, p + e.b);
    p += e.b;
    const name = e.k === "five_value" ? e.n + ".weight" : e.n;
    const dst = where.get(name);
    if (!dst) continue; // e.g. BatchNorm's num_batches_tracked
    const buf = bufs[dst.graph];
    const half = dst.dt === "f16";
    const out = half ? new Uint16Array(buf.buffer, buf.byteOffset + dst.o, dst.len) : new Float32Array(buf.buffer, buf.byteOffset + dst.o, dst.len);
    if (e.k === "five_value") fiveValue(blob, e.shape, out, half);
    else if (e.k === "int6" || e.k === "int8") intN(blob, e.shape, e.k === "int6" ? 6 : 8, out, half);
    else if (e.k === "fp16") {
      const src = new Uint16Array(blob.slice().buffer);
      if (half) out.set(src);
      else for (let i = 0; i < src.length; i++) out[i] = fromHalf(src[i]);
    } else throw new Error(`unknown record kind ${e.k}`);
    filled++;
    if (onProgress && r % 16 === 0) onProgress(r / header.index.length);
  }
  const want = Object.values(manifest.graphs).reduce((a, g) => a + g.tensors.length, 0);
  if (filled !== want) throw new Error(`filled ${filled} of ${want} tensors`);
}

function fiveValue(blob, [o, i], out, half) {
  const rb = Math.ceil(i / 5);
  let nnz = 0;
  for (let k = 0; k < o * rb; k++) {
    const b = blob[k] * 5;
    for (let d = 0; d < 5; d++) if (DIGITS[b + d] !== 1) nnz++;
  }
  // Codes past `i` in a row's last byte are padding; recount only real columns.
  if (rb * 5 !== i) {
    nnz = 0;
    for (let row = 0; row < o; row++)
      for (let c = 0; c < i; c++) if (DIGITS[blob[row * rb + ((c / 5) | 0)] * 5 + (c % 5)] !== 1) nnz++;
  }
  const bitsAt = o * rb;
  const scalesAt = bitsAt + Math.ceil(nnz / 8);
  const sv = new DataView(blob.buffer, blob.byteOffset + scalesAt, 4 * o);
  let bit = 0;
  for (let row = 0; row < o; row++) {
    const lo = sv.getUint16(2 * row, true), hi = sv.getUint16(2 * o + 2 * row, true);
    const loF = half ? 0 : fromHalf(lo), hiF = half ? 0 : fromHalf(hi);
    const base = row * i;
    for (let c = 0; c < i; c++) {
      const code = DIGITS[blob[row * rb + ((c / 5) | 0)] * 5 + (c % 5)];
      if (code === 1) {
        out[base + c] = 0;
        continue;
      }
      const isHi = (blob[bitsAt + (bit >> 3)] >> (bit & 7)) & 1;
      bit++;
      if (half) out[base + c] = (isHi ? hi : lo) ^ (code === 0 ? 0x8000 : 0);
      else out[base + c] = (code === 0 ? -1 : 1) * (isHi ? hiF : loF);
    }
  }
}

function intN(blob, shape, bits, out, half) {
  const o = shape[0];
  const total = shape.reduce((a, b) => a * b, 1);
  const cols = total / o;
  const sv = new DataView(blob.buffer, blob.byteOffset + blob.length - 2 * o, 2 * o);
  for (let row = 0; row < o; row++) {
    const s = fromHalf(sv.getUint16(2 * row, true));
    for (let c = 0; c < cols; c++) {
      const idx = row * cols + c;
      let q;
      if (bits === 8) q = (blob[idx] << 24) >> 24;
      else {
        const g = (idx >> 2) * 3, sh = (idx & 3) * 6;
        const packed = blob[g] | (blob[g + 1] << 8) | (blob[g + 2] << 16);
        q = ((packed >>> sh) & 0x3f) - 32;
      }
      const v = q * s;
      out[idx] = half ? toHalf(v) : v;
    }
  }
}
