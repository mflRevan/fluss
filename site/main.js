(() => {
  "use strict";
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const GOLD = [227, 176, 75], DIM = [140, 107, 42], RULE = "#2a2418", RULE2 = "#4a3f27";

  // ------------------------------------------------------------ voice source
  // A speech-like envelope: phrases of syllables (~4.5 Hz) separated by pauses.
  const sim = { t: 0, phraseEnd: 0, pauseEnd: 0, syl: 0, sylAmp: 0, sylLen: 0.2 };
  function simLevel(dt) {
    sim.t += dt;
    if (sim.t > sim.phraseEnd) {
      if (sim.t > sim.pauseEnd) {
        sim.phraseEnd = sim.t + 1.6 + Math.random() * 2.2;
        sim.pauseEnd = sim.phraseEnd + 0.5 + Math.random() * 0.9;
      } else return 0.015 + Math.random() * 0.01;
    }
    sim.syl -= dt;
    if (sim.syl <= 0) {
      sim.sylLen = 0.14 + Math.random() * 0.16;
      sim.syl = sim.sylLen;
      sim.sylAmp = 0.35 + Math.random() * 0.65;
    }
    const p = 1 - sim.syl / sim.sylLen;
    return sim.sylAmp * Math.sin(Math.PI * p) ** 1.5 * (0.85 + Math.random() * 0.15);
  }

  let mic = null; // { analyser, data, stream, ctx }
  function micLevel() {
    mic.analyser.getFloatTimeDomainData(mic.data);
    let s = 0;
    for (const v of mic.data) s += v * v;
    const rms = Math.sqrt(s / mic.data.length);
    return Math.min(1, Math.sqrt(rms * 6));
  }

  // ------------------------------------------------------------ aperture
  // The voice visual: a six-blade camera iris. It opens with the voice and twists as
  // it opens, closes to a pinhole (showing the gold diamond) in silence and spins while
  // transcribing. Same geometry as the app and the overlay (docs/DESIGN.md).
  function makeIris(canvas, opts) {
    const o = Object.assign({ bezel: true }, opts);
    const TAU = Math.PI * 2, B = 6, HALF = Math.PI / B;
    const ctx = canvas.getContext("2d");
    let size = 0, dpr = 1;
    function resize() {
      dpr = Math.min(2, window.devicePixelRatio || 1);
      const r = canvas.getBoundingClientRect();
      size = Math.max(16, Math.round(r.width));
      canvas.width = size * dpr;
      canvas.height = size * dpr;
    }
    resize();
    new ResizeObserver(resize).observe(canvas);
    let smooth = 0, open = 0.13, rot = 0, lastT = 0;
    const gold = (a) => `rgba(${GOLD.join(",")},${a})`;
    return {
      push(level) {
        smooth += (level - smooth) * (level > smooth ? 0.55 : 0.18);
      },
      level: () => smooth,
      draw(time, mode) {
        const dt = Math.min(0.05, Math.max(0, time - lastT));
        lastT = time;
        const think = mode === "think";
        const target = think ? 0.2 + 0.05 * Math.sin(time * 10) : 0.13 + 0.72 * Math.min(1, smooth * 1.15);
        open = reduce ? target : open + (target - open) * Math.min(1, dt * 18);
        if (!reduce) rot += dt * (think ? 4.2 : 0.2);

        const s = size, c = s / 2;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, s, s);
        ctx.lineCap = "butt";
        if (o.bezel) {
          // Watch-style bezel: 120 graduations, slowly turning, index at 12 o'clock.
          const brot = reduce ? 0 : (time / 40) * TAU;
          const BR = s * 0.47;
          ctx.strokeStyle = RULE;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.arc(c, c, BR, 0, TAU);
          ctx.stroke();
          for (let i = 0; i < 120; i++) {
            const a = brot + (i / 120) * TAU;
            const l = i % 10 === 0 ? s * 0.03 : s * 0.012;
            ctx.strokeStyle = i % 10 === 0 ? RULE2 : RULE;
            ctx.beginPath();
            ctx.moveTo(c + Math.cos(a) * (BR - l), c + Math.sin(a) * (BR - l));
            ctx.lineTo(c + Math.cos(a) * BR, c + Math.sin(a) * BR);
            ctx.stroke();
          }
          ctx.fillStyle = gold(1);
          ctx.beginPath();
          ctx.moveTo(c, s * 0.035);
          ctx.lineTo(c - s * 0.012, s * 0.012);
          ctx.lineTo(c + s * 0.012, s * 0.012);
          ctx.fill();
        }
        const R = s * (o.bezel ? 0.34 : 0.42);
        const lw = Math.max(1, s * (o.bezel ? 0.003 : 0.03));
        const twist = rot + open * 1.3, vr = (R * open) / Math.cos(HALF);
        const hex = [];
        for (let i = 0; i < B; i++) {
          const a = twist + (i * TAU) / B + HALF;
          hex.push([c + Math.cos(a) * vr, c + Math.sin(a) * vr]);
        }
        const hexPath = () => {
          ctx.beginPath();
          hex.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
          ctx.closePath();
        };
        // Blade disc with the opening cut out.
        ctx.fillStyle = gold(0.09);
        ctx.beginPath();
        ctx.arc(c, c, R, 0, TAU);
        ctx.fill();
        ctx.globalCompositeOperation = "destination-out";
        hexPath();
        ctx.fill();
        ctx.globalCompositeOperation = "source-over";
        // Blade edges: rays leaving each hexagon vertex tangentially, ending on the rim.
        ctx.lineWidth = lw;
        ctx.strokeStyle = `rgb(${DIM.join(",")})`;
        for (let i = 0; i < B; i++) {
          const th = twist + (i * TAU) / B;
          const [vx, vy] = hex[(i + B - 1) % B];
          const dx = -Math.sin(th), dy = Math.cos(th), ox = vx - c, oy = vy - c;
          const b = ox * dx + oy * dy;
          const u = -b + Math.sqrt(Math.max(0, b * b - (ox * ox + oy * oy - R * R)));
          ctx.beginPath();
          ctx.moveTo(vx, vy);
          ctx.lineTo(vx + dx * u, vy + dy * u);
          ctx.stroke();
        }
        ctx.strokeStyle = gold(1);
        hexPath();
        ctx.stroke();
        ctx.strokeStyle = `rgb(${DIM.join(",")})`;
        ctx.beginPath();
        ctx.arc(c, c, R, 0, TAU);
        ctx.stroke();
        // Lens markings.
        ctx.strokeStyle = gold(0.35);
        ctx.lineWidth = Math.max(1, s * 0.003);
        for (let i = 0; i < 24; i++) {
          const a = (i * TAU) / 24, l = i % 2 ? 0.05 : 0.1;
          ctx.beginPath();
          ctx.moveTo(c + Math.cos(a) * R * 1.04, c + Math.sin(a) * R * 1.04);
          ctx.lineTo(c + Math.cos(a) * R * (1.04 + l), c + Math.sin(a) * R * (1.04 + l));
          ctx.stroke();
        }
        // The diamond shows through the pinhole when nearly closed.
        const show = Math.max(0, 1 - open / 0.3);
        if (show > 0) {
          const d = Math.max(1.2, R * open * 0.45);
          ctx.fillStyle = gold(0.95 * show);
          ctx.beginPath();
          ctx.moveTo(c, c - d);
          ctx.lineTo(c + d, c);
          ctx.lineTo(c, c + d);
          ctx.lineTo(c - d, c);
          ctx.closePath();
          ctx.fill();
        }
      },
    };
  }

  // ------------------------------------------------------------ hero
  const heroCanvas = document.getElementById("iris");
  const hero = makeIris(heroCanvas, { bezel: true });
  const roLevel = document.getElementById("ro-level");
  const roSrc = document.getElementById("ro-src");
  let heroVisible = true;
  new IntersectionObserver((e) => (heroVisible = e[0].isIntersecting)).observe(heroCanvas);

  const micBtn = document.getElementById("mic-btn");
  const live = document.getElementById("live");
  const liveStatus = document.getElementById("live-status");
  const liveBar = document.getElementById("live-bar");
  const liveDone = document.getElementById("live-done");
  const liveNow = document.getElementById("live-now");
  const roDecode = document.getElementById("ro-decode");
  const liveUi = {
    status(msg, p = null, kind = "") {
      liveStatus.textContent = msg;
      live.dataset.state = kind;
      liveBar.style.transform = `scaleX(${p == null ? 0 : p})`;
    },
    text(done, now, ms) {
      liveDone.textContent = done ? clipText(done) : "";
      liveNow.textContent = now ? (done ? " " : "") + now : "";
      if (ms != null) roDecode.textContent = `${ms} ms`;
    },
  };
  let clipText = (t) => t;
  micBtn.setAttribute("aria-pressed", "false");
  micBtn.addEventListener("click", async () => {
    if (mic) {
      mic.stopLive?.();
      mic.stream.getTracks().forEach((t) => t.stop());
      mic.ctx.close();
      mic = null;
      micBtn.textContent = "Try with your mic";
      micBtn.setAttribute("aria-pressed", "false");
      roSrc.textContent = "simulated";
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
      let ac;
      try {
        ac = new AudioContext({ sampleRate: 16000 }); // the model's rate; the browser resamples
      } catch {
        ac = new AudioContext();
      }
      const src = ac.createMediaStreamSource(stream);
      const analyser = ac.createAnalyser();
      analyser.fftSize = 1024;
      src.connect(analyser);
      mic = { stream, ctx: ac, analyser, data: new Float32Array(analyser.fftSize) };
      micBtn.textContent = "Stop mic";
      micBtn.setAttribute("aria-pressed", "true");
      roSrc.textContent = "microphone";
      live.hidden = false;
      const m = mic;
      import("./phonon/live.js")
        .then(async (mod) => {
          clipText = mod.clip;
          const stop = await mod.startLive(ac, src, liveUi);
          if (mic === m) m.stopLive = stop;
          else stop();
        })
        .catch((e) => liveUi.status(`Live transcription unavailable (${e.message}).`, null, "error"));
    } catch {
      micBtn.textContent = "Mic unavailable";
    }
  });

  // ------------------------------------------------------------ flow demo
  const pill = document.getElementById("pill");
  const pillText = document.getElementById("pill-text");
  const pillStatus = document.getElementById("pill-status");
  const typed = document.getElementById("typed");
  const steps = [...document.querySelectorAll("#steps li")];
  const pillIris = makeIris(document.getElementById("pill-iris"), { bezel: false });
  const SENTENCE = "Can we move the review to Thursday afternoon? The build looks good.";
  const WORDS = SENTENCE.split(" ");
  const flow = { t: 0, phase: -1, visible: false };
  const stage = document.querySelector(".stage");
  new IntersectionObserver((e) => (flow.visible = e[0].isIntersecting), { threshold: 0.3 }).observe(stage);
  // Timeline of the loop (seconds).
  const T = { hold: 1.0, speak: 1.6, release: 4.6, typed: 4.85, clear: 8.2 };
  function setStep(n) {
    steps.forEach((li, i) => li.classList.toggle("on", i === n));
  }
  function flowTick(dt) {
    if (!flow.visible && flow.phase === -1) return;
    flow.t += dt;
    const t = flow.t;
    let phase, level = 0;
    if (t < T.hold) phase = 0;
    else if (t < T.speak) phase = 1;
    else if (t < T.release) phase = 2;
    else if (t < T.typed) phase = 3;
    else if (t < T.clear) phase = 4;
    else {
      flow.t = 0;
      phase = 0;
    }
    if (phase !== flow.phase) {
      flow.phase = phase;
      if (phase === 0) {
        typed.textContent = "";
        pill.classList.remove("show");
        setStep(-1);
      } else if (phase === 1) {
        setStep(0);
        pillStatus.textContent = "REC";
        pillText.textContent = "";
        pill.classList.add("show");
      } else if (phase === 2) {
        setStep(1);
      } else if (phase === 3) {
        setStep(2);
        pillStatus.textContent = "…";
      } else if (phase === 4) {
        setStep(2);
        pill.classList.remove("show");
        typed.textContent = SENTENCE;
      }
    }
    if (phase === 2) {
      const p = (t - T.speak) / (T.release - T.speak);
      const n = Math.min(WORDS.length, Math.floor(p * (WORDS.length + 1.5)));
      // Shown right-aligned so the newest words stay visible (rtl container, ltr text).
      pillText.textContent = "‪" + WORDS.slice(0, n).join(" ") + "‬";
      level = simLevel(dt);
    }
    pillIris.push(level);
    pillIris.draw(performance.now() / 1000, phase === 3 ? "think" : "live");
  }

  // ------------------------------------------------------------ seam demo
  const seamPunct = document.getElementById("seam-punct");
  const seamOut = document.getElementById("seam-out");
  const seamFig = document.querySelector(".seam");
  let seamOn = false, seamT = 0;
  new IntersectionObserver((e) => (seamOn = e[0].isIntersecting), { threshold: 0.4 }).observe(seamFig);
  function seamTick(dt) {
    if (!seamOn) return;
    seamT = (seamT + dt) % 4.4;
    const repaired = seamT > 1.6;
    seamPunct.classList.toggle("gone", repaired);
    seamOut.style.visibility = repaired ? "visible" : "hidden";
  }
  if (reduce) seamOut.style.visibility = "visible";

  // ------------------------------------------------------------ timeline (static SVG)
  const NS = "http://www.w3.org/2000/svg";
  const ticks = document.getElementById("tl-ticks");
  for (let x = 40, i = 0; x <= 960; x += 55, i++) {
    const l = document.createElementNS(NS, "line");
    l.setAttribute("x1", x); l.setAttribute("x2", x);
    l.setAttribute("y1", 140); l.setAttribute("y2", i % 4 === 0 ? 148 : 144);
    ticks.appendChild(l);
  }
  const wave = document.getElementById("tl-wave");
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let x = 46; x < 958; x += 8) {
    const speech = (x > 196 && x < 470) || (x > 520 && x < 808);
    const syl = Math.abs(Math.sin(x / 15)) * 0.65 + rnd() * 0.35;
    const h = speech ? 4 + syl * 34 : 1 + rnd() * 2;
    const l = document.createElementNS(NS, "line");
    l.setAttribute("x1", x); l.setAttribute("x2", x);
    l.setAttribute("y1", 88 - h); l.setAttribute("y2", 88 + h);
    if (x >= 150 && x <= 870) l.setAttribute("class", "in");
    wave.appendChild(l);
  }

  // ------------------------------------------------------------ reveal on scroll
  if (!reduce && "IntersectionObserver" in window) {
    const els = document.querySelectorAll(".sect-head, .split, .flow, .timeline, .table-wrap, .figures, .credits, .caption");
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); }
    }, { rootMargin: "0px 0px -8% 0px" });
    els.forEach((el) => { el.classList.add("reveal"); io.observe(el); });
  }

  // ------------------------------------------------------------ loop
  let last = performance.now();
  function frame(now) {
    const real = Math.min(0.5, (now - last) / 1000); // demo timelines follow the clock
    const dt = Math.min(0.05, real); // signal smoothing stays stable
    last = now;
    const time = now / 1000;
    if (heroVisible) {
      const lv = mic ? micLevel() : reduce ? 0.25 : simLevel(dt);
      hero.push(lv);
      hero.draw(time, "live");
      const db = 20 * Math.log10(Math.max(1e-4, hero.level() * 0.5));
      roLevel.textContent = hero.level() < 0.02 ? "−∞ dB" : `${db.toFixed(1)} dB`;
    }
    flowTick(real);
    seamTick(real);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
})();
