(() => {
  "use strict";
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const GOLD = [227, 176, 75], DIM = [140, 107, 42], RULE = "#2a2418", RULE2 = "#4a3f27";
  const mix = (a, b, t) => `rgb(${a.map((v, i) => Math.round(v + (b[i] - v) * t)).join(",")})`;

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

  // ------------------------------------------------------------ iris
  function makeIris(canvas, opts) {
    const o = Object.assign({ ticks: 72, bezel: true, inner: 0.2, maxLen: 0.2 }, opts);
    const ctx = canvas.getContext("2d");
    const hist = new Float32Array(o.ticks);
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
    let smooth = 0;
    return {
      push(level) {
        smooth += (level - smooth) * 0.5;
        hist.copyWithin(1, 0);
        hist[0] = smooth;
      },
      level: () => smooth,
      draw(time, mode) {
        const s = size, c = s / 2;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, s, s);
        ctx.lineCap = "butt";
        if (o.bezel) {
          // Watch-style bezel: 120 graduations, slowly turning.
          const rot = reduce ? 0 : (time / 40) * Math.PI * 2;
          const R = s * 0.47;
          ctx.strokeStyle = RULE;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.arc(c, c, R, 0, Math.PI * 2);
          ctx.stroke();
          for (let i = 0; i < 120; i++) {
            const a = rot + (i / 120) * Math.PI * 2;
            const major = i % 10 === 0;
            const l = major ? s * 0.03 : s * 0.012;
            ctx.strokeStyle = major ? RULE2 : RULE;
            ctx.beginPath();
            ctx.moveTo(c + Math.cos(a) * (R - l), c + Math.sin(a) * (R - l));
            ctx.lineTo(c + Math.cos(a) * R, c + Math.sin(a) * R);
            ctx.stroke();
          }
          // Fixed index marker at 12 o'clock.
          ctx.fillStyle = mix(DIM, GOLD, 1);
          ctx.beginPath();
          ctx.moveTo(c, s * 0.035);
          ctx.lineTo(c - s * 0.012, s * 0.012);
          ctx.lineTo(c + s * 0.012, s * 0.012);
          ctx.fill();
          ctx.strokeStyle = RULE;
          ctx.beginPath();
          ctx.arc(c, c, s * (o.inner - 0.035), 0, Math.PI * 2);
          ctx.stroke();
        }
        // Voice ticks: newest at 12 o'clock, history sweeping clockwise.
        const r0 = s * o.inner;
        const w = Math.max(1, s * (o.bezel ? 0.0045 : 0.05));
        ctx.lineWidth = w;
        for (let i = 0; i < o.ticks; i++) {
          const a = -Math.PI / 2 + (i / o.ticks) * Math.PI * 2;
          let v = hist[i];
          if (mode === "think") {
            const head = ((time * 1.4) % 1) * o.ticks;
            const d = (i - head + o.ticks) % o.ticks;
            v = d < o.ticks * 0.18 ? 0.55 * (1 - d / (o.ticks * 0.18)) : 0.04;
          }
          const fade = 1 - (i / o.ticks) * 0.55;
          const len = s * ((o.bezel ? 0.025 : 0.06) + o.maxLen * Math.min(1, v));
          const t = Math.min(1, v * 1.8) * fade;
          ctx.strokeStyle = mix([46, 39, 24], GOLD, Math.max(0.12, t));
          ctx.beginPath();
          ctx.moveTo(c + Math.cos(a) * r0, c + Math.sin(a) * r0);
          ctx.lineTo(c + Math.cos(a) * (r0 + len), c + Math.sin(a) * (r0 + len));
          ctx.stroke();
        }
        // Centre diamond.
        const d = s * (o.bezel ? 0.03 : 0.1) * (1 + smooth * 0.35);
        ctx.fillStyle = mix(GOLD, GOLD, 1);
        ctx.beginPath();
        ctx.moveTo(c, c - d);
        ctx.lineTo(c + d, c);
        ctx.lineTo(c, c + d);
        ctx.lineTo(c - d, c);
        ctx.closePath();
        ctx.fill();
      },
    };
  }

  // ------------------------------------------------------------ hero
  const heroCanvas = document.getElementById("iris");
  const hero = makeIris(heroCanvas, { ticks: 72, bezel: true, inner: 0.19, maxLen: 0.2 });
  const roLevel = document.getElementById("ro-level");
  const roSrc = document.getElementById("ro-src");
  let heroVisible = true;
  new IntersectionObserver((e) => (heroVisible = e[0].isIntersecting)).observe(heroCanvas);

  const micBtn = document.getElementById("mic-btn");
  micBtn.setAttribute("aria-pressed", "false");
  micBtn.addEventListener("click", async () => {
    if (mic) {
      mic.stream.getTracks().forEach((t) => t.stop());
      mic.ctx.close();
      mic = null;
      micBtn.textContent = "Try with your mic";
      micBtn.setAttribute("aria-pressed", "false");
      roSrc.textContent = "simulated";
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const ac = new AudioContext();
      const src = ac.createMediaStreamSource(stream);
      const analyser = ac.createAnalyser();
      analyser.fftSize = 1024;
      src.connect(analyser);
      mic = { stream, ctx: ac, analyser, data: new Float32Array(analyser.fftSize) };
      micBtn.textContent = "Stop mic";
      micBtn.setAttribute("aria-pressed", "true");
      roSrc.textContent = "microphone";
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
  const pillIris = makeIris(document.getElementById("pill-iris"), { ticks: 16, bezel: false, inner: 0.22, maxLen: 0.26 });
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
