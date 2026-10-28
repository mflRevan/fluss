"""Builds the website's in-browser Phonon (site/phonon/*.onnx, manifest.json, vocab.json).

Phonon 2.5 English is a quantized Parakeet TDT 0.6B v3 whose container uses Hugging Face
ParakeetForTDT tensor names. We export the architecture to ONNX (encoder in fp16 for
WebGPU, decoder and joint in fp32 for WASM) and strip every weight out: the graphs point
at `<graph>.weights` files that the page rebuilds from Phonon's own Hugging Face download
(site/phonon/weights.js), following manifest.json. Only ~1 MB of graphs is hosted.

    uv venv --python 3.12 .venv
    uv pip install torch --index-url https://download.pytorch.org/whl/cpu
    uv pip install transformers==5.21.0 onnx onnxruntime soundfile librosa
    python build.py --container <model_phonon25_en_.../model.fermion> --config <same dir>/config.json \
        --base <dir with nvidia/parakeet-tdt-0.6b-v3 config.json, generation_config.json, processor files> \
        --nemo128 <nemo128.onnx from istupakov/parakeet-tdt-0.6b-v3-onnx> [--wav test.wav]
"""
import argparse
import json
import os
import re
import tempfile
from pathlib import Path

import numpy as np
import onnx
import torch
from onnx import TensorProto, helper, numpy_helper
from onnx.reference import ReferenceEvaluator
from torch import nn
from transformers import GenerationConfig, ParakeetForTDT, ParakeetTDTConfig

import container

OUT = Path(__file__).resolve().parents[2] / "site" / "phonon"


def load(path, base):
    tensors, header = container.read(path)
    model = ParakeetForTDT(ParakeetTDTConfig.from_pretrained(base))
    sd = {}
    for k, v in tensors.items():
        if k.endswith("num_batches_tracked"):
            sd[k] = torch.tensor(0, dtype=torch.int64)
            continue
        if v.ndim == 2 and re.search(r"\.conv\.pointwise_conv[12]\.weight$", k):
            v = v[:, :, None]  # stored squeezed
        sd[k] = torch.from_numpy(np.ascontiguousarray(v))
    missing, unexpected = model.load_state_dict(sd, strict=False)
    assert not missing and not unexpected, (missing[:5], unexpected[:5])
    model.eval()
    model.generation_config = GenerationConfig.from_pretrained(base)
    return model, tensors, header


class Encoder(nn.Module):
    """features [1,T,128] fp32 -> projected encoder output [1,F,640] fp32; fp16 inside."""

    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, features):
        h = self.m.encoder(input_features=features.half()).last_hidden_state
        return self.m.encoder_projector(h).float()


class Decoder(nn.Module):
    """One prediction-network step: token [1,1], LSTM state -> decoder output [1,640], new state."""

    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, token, h, c):
        o, (h2, c2) = self.m.decoder.lstm(self.m.decoder.embedding(token), (h, c))
        return self.m.decoder.decoder_projector(o)[:, 0], h2, c2


class Joint(nn.Module):
    """enc frame [1,640] + decoder output [1,640] -> token + duration logits [1, 8193+5]."""

    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, enc, dec):
        return self.m.joint.head(self.m.joint.activation(enc + dec))


def export(model, tmp):
    with torch.no_grad():
        torch.onnx.export(Decoder(model), (torch.zeros(1, 1, dtype=torch.int64), torch.zeros(2, 1, 640), torch.zeros(2, 1, 640)), f"{tmp}/decoder.onnx",
                          input_names=["token", "h", "c"], output_names=["dec", "h_out", "c_out"], opset_version=17, dynamo=False, do_constant_folding=False)
        torch.onnx.export(Joint(model), (torch.zeros(1, 640), torch.zeros(1, 640)), f"{tmp}/joint.onnx",
                          input_names=["enc", "dec"], output_names=["logits"], opset_version=17, dynamo=False, do_constant_folding=False)
        model.half()  # last: the decoder and joint above stay fp32
        torch.onnx.export(Encoder(model), (torch.randn(1, 400, 128),), f"{tmp}/encoder.onnx", input_names=["features"], output_names=["enc"],
                          dynamic_axes={"features": {1: "T"}, "enc": {1: "F"}}, opset_version=17, dynamo=False, do_constant_folding=False, external_data=True)


def fold_constants(m):
    """Fold nodes whose inputs are all constants (the web runtime has no float64 kernels)."""
    g = m.graph
    const = {n.output[0]: numpy_helper.to_array(n.attribute[0].t) for n in g.node if n.op_type == "Constant" and n.attribute[0].name == "value"}
    changed = True
    while changed:
        changed = False
        for n in list(g.node):
            if n.op_type == "Constant" or not n.input or not all(i in const for i in n.input if i):
                continue
            ins = [helper.make_tensor_value_info(i, helper.np_dtype_to_tensor_dtype(const[i].dtype), const[i].shape) for i in n.input if i]
            outs = [helper.make_tensor_value_info(o, TensorProto.UNDEFINED, None) for o in n.output]
            vals = ReferenceEvaluator(helper.make_model(helper.make_graph([n], "f", ins, outs), opset_imports=m.opset_import)).run(None, {i: const[i] for i in n.input if i})
            idx = list(g.node).index(n)
            g.node.remove(n)
            for k, (o, v) in enumerate(zip(n.output, vals)):
                const[o] = np.asarray(v)
                g.node.insert(idx + k, helper.make_node("Constant", [], [o], value=numpy_helper.from_array(const[o], o + "_c")))
            changed = True
    used = {i for n in g.node for i in n.input} | {o.name for o in g.output}
    for n in list(g.node):
        if n.op_type == "Constant" and n.output[0] not in used:
            g.node.remove(n)


def externalize(src, name, tensors, manifest, ref_dir):
    """Point every weight at `<name>.weights` (16-byte aligned) and record where it comes from."""
    m = onnx.load(src)
    entries, blob, off = [], [], 0
    for init in m.graph.initializer:
        key = init.name.removeprefix("m.")
        arr = numpy_helper.to_array(init)
        assert init.data_type in (TensorProto.FLOAT16, TensorProto.FLOAT), (key, init.data_type)
        dt = np.float16 if init.data_type == TensorProto.FLOAT16 else np.float32
        data = tensors[key].astype(dt)
        assert data.size == arr.size and np.array_equal(arr.ravel(), data.ravel()), key
        off = (off + 15) // 16 * 16
        entries.append({"n": key, "o": off, "dt": "f16" if dt == np.float16 else "f32", "len": int(arr.size)})
        blob.append((off, data.tobytes()))
        init.ClearField("raw_data")
        init.data_location = TensorProto.EXTERNAL
        del init.external_data[:]
        for k, v in (("location", f"{name}.weights"), ("offset", str(off)), ("length", str(data.nbytes))):
            e = init.external_data.add()
            e.key, e.value = k, v
        off += data.nbytes
    fold_constants(m)
    onnx.save(m, OUT / f"{name}.onnx")
    buf = bytearray(off)
    for o, d in blob:
        buf[o:o + len(d)] = d
    (Path(ref_dir) / f"{name}.weights").write_bytes(buf)  # for the local check below
    manifest["graphs"][name] = {"bytes": off, "tensors": entries}


def float_preprocessor(src):
    """nemo128.onnx computes its window in float64; switch it to float32 (max diff ~1e-4)."""
    m = onnx.load(src)
    for i in m.graph.initializer:
        if i.data_type == TensorProto.DOUBLE:
            i.CopyFrom(numpy_helper.from_array(numpy_helper.to_array(i).astype(np.float32), i.name))
    for n in m.graph.node:
        for a in n.attribute:
            if n.op_type == "Cast" and a.name == "to" and a.i == TensorProto.DOUBLE:
                a.i = TensorProto.FLOAT
            if a.type == onnx.AttributeProto.TENSOR and a.t.data_type == TensorProto.DOUBLE:
                a.t.CopyFrom(numpy_helper.from_array(numpy_helper.to_array(a.t).astype(np.float32), a.t.name))
    for v in list(m.graph.value_info) + list(m.graph.input) + list(m.graph.output):
        if v.type.tensor_type.elem_type == TensorProto.DOUBLE:
            v.type.tensor_type.elem_type = TensorProto.FLOAT
    onnx.checker.check_model(m)
    onnx.save(m, OUT / "nemo128.onnx")


def check(wav, ref_dir, vocab):
    """Transcribe with the exported graphs, the same greedy TDT loop as worker.js."""
    import onnxruntime as ort
    import soundfile as sf
    for f in ("encoder", "decoder", "joint"):
        (OUT / f"{f}.weights").write_bytes((Path(ref_dir) / f"{f}.weights").read_bytes())
    try:
        w, _ = sf.read(wav, dtype="float32")
        s = {f: ort.InferenceSession(str(OUT / f"{f}.onnx")) for f in ("nemo128", "encoder", "decoder", "joint")}
        feats, flen = s["nemo128"].run(None, {"waveforms": w[None], "waveforms_lens": np.array([len(w)], dtype=np.int64)})
        e = s["encoder"].run(None, {"features": np.ascontiguousarray(feats[:, :, :flen[0]].transpose(0, 2, 1))})[0][0]
        blank = len(vocab)
        h = c = np.zeros((2, 1, 640), np.float32)
        d, h1, c1 = s["decoder"].run(None, {"token": np.array([[blank]], np.int64), "h": h, "c": c})
        toks, f, sym = [], 0, 0
        while f < len(e):
            lg = s["joint"].run(None, {"enc": e[f:f + 1], "dec": d})[0][0]
            k, dur = int(lg[:blank + 1].argmax()), int(lg[blank + 1:].argmax())
            if k != blank:
                toks.append(k)
                d, h1, c1 = s["decoder"].run(None, {"token": np.array([[k]], np.int64), "h": h1, "c": c1})
                sym += 1
            if dur > 0:
                f, sym = f + dur, 0
            elif k == blank or sym >= 10:
                f, sym = f + 1, 0
        print("check:", "".join(vocab[i] for i in toks).replace("▁", " ").strip())
    finally:
        for f in ("encoder", "decoder", "joint"):
            (OUT / f"{f}.weights").unlink(missing_ok=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--container", required=True)
    ap.add_argument("--config", required=True, help="the container's NeMo config.json (vocabulary)")
    ap.add_argument("--base", required=True)
    ap.add_argument("--nemo128", required=True)
    ap.add_argument("--wav")
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    model, tensors, header = load(a.container, a.base)
    manifest = {"format": header["format"], "graphs": {}}
    with tempfile.TemporaryDirectory() as tmp:
        export(model, tmp)
        for name in ("encoder", "decoder", "joint"):
            externalize(f"{tmp}/{name}.onnx", name, tensors, manifest, tmp)
        (OUT / "manifest.json").write_text(json.dumps(manifest, separators=(",", ":")))
        float_preprocessor(a.nemo128)
        vocab = json.loads(Path(a.config).read_text(encoding="utf-8"))["labels"]
        (OUT / "vocab.json").write_text(json.dumps(vocab, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
        for f in sorted(OUT.glob("*")):
            print(f"{f.name:16} {os.path.getsize(f):>10}")
        if a.wav:
            check(a.wav, tmp, vocab)


if __name__ == "__main__":
    main()
