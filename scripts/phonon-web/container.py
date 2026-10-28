"""Reads a `fermion-five-value-parakeet-v1` container (Phonon) into fp32 numpy arrays
keyed by Hugging Face ParakeetForTDT names. Mirrors site/phonon/weights.js (which must produce the same bytes)."""
import json
import numpy as np


def five_value(blob, o, i):
    rb = (i + 4) // 5
    x = np.frombuffer(blob, dtype=np.uint8, count=o * rb).reshape(o, rb).astype(np.uint16)
    d = np.empty((o, rb, 5), dtype=np.uint8)
    for k in range(5):
        d[:, :, k] = (x // (3 ** k)) % 3
    codes = d.reshape(o, rb * 5)[:, :i]
    nz = codes != 1
    nnz = int(nz.sum())
    off = o * rb
    rbytes = (nnz + 7) // 8
    bits = np.unpackbits(np.frombuffer(blob[off:off + rbytes], dtype=np.uint8), bitorder="little")[:nnz].astype(bool)
    off += rbytes
    lo = np.frombuffer(blob[off:off + 2 * o], dtype=np.float16).astype(np.float32)
    hi = np.frombuffer(blob[off + 2 * o:off + 4 * o], dtype=np.float16).astype(np.float32)
    assert off + 4 * o == len(blob)
    is_hi = np.zeros((o, i), dtype=bool)
    is_hi[nz] = bits
    mag = np.where(is_hi, hi[:, None], lo[:, None])
    return (codes.astype(np.float32) - 1) * mag


def intn(blob, shape, bits):
    o = shape[0]
    total = int(np.prod(shape))
    scales = np.frombuffer(blob[-2 * o:], dtype=np.float16).astype(np.float32)
    body = blob[:-2 * o]
    if bits == 8:
        q = np.frombuffer(body, dtype=np.int8).astype(np.float32)[:total]
    else:
        b = np.frombuffer(body, dtype=np.uint8).reshape(-1, 3).astype(np.uint32)
        packed = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        u = np.stack([(packed >> s) & 0x3F for s in (0, 6, 12, 18)], axis=1).ravel()
        q = u[:total].astype(np.float32) - 32
    return (q.reshape(o, -1) * scales[:, None]).reshape(shape)


def read(path):
    out, index = {}, []
    with open(path, "rb") as fh:
        n = int.from_bytes(fh.read(8), "little")
        header = json.loads(fh.read(n))
        for e in header["index"]:
            blob = fh.read(e["b"])
            k, shape = e["k"], tuple(e["shape"])
            if k == "five_value":
                name, arr = e["n"] + ".weight", five_value(blob, *shape)
            elif k.startswith("int"):
                name, arr = e["n"], intn(blob, shape, int(k[3:]))
            elif k == "fp16":
                name, arr = e["n"], np.frombuffer(blob, dtype=np.float16).astype(np.float32).reshape(shape)
            else:
                raise ValueError(k)
            out[name] = arr
            index.append(e)
    return out, header
