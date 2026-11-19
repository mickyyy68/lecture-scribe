// Renders the deck (JSON from the slide model) as 1280×720 slides with the theme classes
// (theme/base.css). Used by the app's slide panel and by the exported "· Slides.html" deck. All
// text is escaped. Fixed labels follow the slides' language, from <html lang>.
(function () {
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

  const LABELS = {
    en: { def: "Definition", notice: "Notice", check: "Check", aside: "Aside", review: "Review", reviewTitle: "Review questions", sketch: "sketch",
      sketchTip: "Qualitative drawing: it shows the idea, not real data", plot: "Plot", unsure: "Uncertain fix of the transcript",
      notFound: "Not found in the transcript", reveal: "Click to see the answer", hint: "Click a slide or press P to present · ← → to move · Esc to leave" },
    it: { def: "Definizione", notice: "Avviso", check: "Da verificare", aside: "A margine", review: "Ripasso", reviewTitle: "Domande di ripasso", sketch: "schizzo",
      sketchTip: "Disegno qualitativo: illustra l'idea, non dati reali", plot: "Grafico", unsure: "Correzione incerta della trascrizione",
      notFound: "Non trovato nella trascrizione", reveal: "Clicca per vedere la risposta", hint: "Clicca una slide o premi P per presentare · ← → per scorrere · Esc per uscire" },
    fr: { def: "Définition", notice: "Avis", check: "À vérifier", aside: "En marge", review: "Révision", reviewTitle: "Questions de révision", sketch: "croquis",
      sketchTip: "Dessin qualitatif : il illustre l'idée, pas des données réelles", plot: "Graphique", unsure: "Correction incertaine de la transcription",
      notFound: "Introuvable dans la transcription", reveal: "Cliquez pour voir la réponse", hint: "Cliquez une diapositive ou appuyez sur P pour présenter · ← → pour avancer · Échap pour sortir" },
    de: { def: "Definition", notice: "Hinweis", check: "Prüfen", aside: "Am Rande", review: "Wiederholung", reviewTitle: "Wiederholungsfragen", sketch: "Skizze",
      sketchTip: "Qualitative Zeichnung: zeigt die Idee, keine echten Daten", plot: "Diagramm", unsure: "Unsichere Korrektur des Transkripts",
      notFound: "Nicht im Transkript gefunden", reveal: "Klicken, um die Antwort zu sehen", hint: "Folie anklicken oder P drücken zum Präsentieren · ← → zum Blättern · Esc zum Beenden" },
    es: { def: "Definición", notice: "Aviso", check: "A verificar", aside: "Al margen", review: "Repaso", reviewTitle: "Preguntas de repaso", sketch: "boceto",
      sketchTip: "Dibujo cualitativo: ilustra la idea, no datos reales", plot: "Gráfico", unsure: "Corrección incierta de la transcripción",
      notFound: "No está en la transcripción", reveal: "Haz clic para ver la respuesta", hint: "Haz clic en una diapositiva o pulsa P para presentar · ← → para avanzar · Esc para salir" },
    pt: { def: "Definição", notice: "Aviso", check: "A verificar", aside: "À margem", review: "Revisão", reviewTitle: "Perguntas de revisão", sketch: "esboço",
      sketchTip: "Desenho qualitativo: ilustra a ideia, não dados reais", plot: "Gráfico", unsure: "Correção incerta da transcrição",
      notFound: "Não encontrado na transcrição", reveal: "Clique para ver a resposta", hint: "Clique num slide ou prima P para apresentar · ← → para avançar · Esc para sair" },
  };
  const say = (k) => (LABELS[(document.documentElement.lang || "en").slice(0, 2).toLowerCase()] || LABELS.en)[k];
  window.slideLabel = say;

  // ---- LaTeX: $…$ in any text, "math" blocks. KaTeX is bundled; without it the source shows as code.
  // `$x$`: no space just inside the dollars and no digit right after, so "$5 e $10" stays text.
  // A tab or newline right after "$" may be a broken \theta or \nabla (see fixTex), so it's allowed.
  const MATH = /\$(?![ $]|\n(?![A-Za-z]))([^$]*?[^\s\\$])\$(?!\d)/g;
  // In JSON, "\t", "\f", "\b", "\r", "\n" are valid escapes, so a forgotten double backslash in
  // \theta, \frac, \beta, \rho or \nabla arrives as a control character. Put the command back.
  const fixTex = (s) =>
    String(s ?? "").replace(/\f/g, "\\f").replace(/\x08/g, "\\b").replace(/\t(?=[a-zA-Z])/g, "\\t").replace(/\r(?=[a-zA-Z])/g, "\\r").replace(/\n(?=[a-zA-Z])/g, "\\n");
  function tex(src, display) {
    const s = fixTex(src);
    if (window.katex) {
      try { return katex.renderToString(s, { displayMode: !!display, throwOnError: false, output: "html", strict: "ignore" }); } catch (e) { /* shown as code */ }
    }
    return `<code class="ic">${esc(s)}</code>`;
  }
  // SVG labels can't hold KaTeX: math becomes Unicode (x_1 → x₁, \alpha → α, h^{(1)} → h⁽¹⁾).
  const SYM = { alpha: "α", beta: "β", gamma: "γ", delta: "δ", epsilon: "ε", varepsilon: "ε", zeta: "ζ", eta: "η", theta: "θ", iota: "ι", kappa: "κ", lambda: "λ", mu: "μ", nu: "ν", xi: "ξ", pi: "π", rho: "ρ", sigma: "σ", tau: "τ", phi: "φ", varphi: "φ", chi: "χ", psi: "ψ", omega: "ω",
    Gamma: "Γ", Delta: "Δ", Theta: "Θ", Lambda: "Λ", Xi: "Ξ", Pi: "Π", Sigma: "Σ", Phi: "Φ", Psi: "Ψ", Omega: "Ω",
    to: "→", rightarrow: "→", leftarrow: "←", gets: "←", leftrightarrow: "↔", Rightarrow: "⇒", Leftrightarrow: "⇔", cdot: "·", times: "×", le: "≤", leq: "≤", ge: "≥", geq: "≥", neq: "≠", ne: "≠", approx: "≈", sim: "∼", infty: "∞",
    sum: "∑", prod: "∏", partial: "∂", nabla: "∇", in: "∈", notin: "∉", neg: "¬", lnot: "¬", land: "∧", wedge: "∧", lor: "∨", vee: "∨", forall: "∀", exists: "∃", pm: "±", ldots: "…", dots: "…", cdots: "⋯", emptyset: "∅", cup: "∪", cap: "∩", subseteq: "⊆", subset: "⊂", mid: "|", top: "⊤", bot: "⊥", log: "log", exp: "exp", max: "max", min: "min", arg: "arg" };
  const SUB = { 0: "₀", 1: "₁", 2: "₂", 3: "₃", 4: "₄", 5: "₅", 6: "₆", 7: "₇", 8: "₈", 9: "₉", "+": "₊", "-": "₋", "=": "₌", "(": "₍", ")": "₎", a: "ₐ", e: "ₑ", h: "ₕ", i: "ᵢ", j: "ⱼ", k: "ₖ", l: "ₗ", m: "ₘ", n: "ₙ", o: "ₒ", p: "ₚ", r: "ᵣ", s: "ₛ", t: "ₜ", u: "ᵤ", v: "ᵥ", x: "ₓ", ",": ",", " ": " " };
  const SUP = { 0: "⁰", 1: "¹", 2: "²", 3: "³", 4: "⁴", 5: "⁵", 6: "⁶", 7: "⁷", 8: "⁸", 9: "⁹", "+": "⁺", "-": "⁻", "=": "⁼", "(": "⁽", ")": "⁾", n: "ⁿ", i: "ⁱ", T: "ᵀ", "*": "*", "'": "′", ",": ",", " ": " " };
  function texPlain(t) {
    t = fixTex(t)
      .replace(/\\(?:mathrm|mathbf|mathit|mathcal|mathbb|text|textbf|textit|operatorname|hat|bar|vec|tilde|overline|boldsymbol)\s*\{([^{}]*)\}/g, "$1")
      .replace(/\\[dt]?frac\s*\{([^{}]*)\}\s*\{([^{}]*)\}/g, (_, a, b) => `${a.length > 1 ? `(${a})` : a}/${b.length > 1 ? `(${b})` : b}`)
      .replace(/\\sqrt\s*\{([^{}]*)\}/g, "√($1)")
      .replace(/\\left|\\right/g, "")
      .replace(/\\([A-Za-z]+)/g, (m, w) => SYM[w] ?? w)
      .replace(/\\[,;:! ]/g, " ")
      .replace(/\\([{}_$%#&])/g, "$1");
    const conv = (s, map, op) => ([...s].every((c) => c in map) ? [...s].map((c) => map[c]).join("") : op + s);
    t = t.replace(/([_^])\{([^{}]*)\}|([_^])([^\s{}])/g, (m, a, g, b, c) => ((a || b) === "_" ? conv(g ?? c, SUB, "_") : conv(g ?? c, SUP, "^")));
    return t.replace(/[{}]/g, "");
  }
  // Diagram labels are plain text: drop markdown backticks, turn math into Unicode.
  const plain = (s) => String(s ?? "").replace(/`/g, "").replace(MATH, (_, m) => texPlain(m));

  const UNSURE = () => `<span class="unsure" title="${esc(say("unsure"))}">(?)</span>`;
  // `code`, $math$, **bold** and "(?)". Code and math are set aside first so ** or $ inside them stay literal.
  function inline(s) {
    const keep = [];
    const hold = (h) => `\u0000${keep.push(h) - 1}\u0000`;
    const t = String(s ?? "")
      .replace(/`([^`]+)`/g, (_, c) => hold(`<code class="ic">${esc(c)}</code>`))
      .replace(MATH, (_, m) => hold(tex(m)));
    return esc(t)
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\(\?\)/g, UNSURE())
      .replace(/\u0000(\d+)\u0000/g, (_, i) => keep[+i]);
  }
  const two = (n) => String(n).padStart(2, "0");

  // ---- Arithmetic for "plot": x, numbers, + - * / ^, ( ), pi, e and the usual functions. No eval.
  const FN = { exp: Math.exp, ln: Math.log, log: Math.log, log2: Math.log2, log10: Math.log10, sqrt: Math.sqrt, abs: Math.abs, sin: Math.sin, cos: Math.cos, tan: Math.tan,
    tanh: Math.tanh, sinh: Math.sinh, cosh: Math.cosh, floor: Math.floor, ceil: Math.ceil, sign: Math.sign, round: Math.round, max: Math.max, min: Math.min, pow: Math.pow,
    sigmoid: (v) => 1 / (1 + Math.exp(-v)), relu: (v) => Math.max(0, v), step: (v) => (v >= 0 ? 1 : 0), heaviside: (v) => (v >= 0 ? 1 : 0) };
  function compile(src) {
    const s = String(src ?? "").replace(/^\s*(?:[a-zA-Z]\w*\s*\(\s*x\s*\)|y)\s*=/, "").replace(/\*\*/g, "^").replace(/·|×/g, "*").replace(/−/g, "-");
    const toks = s.match(/\d+\.?\d*(?:[eE][-+]?\d+)?|\.\d+|[A-Za-z_]\w*|[-+*/^(),]/g) || [];
    if (toks.join("") !== s.replace(/\s+/g, "")) return null; // unknown characters
    let i = 0;
    const peek = () => toks[i], take = (t) => (toks[i] === t ? (i++, true) : false);
    const fail = () => { throw 0; };
    function expr() {
      let a = term();
      for (;;) { if (take("+")) { const l = a, r = term(); a = (x) => l(x) + r(x); } else if (take("-")) { const l = a, r = term(); a = (x) => l(x) - r(x); } else return a; }
    }
    function term() {
      let a = unary();
      for (;;) {
        if (take("*")) { const l = a, r = unary(); a = (x) => l(x) * r(x); }
        else if (take("/")) { const l = a, r = unary(); a = (x) => l(x) / r(x); }
        else if (peek() === "(" || /^[A-Za-z_.\d]/.test(peek() || "")) { const l = a, r = unary(); a = (x) => l(x) * r(x); } // 2x, 2(x+1)
        else return a;
      }
    }
    function unary() { if (take("-")) { const a = unary(); return (x) => -a(x); } if (take("+")) return unary(); return power(); }
    function power() { const a = atom(); if (take("^")) { const b = unary(); return (x) => Math.pow(a(x), b(x)); } return a; }
    function atom() {
      const t = toks[i++];
      if (t == null) fail();
      if (/^[\d.]/.test(t)) { const v = parseFloat(t); return () => v; }
      if (t === "(") { const a = expr(); if (!take(")")) fail(); return a; }
      if (t === "x") return (x) => x;
      if (t === "pi") return () => Math.PI;
      if (t === "e") return () => Math.E;
      const f = FN[t];
      if (!f || !take("(")) fail();
      const args = [expr()];
      while (take(",")) args.push(expr());
      if (!take(")")) fail();
      return (x) => f(...args.map((g) => g(x)));
    }
    try { const f = expr(); return i === toks.length ? f : null; } catch (e) { return null; }
  }
  const num = (v) => (v === null || v === "" || v === undefined || !isFinite(+v) ? null : +v);
  // About n round ticks between lo and hi.
  function ticks(lo, hi, n) {
    const raw = (hi - lo) / Math.max(1, n), p = Math.pow(10, Math.floor(Math.log10(raw))), m = raw / p;
    const step = (m < 1.5 ? 1 : m < 3 ? 2 : m < 7 ? 5 : 10) * p, out = [];
    for (let v = Math.ceil(lo / step - 1e-9) * step; v <= hi + step * 1e-9; v += step) out.push(Math.abs(v) < step * 1e-9 ? 0 : v);
    return out;
  }
  const fmtNum = (v) => { const a = Math.abs(v); return a !== 0 && (a >= 1e5 || a < 1e-3) ? v.toExponential(1) : String(+v.toPrecision(4)); };
  let uid = 0;

  // ---- "graph": nodes and edges laid out in layers (trees, search graphs, networks, state machines).
  function graph(b) {
    const nodes = (b.nodes || []).filter((n) => n && n.id != null).slice(0, 18);
    const N = nodes.length;
    if (!N) return "";
    const idx = new Map(nodes.map((n, i) => [String(n.id), i]));
    const directed = b.directed !== false, horiz = b.dir === "right";
    const edges = (b.edges || []).map((e) => ({ ...e, a: idx.get(String(e?.from)), b: idx.get(String(e?.to)) })).filter((e) => e.a != null && e.b != null).slice(0, 40);
    // Layers: given "rank"s, else the longest path in a DAG, else breadth-first from the roots.
    let rank = nodes.map((n) => num(n.rank));
    if (rank.every((r) => r == null)) {
      const out = nodes.map(() => []), indeg = nodes.map(() => 0);
      edges.forEach((e) => { if (e.a !== e.b) { out[e.a].push(e.b); indeg[e.b]++; if (!directed) out[e.b].push(e.a); } });
      rank = nodes.map(() => -1);
      if (directed) { // Kahn: a topological order exists only without cycles
        const deg = indeg.slice(), q = nodes.map((_, i) => i).filter((i) => !deg[i]), order = [];
        q.forEach((i) => (rank[i] = 0));
        while (q.length) { const u = q.shift(); order.push(u); for (const v of out[u]) { rank[v] = Math.max(rank[v], rank[u] + 1); if (!--deg[v]) q.push(v); } }
        if (order.length < N) rank.fill(-1);
      }
      if (rank.some((r) => r < 0)) {
        rank.fill(-1);
        const roots = directed ? nodes.map((_, i) => i).filter((i) => !indeg[i]) : [];
        for (const start of roots.concat(nodes.map((_, i) => i))) {
          if (rank[start] >= 0) continue;
          rank[start] = 0;
          const q = [start];
          while (q.length) { const u = q.shift(); for (const v of out[u]) if (rank[v] < 0) { rank[v] = rank[u] + 1; q.push(v); } }
        }
      }
    }
    rank = rank.map((r) => Math.max(0, Math.round(r ?? 0)));
    const R = Math.max(...rank) + 1;
    const layers = Array.from({ length: R }, () => []);
    nodes.forEach((_, i) => layers[rank[i]].push(i));
    // Fewer crossings: order each layer by the mean position of its neighbours (a few sweeps).
    const pos = new Array(N), adj = nodes.map(() => []);
    edges.forEach((e) => { if (e.a !== e.b) { adj[e.a].push(e.b); adj[e.b].push(e.a); } });
    const setPos = () => layers.forEach((L) => L.forEach((v, k) => (pos[v] = k / Math.max(1, L.length - 1))));
    setPos();
    for (let it = 0; it < 4; it++) {
      const order = it % 2 ? [...layers.keys()].reverse() : [...layers.keys()];
      for (const r of order) {
        const ref = it % 2 ? r + 1 : r - 1;
        if (ref < 0 || ref >= R) continue;
        const key = (v) => { const ns = adj[v].filter((w) => rank[w] === ref); return ns.length ? ns.reduce((s, w) => s + pos[w], 0) / ns.length : pos[v]; };
        const keys = new Map(layers[r].map((v) => [v, key(v)]));
        layers[r].sort((p, q) => keys.get(p) - keys.get(q));
        setPos();
      }
    }
    // Node sizes: circles for short labels (a, s₀, 3), boxes otherwise.
    const label = nodes.map((n) => plain(n.label ?? n.id));
    const wide = (s) => [...s].length * 10 + 30;
    const circle = b.shape ? b.shape === "circle" : label.every((l) => [...l].length <= 3);
    const size = label.map((l) => (circle ? { w: Math.max(50, wide(l) + 4), h: Math.max(50, wide(l) + 4) } : { w: Math.max(64, wide(l)), h: 46 }));
    const elbl = edges.some((e) => e.label != null && e.label !== "");
    const subW = !horiz && nodes.some((n) => n.sub) ? Math.max(...nodes.map((n) => [...plain(n.sub || "")].length)) * 8.4 + 10 : 0;
    const longLbl = Math.max(0, ...edges.map((e) => [...plain(e.label ?? "")].length));
    const gapMain = horiz ? (elbl ? Math.max(120, longLbl * 9 + 36) : 90) : elbl ? 70 : 56, gapCross = horiz ? 26 : subW ? 70 : 34;
    const mainOf = (v) => (horiz ? size[v].w : size[v].h), crossOf = (v) => (horiz ? size[v].h : size[v].w);
    const span = layers.map((L) => L.reduce((s, v) => s + crossOf(v), 0) + gapCross * Math.max(0, L.length - 1));
    const maxSpan = Math.max(...span), thick = layers.map((L) => Math.max(...L.map(mainOf), 0));
    const main = []; let at = 0;
    layers.forEach((_, r) => { main[r] = at + thick[r] / 2; at += thick[r] + gapMain; });
    const P = new Array(N);
    layers.forEach((L, r) => {
      let c = (maxSpan - span[r]) / 2;
      L.forEach((v) => { const m = c + crossOf(v) / 2; P[v] = horiz ? { x: main[r], y: m } : { x: m, y: main[r] }; c += crossOf(v) + gapCross; });
    });
    // "sub" notes go beside the node in a top-down graph (below it they'd sit on the edges).
    const pad = 24;
    const sub = horiz && nodes.some((n) => n.sub) ? 22 : 0;
    const W = (horiz ? at - gapMain : maxSpan) + 2 * pad + subW, H = (horiz ? maxSpan : at - gapMain) + 2 * pad + sub;
    P.forEach((p) => { p.x += pad; p.y += pad; });
    // Where the line from the centre toward (dx, dy) leaves the node.
    const edgeOut = (v, dx, dy) => {
      const len = Math.hypot(dx, dy) || 1, { w, h } = size[v];
      const t = circle ? w / 2 / len : Math.min(w / 2 / Math.abs(dx || 1e-9), h / 2 / Math.abs(dy || 1e-9));
      return { x: P[v].x + dx * t, y: P[v].y + dy * t };
    };
    const pair = new Set(edges.map((e) => `${e.a}>${e.b}`));
    let g = "";
    const arrow = (x, y, a, on) => {
      const L = 12, w = 6, bx = x - Math.cos(a) * L, by = y - Math.sin(a) * L;
      return `<path class="ah${on ? " on" : ""}" d="M${x} ${y}L${bx - Math.sin(a) * w} ${by + Math.cos(a) * w}L${bx + Math.sin(a) * w} ${by - Math.cos(a) * w}Z"/>`;
    };
    const lbl = (x, y, s) => (s == null || s === "" ? "" : `<text class="el" x="${x}" y="${y + 5}" text-anchor="middle">${esc(plain(s))}</text>`);
    for (const e of edges) {
      const on = !!e.on, cls = `ed${on ? " on" : ""}${e.dashed ? " dash" : ""}`;
      if (e.a === e.b) { // self-loop above (or right of) the node
        const p = P[e.a], s = size[e.a], top = horiz ? { x: p.x + s.w / 2, y: p.y } : { x: p.x, y: p.y - s.h / 2 };
        const d = horiz ? `M${top.x} ${top.y - 8}C${top.x + 46} ${top.y - 34} ${top.x + 46} ${top.y + 34} ${top.x} ${top.y + 8}` : `M${top.x - 8} ${top.y}C${top.x - 34} ${top.y - 46} ${top.x + 34} ${top.y - 46} ${top.x + 8} ${top.y}`;
        g += `<path class="${cls}" d="${d}"/>` + (directed ? arrow(horiz ? top.x + 1 : top.x + 8, horiz ? top.y + 8 : top.y + 1, horiz ? Math.PI * 0.85 : Math.PI * 0.35, on) : "");
        g += lbl(horiz ? top.x + 50 : top.x, horiz ? top.y : top.y - 46, e.label);
        continue;
      }
      const A = P[e.a], B2 = P[e.b], dx = B2.x - A.x, dy = B2.y - A.y, len = Math.hypot(dx, dy) || 1;
      // Both directions present: bend each a little so they don't overlap.
      // Edges that skip a layer curve outward, away from the nodes in between.
      const nx = -dy / len, ny = dx / len, skip = Math.abs(rank[e.a] - rank[e.b]) > 1;
      let bend = directed && pair.has(`${e.b}>${e.a}`) ? 22 : 0;
      if (skip) {
        const mid = horiz ? (A.y + B2.y) / 2 - (pad + maxSpan / 2) : (A.x + B2.x) / 2 - (pad + maxSpan / 2);
        const side = (horiz ? ny : nx) * (mid || 1) >= 0 ? 1 : -1;
        bend = side * Math.min(90, 26 * Math.abs(rank[e.a] - rank[e.b]));
      }
      const cx = (A.x + B2.x) / 2 + nx * bend, cy = (A.y + B2.y) / 2 + ny * bend;
      const s0 = edgeOut(e.a, cx - A.x, cy - A.y), s1 = edgeOut(e.b, cx - B2.x, cy - B2.y);
      g += bend ? `<path class="${cls}" d="M${s0.x} ${s0.y}Q${cx} ${cy} ${s1.x} ${s1.y}"/>` : `<line class="${cls}" x1="${s0.x}" y1="${s0.y}" x2="${s1.x}" y2="${s1.y}"/>`;
      if (directed) g += arrow(s1.x, s1.y, Math.atan2(s1.y - cy, s1.x - cx), on);
      g += lbl(bend ? (s0.x + 2 * cx + s1.x) / 4 : cx, bend ? (s0.y + 2 * cy + s1.y) / 4 : cy, e.label);
    }
    nodes.forEach((n, v) => {
      const p = P[v], s = size[v], on = n.on ? " on" : "";
      g += `<g class="nd${on}">` + (circle ? `<circle cx="${p.x}" cy="${p.y}" r="${s.w / 2}"/>` : `<rect x="${p.x - s.w / 2}" y="${p.y - s.h / 2}" width="${s.w}" height="${s.h}"/>`) +
        `<text class="nl" x="${p.x}" y="${p.y + 6}" text-anchor="middle">${esc(label[v])}</text></g>`;
      if (n.sub) g += horiz ? `<text class="ns" x="${p.x}" y="${p.y + s.h / 2 + 18}" text-anchor="middle">${esc(plain(n.sub))}</text>`
        : `<text class="ns" x="${p.x + s.w / 2 + 6}" y="${p.y - s.h / 2 + 4}">${esc(plain(n.sub))}</text>`;
    });
    return `<figure class="s-graph">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}<svg class="pr-graph" viewBox="0 0 ${W} ${H}" style="max-width:${Math.round(W * 1.3)}px" role="img">${g}</svg></figure>`;
  }

  // ---- "plot": functions of x and/or data points on axes.
  const COLORS = ["var(--chart-4)", "var(--warning)", "var(--success)", "var(--destructive)"];
  function plot(b) {
    const X = b.x || {}, Y = b.y || {}, series = (b.series || []).filter((s) => s && (s.fn || Array.isArray(s.points))).slice(0, 4);
    if (!series.length) return "";
    const pts = series.flatMap((s) => (Array.isArray(s.points) ? s.points : [])).filter((p) => Array.isArray(p) && num(p[0]) != null && num(p[1]) != null);
    let x0 = num(X.min), x1 = num(X.max);
    if (x0 == null || x1 == null || !(x1 > x0)) {
      if (pts.length) { x0 = Math.min(...pts.map((p) => +p[0])); x1 = Math.max(...pts.map((p) => +p[0])); }
      else { x0 = -5; x1 = 5; }
      if (!(x1 > x0)) { x0 -= 1; x1 += 1; }
    }
    const data = series.map((s) => {
      if (Array.isArray(s.points)) return s.points.filter((p) => Array.isArray(p) && num(p[0]) != null && num(p[1]) != null).map((p) => [+p[0], +p[1]]);
      const f = compile(s.fn);
      if (!f) return [];
      return Array.from({ length: 241 }, (_, k) => { const x = x0 + ((x1 - x0) * k) / 240, y = f(x); return [x, Number.isFinite(y) ? y : NaN]; });
    });
    let y0 = num(Y.min), y1 = num(Y.max);
    if (y0 == null || y1 == null || !(y1 > y0)) {
      const ys = data.flat().map((p) => p[1]).filter(Number.isFinite).sort((a, c) => a - c);
      if (!ys.length) return `<div class="pr-def s-def s-check"><span class="tag">${esc(say("plot"))}</span><p class="s-checktext">${esc(series.map((s) => s.fn).join(", "))}</p></div>`;
      // Functions that blow up (1/x, log near 0) would flatten everything else: ignore the extreme 2%.
      const cut = series.some((s) => s.fn) ? Math.floor(ys.length * 0.02) : 0;
      let lo = ys[cut], hi = ys[ys.length - 1 - cut];
      if (lo > 0 && lo < (hi - lo) * 0.25) lo = 0;
      if (hi < 0 && -hi < (hi - lo) * 0.25) hi = 0;
      if (!(hi > lo)) { lo -= 1; hi += 1; }
      const m = (hi - lo) * 0.08;
      y0 = y0 ?? lo - (lo === 0 ? 0 : m); y1 = y1 ?? hi + m;
    }
    const W = 640, H = 380, L = 64, Rm = 20, T = 16, Bm = (X.label ? 62 : 40);
    const sx = (x) => L + ((x - x0) / (x1 - x0)) * (W - L - Rm), sy = (y) => T + ((y1 - y) / (y1 - y0)) * (H - T - Bm);
    const id = `pc${++uid}`;
    let g = `<clipPath id="${id}"><rect x="${L}" y="${T}" width="${W - L - Rm}" height="${H - T - Bm}"/></clipPath>`;
    for (const v of ticks(y0, y1, 5)) g += `<line class="grid" x1="${L}" x2="${W - Rm}" y1="${sy(v)}" y2="${sy(v)}"/><text class="tk" x="${L - 8}" y="${sy(v) + 5}" text-anchor="end">${fmtNum(v)}</text>`;
    for (const v of ticks(x0, x1, 6)) g += `<line class="grid" y1="${T}" y2="${H - Bm}" x1="${sx(v)}" x2="${sx(v)}"/><text class="tk" x="${sx(v)}" y="${H - Bm + 20}" text-anchor="middle">${fmtNum(v)}</text>`;
    if (y0 < 0 && y1 > 0) g += `<line class="ax0" x1="${L}" x2="${W - Rm}" y1="${sy(0)}" y2="${sy(0)}"/>`;
    if (x0 < 0 && x1 > 0) g += `<line class="ax0" y1="${T}" y2="${H - Bm}" x1="${sx(0)}" x2="${sx(0)}"/>`;
    g += `<path class="frame" d="M${L} ${T}V${H - Bm}H${W - Rm}"/>`;
    for (const mk of (b.marks || []).slice(0, 4)) {
      const x = num(mk?.x);
      if (x == null || x < x0 || x > x1) continue;
      g += `<line class="mark" y1="${T}" y2="${H - Bm}" x1="${sx(x)}" x2="${sx(x)}"/>` + (mk.label ? `<text class="ml" x="${sx(x) + 6}" y="${T + 16}">${esc(plain(mk.label))}</text>` : "");
    }
    data.forEach((d, k) => {
      const s = series[k], col = COLORS[k], line = s.fn || s.style !== "dots";
      let path = "", pen = false;
      for (const [x, y] of d) {
        if (!Number.isFinite(y) || y < y0 - (y1 - y0) * 3 || y > y1 + (y1 - y0) * 3) { pen = false; continue; }
        path += `${pen ? "L" : "M"}${sx(x).toFixed(1)} ${sy(y).toFixed(1)}`; pen = true;
      }
      if (line && path) g += `<path class="ser${k ? " s" + k : ""}" style="stroke:${col}" clip-path="url(#${id})" d="${path}"/>`;
      if (!s.fn) d.forEach(([x, y]) => { if (y >= y0 && y <= y1 && x >= x0 && x <= x1) g += `<circle class="pt" style="fill:${col}" cx="${sx(x)}" cy="${sy(y)}" r="4.5"/>`; });
    });
    const named = series.map((s, k) => [s, k]).filter(([s]) => s.label);
    named.forEach(([s, k], j) => {
      const y = T + 14 + j * 24, x = W - Rm - 210;
      g += `<line class="ser${k ? " s" + k : ""}" style="stroke:${COLORS[k]}" x1="${x}" x2="${x + 26}" y1="${y}" y2="${y}"/><text class="lg" x="${x + 34}" y="${y + 5}">${esc(plain(s.label))}</text>`;
    });
    if (X.label) g += `<text class="al" x="${(L + W - Rm) / 2}" y="${H - 8}" text-anchor="middle">${esc(plain(X.label))}</text>`;
    if (Y.label) g += `<text class="al" transform="translate(16 ${(T + H - Bm) / 2}) rotate(-90)" text-anchor="middle">${esc(plain(Y.label))}</text>`;
    return `<figure class="s-plot">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}<svg class="pr-plot" viewBox="0 0 ${W} ${H}" role="img">${g}</svg></figure>`;
  }

  // ---- "bars": a few values said in class, as horizontal bars.
  function bars(b) {
    const items = (b.items || []).filter((it) => it && num(it.value) != null).slice(0, 8);
    if (!items.length) return "";
    const W = 640, row = 46, lw = 170, vw = 90, H = items.length * row + 8;
    const hi = Math.max(0, ...items.map((it) => +it.value)), lo = Math.min(0, ...items.map((it) => +it.value)), span = hi - lo || 1;
    const sx = (v) => lw + ((v - lo) / span) * (W - lw - vw);
    let g = lo < 0 ? `<line class="ax0" x1="${sx(0)}" x2="${sx(0)}" y1="0" y2="${H}"/>` : "";
    items.forEach((it, i) => {
      const v = +it.value, y = 4 + i * row, a = sx(Math.min(0, v)), w = Math.max(2, Math.abs(sx(v) - sx(0)));
      g += `<text class="bl" x="${lw - 12}" y="${y + row / 2 + 5}" text-anchor="end">${esc(plain(it.label))}</text>`;
      g += `<rect class="bar${it.on ? " on" : ""}" x="${a}" y="${y + 8}" width="${w}" height="${row - 16}"/>`;
      g += `<text class="bv" x="${Math.max(sx(v), sx(0)) + 8}" y="${y + row / 2 + 5}">${esc(fmtNum(v) + (b.unit ? " " + plain(b.unit) : ""))}</text>`;
    });
    return `<figure class="s-bars">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}<svg class="pr-bars" viewBox="0 0 ${W} ${H}" role="img">${g}</svg></figure>`;
  }

  function cell(c) {
    if (c && typeof c === "object") {
      if ("yes" in c) return `<span class="pr-yes">${inline(c.yes)}</span>`;
      if ("no" in c) return `<span class="pr-no">${inline(c.no)}</span>`;
      return inline(c.text);
    }
    return inline(c);
  }

  const unsure = (h) => h.replace(/\(\?\)/g, `<span class="unsure" title="${esc(say("notFound"))}">(?)</span>`);
  function termLine(l) { return unsure(termLine0(l)); }
  function termLine0(l) {
    const s = String(l);
    if (s.startsWith("$ ")) return `<span class="c">$</span> <span class="k">${esc(s.slice(2))}</span>`;
    const m = s.match(/^(\S+[#>]) (.*)$/);
    if (m && s.length < 160) return `<span class="c">${esc(m[1])}</span> <span class="k">${esc(m[2])}</span>`;
    if (s.startsWith("# ") || s.startsWith("! ")) return `<span class="c">${esc(s)}</span>`;
    return esc(s);
  }

  // ---- "sketch": a qualitative drawing (clouds of points, regions, curves, arrows) on a 0–100 canvas
  // with y going up. No numbers: it illustrates what the professor described, and says so.
  function sketch(b) {
    const items = (b.items || []).filter((it) => it && typeof it === "object").slice(0, 20);
    if (!items.length) return "";
    const W = 640, H = 400, M = 18, ax = b.axes && typeof b.axes === "object";
    const L = ax ? 44 : M, Bm = ax ? 40 : M;
    const P = (x, y) => [L + (Math.max(-5, Math.min(105, +x || 0)) / 100) * (W - L - M), H - Bm - (Math.max(-5, Math.min(105, +y || 0)) / 100) * (H - Bm - M)];
    const xy = (a) => (Array.isArray(a) ? P(a[0], a[1]) : P(50, 50));
    const col = (it) => COLORS[Math.max(0, Math.min(3, Math.round(+it.color || 0)))];
    const smooth = (pts, closed) => { // Catmull-Rom through the points
      if (pts.length < 3) return pts.map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join("");
      const n = pts.length, at = (i) => (closed ? pts[(i + n) % n] : pts[Math.max(0, Math.min(n - 1, i))]);
      let d = `M${pts[0][0].toFixed(1)} ${pts[0][1].toFixed(1)}`;
      for (let i = 0; i < (closed ? n : n - 1); i++) {
        const p0 = at(i - 1), p1 = at(i), p2 = at(i + 1), p3 = at(i + 2);
        d += `C${(p1[0] + (p2[0] - p0[0]) / 6).toFixed(1)} ${(p1[1] + (p2[1] - p0[1]) / 6).toFixed(1)} ${(p2[0] - (p3[0] - p1[0]) / 6).toFixed(1)} ${(p2[1] - (p3[1] - p1[1]) / 6).toFixed(1)} ${p2[0].toFixed(1)} ${p2[1].toFixed(1)}`;
      }
      return d + (closed ? "Z" : "");
    };
    const ellipse = (it, k) => { // points of a rotated ellipse, in canvas space
      const a = ((+it.angle || 0) * Math.PI) / 180, rx = +it.rx || +it.sx || 15, ry = +it.ry || +it.sy || 10, cx = +it.cx || 50, cy = +it.cy || 50;
      return Array.from({ length: k }, (_, i) => { const t = (2 * Math.PI * i) / k, u = rx * Math.cos(t), v = ry * Math.sin(t); return P(cx + u * Math.cos(a) - v * Math.sin(a), cy + u * Math.sin(a) + v * Math.cos(a)); });
    };
    const arrowHead = (x0, y0, x1, y1, c) => {
      const a = Math.atan2(y1 - y0, x1 - x0), L2 = 12, w = 6, bx = x1 - Math.cos(a) * L2, by = y1 - Math.sin(a) * L2;
      return `<path style="fill:${c}" d="M${x1} ${y1}L${bx - Math.sin(a) * w} ${by + Math.cos(a) * w}L${bx + Math.sin(a) * w} ${by - Math.cos(a) * w}Z"/>`;
    };
    // Labels stay inside the canvas: one that would run off the right edge is right-aligned instead.
    const text = (x, y, s, anchor, cls) => {
      if (s == null || s === "") return "";
      const t = plain(s), w = [...t].length * 8.6;
      anchor = anchor || "start";
      if (anchor === "start" && x + w > W - 4) { anchor = "end"; x = Math.min(x - 16, W - 4); }
      if (anchor === "end" && x - w < 4) { anchor = "start"; x = 4; }
      if (anchor === "middle") x = Math.max(4 + w / 2, Math.min(W - 4 - w / 2, x));
      return `<text class="${cls || "sl"}" x="${x.toFixed(1)}" y="${Math.max(14, Math.min(H - 4, y)).toFixed(1)}" text-anchor="${anchor}">${esc(t)}</text>`;
    };
    let back = "", front = "";
    items.forEach((it, idx) => {
      const c = col(it), dash = it.dashed ? ' stroke-dasharray="7 5"' : "";
      switch (it.shape) {
        case "region": {
          const pts = ellipse(it, 48);
          back += `<path class="rg" style="fill:${c};stroke:${c}" d="${smooth(pts, true)}"/>`;
          const top = pts.reduce((p, q) => (q[1] < p[1] ? q : p));
          back += text(top[0], top[1] - 8, it.label, "middle");
          break;
        }
        case "cloud": { // deterministic: the same slide always draws the same points
          let seed = 1234567 + idx * 7919;
          const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
          const gauss = () => Math.sqrt(-2 * Math.log(rnd() + 1e-9)) * Math.cos(2 * Math.PI * rnd());
          const a = ((+it.angle || 0) * Math.PI) / 180, sx = +it.sx || 12, sy = +it.sy || 6, cx = +it.cx || 50, cy = +it.cy || 50;
          const n = Math.max(5, Math.min(150, Math.round(+it.n || 50)));
          for (let i = 0; i < n; i++) {
            const u = gauss() * sx, v = gauss() * sy, [x, y] = P(cx + u * Math.cos(a) - v * Math.sin(a), cy + u * Math.sin(a) + v * Math.cos(a));
            front += `<circle class="cp" style="fill:${c}" cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="3.6"/>`;
          }
          const [lx, ly] = P(cx + sx * 1.6, cy + sy * 1.9);
          front += text(lx, ly, it.label, "start");
          break;
        }
        case "curve": {
          const pts = (it.points || []).filter(Array.isArray).slice(0, 30).map(xy);
          if (pts.length < 2) break;
          front += `<path class="cv" style="stroke:${c}"${dash} d="${smooth(pts, false)}"/>`;
          if (it.arrow) { const [p, q] = pts.slice(-2); front += arrowHead(p[0], p[1], q[0], q[1], c); }
          const e = pts[pts.length - 1];
          front += text(e[0] + 8, e[1] - 8, it.label);
          break;
        }
        case "arrow":
        case "line": {
          const [x0, y0] = xy(it.from), [x1, y1] = xy(it.to);
          front += `<line class="${it.shape === "arrow" ? "ar" : "ln"}" style="stroke:${it.shape === "arrow" ? c : ""}"${dash} x1="${x0}" y1="${y0}" x2="${x1}" y2="${y1}"/>`;
          if (it.shape === "arrow") front += arrowHead(x0, y0, x1, y1, c);
          front += text(x1 + (x1 >= x0 ? 8 : -8), y1 - 8, it.label, x1 >= x0 ? "start" : "end");
          break;
        }
        case "point": {
          const [x, y] = xy(it.at);
          front += `<circle class="pt${it.hollow ? " hollow" : ""}" style="${it.hollow ? `stroke:${c}` : `fill:${c}`}" cx="${x}" cy="${y}" r="${it.on ? 8 : 6}"/>` + text(x + 12, y - 10, it.label);
          break;
        }
        case "text": {
          const [x, y] = xy(it.at);
          front += text(x, y, it.text ?? it.label, "middle", "st");
          break;
        }
      }
    });
    let axes = "";
    if (ax) {
      const [ox, oy] = P(0, 0), [ex] = P(100, 0), [, ey] = P(0, 100);
      axes = `<line class="sa" x1="${ox}" y1="${oy}" x2="${ex}" y2="${oy}"/><line class="sa" x1="${ox}" y1="${oy}" x2="${ox}" y2="${ey}"/>` +
        arrowHead(ox, oy, ex + 6, oy, "var(--foreground)") + arrowHead(ox, oy, ox, ey - 6, "var(--foreground)") +
        text((ox + ex) / 2, H - 10, b.axes.x, "middle", "al") + (b.axes.y ? `<text class="al" transform="translate(16 ${(oy + ey) / 2}) rotate(-90)" text-anchor="middle">${esc(plain(b.axes.y))}</text>` : "");
    }
    return `<figure class="s-sketch">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}<div class="s-sk"><span class="s-badge" title="${esc(say("sketchTip"))}">${esc(say("sketch"))}</span>` +
      `<svg class="pr-sketch" viewBox="0 0 ${W} ${H}" role="img">${back}${axes}${front}</svg></div></figure>`;
  }

  // ---- "timeline": a few steps or eras, left to right.
  const timeline = (b) => {
    const items = (b.items || []).filter((it) => it && typeof it === "object").slice(0, 6);
    if (!items.length) return "";
    return `<figure class="s-tl">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}<ol style="grid-template-columns:repeat(${items.length},minmax(0,1fr))">` +
      items.map((it) => `<li class="${it.on ? "on" : ""}"><span class="when">${inline(it.when || "")}</span><i class="rail"></i><b>${inline(it.title)}</b>${it.text ? `<p>${inline(it.text)}</p>` : ""}</li>`).join("") +
      `</ol></figure>`;
  };

  // ---- "arch": a network architecture as layers left to right, sized by width (autoencoder funnel, CNN…).
  function arch(b) {
    const layers = (b.layers || []).filter((l) => l && typeof l === "object").slice(0, 10);
    if (!layers.length) return "";
    const n = layers.length, W = Math.max(420, Math.min(860, 92 * n + 60)), top = 34, maxH = 230;
    const sizes = layers.map((l) => num(String(l.size ?? "").replace(/[^\d.]/g, "")));
    const known = sizes.filter((v) => v != null && v > 0), lo = Math.min(...known), hi = Math.max(...known);
    const rel = layers.map((l, i) => {
      const w = num(l.w);
      if (w != null) return Math.max(0.08, Math.min(1, w));
      if (sizes[i] != null && sizes[i] > 0 && hi > lo) return 0.2 + (0.8 * Math.log(sizes[i] / lo)) / Math.log(hi / lo);
      return l.kind === "latent" ? 0.3 : 0.65;
    });
    const step = (W - 60) / n, bw = Math.min(54, step * 0.42), cy = top + maxH / 2;
    const X = (i) => 30 + step * (i + 0.5), hgt = (i) => 24 + rel[i] * (maxH - 24);
    let g = "";
    for (let i = 0; i + 1 < n; i++) { // connectors: the funnel between consecutive layers
      const x0 = X(i) + bw / 2, x1 = X(i + 1) - bw / 2, h0 = hgt(i) / 2, h1 = hgt(i + 1) / 2;
      g += `<path class="cn" d="M${x0} ${cy - h0}L${x1} ${cy - h1}L${x1} ${cy + h1}L${x0} ${cy + h0}Z"/>`;
    }
    layers.forEach((l, i) => {
      const h = hgt(i), cls = ["ly", l.kind || "", l.on ? "on" : ""].filter(Boolean).join(" ");
      g += `<rect class="${cls}" x="${X(i) - bw / 2}" y="${cy - h / 2}" width="${bw}" height="${h}"/>`;
      if (l.size != null && l.size !== "") g += `<text class="sz" x="${X(i)}" y="${cy - h / 2 - 9}" text-anchor="middle">${esc(plain(l.size))}</text>`;
      g += `<text class="nm" x="${X(i)}" y="${top + maxH + 24}" text-anchor="middle">${esc(plain(l.name))}</text>`;
    });
    let gy = top + maxH + 44;
    const groups = (b.groups || []).slice(0, 4).map((gr) => { const a = Math.max(0, Math.min(n - 1, Math.round(+gr.from || 0))); return { a, z: Math.max(a, Math.min(n - 1, Math.round(num(gr.to) ?? a))), label: gr.label }; });
    // A layer shared by two groups (the latent z between Encoder and Decoder): brackets meet at its centre.
    const shared = (i) => groups.filter((g2) => g2.a === i || g2.z === i).length > 1;
    for (const { a, z, label } of groups) {
      const gr = { label };
      const x0 = shared(a) ? X(a) + 3 : X(a) - bw / 2, x1 = shared(z) ? X(z) - 3 : X(z) + bw / 2;
      const t = plain(gr.label), fs = Math.max(9, Math.min(14, (x1 - x0 - 6) / Math.max(1, [...t].length * 0.62)));
      g += `<path class="br" d="M${x0} ${gy}V${gy + 8}H${x1}V${gy}"/><text class="gl" style="font-size:${fs.toFixed(1)}px" x="${(x0 + x1) / 2}" y="${gy + 28}" text-anchor="middle">${esc(t)}</text>`;
    }
    const H = (b.groups || []).length ? gy + 40 : top + maxH + 36;
    return `<figure class="s-arch">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}<svg class="pr-arch" viewBox="0 0 ${W} ${H}" style="max-width:${Math.round(W * 1.25)}px" role="img">${g}</svg></figure>`;
  }

  // ---- "algo": pseudocode, numbered, indented by leading spaces, keywords in bold.
  const KW = /^(per ogni|per|for each|for|while|finché|mentre|if|se|else if|altrimenti se|else|altrimenti|return|restituisci|ritorna|repeat|ripeti|until|fino a|end|fine|function|funzione|procedure|procedura|input|output|do|allora|then)\b/i;
  const algo = (b) => {
    const hl = new Set(b.hl || []);
    const lines = (b.lines || []).slice(0, 14).map((l, i) => {
      const s = String(l ?? ""), ind = s.length - s.trimStart().length, body = s.trimStart();
      const m = body.match(KW);
      const html = m ? `<b class="kw">${esc(m[0])}</b>${inline(body.slice(m[0].length))}` : inline(body);
      return `<li class="${hl.has(i) ? "hl" : ""}" style="padding-left:${Math.min(8, Math.round(ind / 2)) * 22}px">${html}</li>`;
    });
    return `<div class="pr-term s-algo"><div class="bar"><span>${esc(plain(b.title || "Algoritmo"))}</span><span>pseudocodice</span></div><ol>${lines.join("")}</ol></div>`;
  };

  // ---- "code": a short Python listing with light syntax colouring.
  const PYKW = new Set("False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield".split(" "));
  function pyLine(s) {
    let out = "";
    const re = /(#.*$)|("(?:\\.|[^"\\])*"?|'(?:\\.|[^'\\])*'?)|(\b\d+(?:\.\d*)?(?:e[-+]?\d+)?\b)|([A-Za-z_]\w*)(?=\s*\()|([A-Za-z_]\w*)|(\s+|.)/g;
    for (const m of s.matchAll(re)) {
      if (m[1]) out += `<span class="cm">${esc(m[1])}</span>`;
      else if (m[2]) out += `<span class="st">${esc(m[2])}</span>`;
      else if (m[3]) out += `<span class="nu">${esc(m[3])}</span>`;
      else if (m[4]) out += PYKW.has(m[4]) ? `<span class="kw">${esc(m[4])}</span>` : `<span class="fn">${esc(m[4])}</span>`;
      else if (m[5]) out += PYKW.has(m[5]) ? `<span class="kw">${esc(m[5])}</span>` : esc(m[5]);
      else out += esc(m[6]);
    }
    return out;
  }
  const code = (b) => {
    const hl = new Set(b.hl || []);
    const lines = (b.lines || []).slice(0, 14).map((l, i) => {
      let s = String(l ?? ""), unsureMark = "";
      if (/\s\(\?\)$/.test(s)) { s = s.replace(/\s*\(\?\)$/, ""); unsureMark = ` <span class="unsure" title="${esc(say("notFound"))}">(?)</span>`; }
      const h = pyLine(s) + unsureMark;
      return hl.has(i) ? `<span class="hl">${h}</span>` : h;
    });
    return `<div class="pr-term s-code"><div class="bar"><span>${esc(plain(b.caption || b.file || ""))}</span><span>${esc(b.lang || "python")}</span></div><pre>${lines.join("\n")}</pre></div>`;
  };

  const B = {
    p: (b) => `<p class="body-lg s-p">${inline(b.text)}</p>`,
    list: (b) =>
      `<ol class="pr-list body s-list">${(b.items || []).slice(0, 6).map((it, i) => `<li><span class="n">${two(i + 1)}</span><span>${inline(it)}</span></li>`).join("")}</ol>`,
    def: (b) =>
      `<div class="pr-def s-def"><span class="tag">${esc(b.tag || say("def"))}</span>` +
      `<p class="term">${inline(b.term)}</p><p class="text">${inline(b.text)}</p>` +
      (b.src ? `<p class="src">${esc(b.src)}</p>` : "") + `</div>`,
    table: (b) =>
      `<table class="pr-table s-table"><thead><tr>${(b.head || []).map((h) => `<th>${esc(h)}</th>`).join("")}</tr></thead><tbody>` +
      (b.rows || []).slice(0, 6).map((r, i) => `<tr${b.focus === i ? ' class="on"' : ""}>${(r || []).map((c) => `<td>${cell(c)}</td>`).join("")}</tr>`).join("") +
      `</tbody></table>`,
    term: (b) => {
      const hl = new Set(b.hl || []);
      const lines = (b.lines || []).slice(0, 10).map((l, i) => (hl.has(i) ? `<span class="hl">${termLine(l)}</span>` : termLine(l)));
      return `<div class="pr-term s-term"><div class="bar"><span>${esc(b.host || "terminale")}</span><span>${esc(b.tool || "")}</span></div><pre>${lines.join("\n")}</pre></div>`;
    },
    header: (b) => {
      const bits = `<div class="pr-bits"><span style="grid-column:1">0</span><span style="grid-column:9">8</span><span style="grid-column:17">16</span><span style="grid-column:25">24</span><span style="grid-column:32">31</span></div>`;
      const rows = (b.rows || []).slice(0, 7).map((r) =>
        `<div class="pr-row">${(r || []).map((f) => {
          const span = Math.max(1, Math.min(32, f.opt ? 32 : Number(f.bits) || 8));
          return `<div class="pr-field${f.on ? " on" : ""}${f.opt ? " opt" : ""}" style="grid-column:span ${span}">${esc(f.name)}${f.bits && !f.opt ? ` <small>${esc(f.bits)}</small>` : ""}</div>`;
        }).join("")}</div>`).join("");
      return `<div class="pr-header s-header">${b.caption ? `<p class="s-cap">${esc(b.caption)}</p>` : ""}${bits}${rows}</div>`;
    },
    stack: (b) => {
      const focus = b.focus != null, lv = ["pr-l5", "pr-l4", "pr-l3", "pr-l2", "pr-l1"];
      return `<div class="pr-stack${focus ? " is-focus" : ""}">${(b.layers || []).slice(0, 7)
        .map((l, i) => `<div class="pr-layer ${lv[Math.min(i, 4)]}${focus && b.focus === i ? " on" : ""}"><span class="k">${esc(l.k || "")}</span><span>${esc(l.name)}</span><span class="p">${esc(l.p || "")}</span></div>`)
        .join("")}</div>`;
    },
    seq: (b) => {
      const hosts = (b.hosts || []).slice(0, 3), msgs = (b.msgs || []).slice(0, 6);
      if (hosts.length < 2) return "";
      // Few messages: a narrower canvas, so the diagram is drawn larger in its column.
      const W = msgs.length <= 2 ? 420 : msgs.length <= 4 ? 500 : 560, M = 76, top = 48, row = 76, slant = 22;
      const x = (i) => M + (Math.max(0, Math.min(hosts.length - 1, Number(i) || 0)) * (W - 2 * M)) / (hosts.length - 1);
      const H = top + 34 + msgs.length * row;
      let g = "";
      hosts.forEach((h, i) => {
        g += `<line class="life" x1="${x(i)}" y1="${top}" x2="${x(i)}" y2="${H}"/>`;
        g += `<rect class="node" x="${x(i) - 72}" y="2" width="144" height="${top - 6}"/><text class="host" x="${x(i)}" y="${top / 2 + 6}" text-anchor="middle">${esc(plain(h))}</text>`;
      });
      msgs.forEach((m, k) => {
        const x0 = x(m.from), x1 = x(m.to), y0 = top + 34 + k * row, y1 = y0 + slant, lost = !!m.lost;
        const dir = Math.sign(x1 - x0) || 1;
        const ex = lost ? x0 + (x1 - x0) * 0.62 : x1 - dir * 2, ey = lost ? y0 + slant * 0.62 : y1;
        const mx = (x0 + x1) / 2, my = (y0 + y1) / 2;
        g += `<circle class="dot" cx="${x0}" cy="${y0}" r="4"/><line class="msg${lost ? " lost" : ""}" x1="${x0}" y1="${y0}" x2="${ex}" y2="${ey}"/>`;
        if (lost) g += `<path class="msg lost" style="stroke-dasharray:none" d="M${ex - 8} ${ey - 8}L${ex + 8} ${ey + 8}M${ex + 8} ${ey - 8}L${ex - 8} ${ey + 8}"/>`;
        else {
          const a = Math.atan2(y1 - y0, x1 - x0), L = 12, w = 6, bx = x1 - Math.cos(a) * L, by = y1 - Math.sin(a) * L;
          g += `<path class="head" d="M${x1} ${y1}L${bx - Math.sin(a) * w} ${by + Math.cos(a) * w}L${bx + Math.sin(a) * w} ${by - Math.cos(a) * w}Z"/>`;
        }
        g += `<text class="lbl" x="${mx}" y="${my - 10}" text-anchor="middle">${esc(plain(m.label))}</text>`;
        const meta = [m.meta, lost ? "perso" : ""].filter(Boolean).join(" · ");
        if (meta) g += `<text class="meta" x="${mx}" y="${my + 24}" text-anchor="middle">${esc(plain(meta))}</text>`;
        if (m.note) g += `<text class="meta note" x="${mx}" y="${my + (meta ? 42 : 24)}" text-anchor="middle">${esc(plain(m.note))}</text>`;
      });
      return `<figure class="s-seq">${b.caption ? `<p class="s-cap">${esc(b.caption)}</p>` : ""}<svg class="pr-seq" viewBox="0 0 ${W} ${H + 8}" role="img">${g}</svg></figure>`;
    },
    net: (b) => {
      const nodes = (b.nodes || []).slice(0, 5), links = b.links || [];
      return `<div class="s-netw">${b.caption ? `<p class="s-cap">${esc(b.caption)}</p>` : ""}<div class="s-net">${nodes
        .map((n, i) => `<div class="s-node"><b>${esc(plain(n.name))}</b>${n.sub ? `<small>${esc(plain(n.sub))}</small>` : ""}</div>` +
          (i < nodes.length - 1 ? `<div class="s-link"><span>${esc(plain(links[i] || ""))}</span></div>` : "")).join("")}</div></div>`;
    },
    graph,
    sketch,
    timeline,
    arch,
    algo,
    code,
    aside: (b) => `<aside class="s-aside"><span class="tag">${esc(b.tag || say("aside"))}</span><p>${inline(b.text)}</p></aside>`,
    plot,
    bars,
    math: (b) => `<figure class="s-math">${b.caption ? `<p class="s-cap">${esc(plain(b.caption))}</p>` : ""}${tex(b.tex, true)}</figure>`,
    quote: (b) => `<p class="quote s-quote">${inline(b.text)}</p>`,
    notice: (b) => `<div class="pr-def s-def s-notice"><span class="tag">${esc(say("notice"))}</span><p class="s-noticetext">${inline(b.text)}</p></div>`,
    check: (b) => `<div class="pr-def s-def s-check"><span class="tag">${esc(say("check"))}</span><p class="s-checktext">${inline(b.text)}</p></div>`,
  };
  const blocks = (arr) => (Array.isArray(arr) ? arr : arr ? [arr] : []).map((b) => (B[b && b.type] || B.p)(b || {})).join("");

  // One slide. i is 1-based; foot = text on the left of the footer.
  function slide(s, i, n, foot) {
    const end = `<div class="pr-foot"><span>${esc(foot)}</span><span>${two(i)} / ${two(n)}</span></div><div class="pr-progress" style="width:${((i / n) * 100).toFixed(1)}%"></div>`;
    // Review cards: question, then the answer, hidden until clicked (shown when printing).
    if (s.kind === "review") {
      const cards = (s.cards || []).filter((c) => c && typeof c === "object").slice(0, 3);
      return `<div class="pr-slide s-review"><p class="pr-eyebrow eyebrow">${esc([s.eyebrow || say("review"), two(i)].join(" — "))}</p><h1 class="pr-title slide-title">${inline(s.title || say("reviewTitle"))}</h1>` +
        `<div class="pr-body s-cards">${cards.map((c, k) => `<div class="s-card"><span class="qn">${two(k + 1)}</span><p class="q">${inline(c.q)}</p><p class="a" title="${esc(say("reveal"))}">${inline(c.a)}</p>${c.from ? `<span class="at">${esc(c.from)}</span>` : ""}</div>`).join("")}</div>${end}</div>`;
    }
    if (s.kind === "cover")
      return `<div class="pr-slide s-cover"><p class="pr-eyebrow eyebrow">${esc(s.eyebrow || "")}</p><h1 class="pr-title cover-title">${inline(s.title)}</h1>${s.subtitle ? `<p class="pr-sub subtitle">${inline(s.subtitle)}</p>` : ""}${end}</div>`;
    const one = s.layout === "one" || !(s.right && s.right.length);
    const body = one
      ? `<div class="pr-body s-one">${blocks((s.left || []).concat(s.right || []))}</div>`
      : `<div class="pr-body"><div class="s-col">${blocks(s.left)}</div><div class="s-col">${blocks(s.right)}</div></div>`;
    const eb = [s.eyebrow, two(i)].filter(Boolean).join(" — ");
    return `<div class="pr-slide"><p class="pr-eyebrow eyebrow">${esc(eb)}</p><h1 class="pr-title slide-title">${inline(s.title)}</h1>${s.subtitle ? `<p class="pr-sub subtitle">${inline(s.subtitle)}</p>` : ""}${body}${end}</div>`;
  }

  window.renderSlide = slide;
  // Docs draw ```chart blocks and formulas with the same code (ui.html only).
  window.renderBlocks = blocks;
  window.renderTex = tex;
  // Speaker notes: same inline markup as the slides ($math$, `code`, **bold**, "(?)"). Escapes HTML.
  window.renderNotes = inline;

  // Clicking a review card shows its answer (and doesn't open the slide).
  document.addEventListener("click", (e) => {
    const c = e.target.closest && e.target.closest(".s-card");
    if (c) { c.classList.toggle("open"); e.stopPropagation(); }
  }, true);

  // If a slide's content runs into the footer, tighten it (two steps) until it fits.
  window.fitSlideText = (root) => {
    (root || document).querySelectorAll(".pr-slide").forEach((el) => {
      const body = el.querySelector(".pr-body"), foot = el.querySelector(".pr-foot");
      if (!body || !foot) return;
      el.classList.remove("dense", "denser");
      const over = () => body.offsetTop + body.scrollHeight > foot.offsetTop - 8;
      if (over()) el.classList.add("dense");
      if (over()) el.classList.add("denser");
    });
  };

  // Follow the macOS light/dark setting (the theme needs data-theme on <html>).
  const mq = matchMedia("(prefers-color-scheme: dark)");
  const apply = () => document.documentElement.setAttribute("data-theme", mq.matches ? "dark" : "light");
  apply();
  mq.addEventListener("change", apply);
})();
