// Injected before the page's own script (see runner.swift). It plays the Rust side of the IPC
// with a small fake lecture, and gives tests the `T` helpers. Commands the page sends are kept
// in T.sent(); the common ones get the reply the app would give.
(() => {
  const PATH = "/tmp/lecture-scribe-ui-test/Test Lecture.md";
  const LINES = [
    [0, "Good morning, today we start gradient descent."],
    [6, "The idea is simple: we want to minimise a loss function."],
    [14, "We take the gradient and move in the opposite direction."],
    [22, "The step size is called the learning rate, eta."],
    [31, "If eta is too large we overshoot, if it's too small it takes forever."],
    [40, "Stochastic gradient descent uses one example at a time."],
    [49, "Mini-batches are the compromise everyone uses in practice."],
    [58, "Next week there's the lab on this, bring your laptops."],
  ];
  // Past lectures in the transcripts folder, newest first, as slides::recent lists them.
  const RECENT = [
    { name: "2026-11-17 10.02 Test Lecture", path: PATH },
    { name: "2026-11-10 10.00 Backpropagation", path: "/tmp/lecture-scribe-ui-test/2026-11-10 10.00 Backpropagation.md" },
    { name: "2026-11-03 10.01 Gradient descent, part 1", path: "/tmp/lecture-scribe-ui-test/2026-11-03 10.01 Gradient descent, part 1.md" },
    { name: "2026-10-27 09.58 Linear regression", path: "/tmp/lecture-scribe-ui-test/2026-10-27 09.58 Linear regression.md" },
    { name: "2025-06-12 14.30 Networks – DHCP", path: "/tmp/lecture-scribe-ui-test/2025-06-12 14.30 Networks – DHCP.md" },
  ];
  const sent = [], errors = [];
  // The lecture's docs, as docs.rs keeps them: path → {md, t (creation order)}.
  const DOCDIR = PATH.replace(/\.md$/, " · Docs");
  const docs = {};
  let docN = 0;
  const docList = () => Object.keys(docs).sort((a, b) => docs[a].t - docs[b].t).map((p) => ({ name: p.split("/").pop().replace(/\.md$/, ""), path: p }));
  const freeDoc = (name) => { for (let n = 1; ; n++) { const p = `${DOCDIR}/${name}${n > 1 ? " " + n : ""}.md`; if (!docs[p]) return p; } };
  let lines = LINES.map((l) => [...l]);
  const fire = (e) => setTimeout(() => window.onEngine(e), 0);
  const slidesState = () => ({ type: "slides", path: PATH, title: "Test Lecture", lines: lines.length, done: 0, slides: [], lang: "en", usage: null, flagged: 0, lookups: [] });

  addEventListener("error", (e) => errors.push(e.message));
  addEventListener("unhandledrejection", (e) => errors.push(String(e.reason)));
  // wry's webview has no native dialogs: confirm() silently returns false, so they must never be used.
  for (const name of ["confirm", "alert", "prompt"]) {
    window[name] = () => { errors.push(`${name}() used: it does nothing in the app's webview (use ask())`); return name === "prompt" ? null : false; };
  }

  window.ipc = { postMessage(raw) {
    const m = JSON.parse(raw); sent.push(m);
    switch (m.cmd) {
      case "ready":
        fire({ type: "init", models: [{ label: "Parakeet V3 0.6B", langs: "multi", path: "/m/parakeet.gguf" }], modelBytes: 0,
          devices: ["Test Microphone"], defaultDevice: "Test Microphone", themeCss: "", themes: [],
          settings: { onboarded: true, model: "/m/parakeet.gguf", course: "Machine Learning" }, outDir: "/tmp/lecture-scribe-ui-test", recent: RECENT });
        break;
      case "recentList": fire({ type: "recent", recent: RECENT }); break;
      case "preload": fire({ type: "modelReady", path: m.model, ms: 1 }); break;
      case "slidesState": fire(slidesState()); break;
      case "docsList": fire({ type: "docs", path: m.path, docs: docList(), open: null }); break;
      case "docRead": fire({ type: "doc", path: m.path, doc: m.doc, name: m.doc.split("/").pop().replace(/\.md$/, ""), md: docs[m.doc].md, images: docs[m.doc].images || {} }); break;
      case "docSave": docs[m.doc].md = m.md; fire({ type: "docSaved", path: m.path, doc: m.doc, seq: m.seq }); break;
      case "docCreate": { const p = freeDoc(m.name); docs[p] = { md: m.md, t: ++docN }; fire({ type: "docs", path: m.path, docs: docList(), open: p }); break; }
      case "docRename": { const p = freeDoc(m.name); docs[p] = docs[m.doc]; delete docs[m.doc]; fire({ type: "docs", path: m.path, docs: docList(), open: p }); break; }
      case "docDelete": delete docs[m.doc]; fire({ type: "docs", path: m.path, docs: docList(), open: null }); break;
      case "docImage": fire({ type: "docImage", path: m.path, doc: m.doc, id: m.id, src: `images/pasted-${m.id}.png`, url: m.data }); break;
      case "lineEdit": {
        // What slides::edit_body does to the file.
        const text = m.text.split(/\s+/).filter(Boolean).join(" ");
        if (m.op === "insert" && text) lines.splice(m.i, 0, [lines[m.i - 1]?.[0] ?? lines[0]?.[0] ?? 0, text]);
        else if (m.op === "delete" || (m.op === "set" && !text)) lines.splice(m.i, 1);
        else if (m.op === "set") lines[m.i][1] = text;
        fire({ type: "lineEdited", path: m.path, lines: lines.map((l) => [...l]) });
        break;
      }
    }
  } };

  const native = (op, extra = {}) => window.webkit.messageHandlers.native.postMessage({ op, ...extra });
  const el = (x) => (typeof x === "string" ? document.querySelector(x) : x);
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const show = (v) => JSON.stringify(v);
  // Where to click: the element's centre, or {dx, dy} from its top-left corner.
  async function point(x, at) {
    const e = el(x); if (!e) throw new Error(`nothing matches ${x}`);
    e.scrollIntoView({ block: "center" }); await wait(20);
    const r = e.getBoundingClientRect();
    return at ? { x: r.left + at.dx, y: r.top + at.dy } : { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  }
  const tests = [];

  window.T = {
    PATH, LINES, RECENT,
    test: (name, fn) => tests.push({ name, fn }),
    sent: (cmd) => sent.filter((m) => !cmd || m.cmd === cmd),
    clearSent: () => { sent.length = 0; },
    fire: (e) => window.onEngine(e),
    wait,
    $: el,
    /** Opens the fake lecture, like picking it in "Open a past lecture". */
    async open() {
      lines = LINES.map((l) => [...l]);
      window.onEngine({ type: "transcript", path: PATH, title: "Test Lecture", lines: lines.map((l) => [...l]) });
      await T.until(() => document.querySelectorAll("#segs .seg").length === LINES.length, "the lecture's lines are shown");
      await wait(20);
    },
    docs,
    DOCDIR,
    /** Adds a doc to the fake lecture's folder. */
    addDoc(name, md, images) { const p = `${DOCDIR}/${name}.md`; docs[p] = { md, t: ++docN, images }; return p; },
    /** Opens the fake lecture and its Docs tab, and waits for the first doc (if any) to load. */
    async openDocs() {
      await T.open();
      await T.click("#docsBtn");
      const n = Object.keys(docs).length;
      await T.until(() => (n ? document.querySelector("#docPage").style.display !== "none" && Docs.doc : document.querySelector("#docEmpty").textContent.includes("No docs")), "the Docs tab to load");
      await wait(30);
    },
    /** The open doc's Markdown as last saved (waits for the autosave). */
    async saved(md, msg) { await T.until(() => docs[Docs.doc]?.md === md, `${msg || "saved"}: expected ${JSON.stringify(md)}, have ${JSON.stringify(docs[Docs.doc]?.md)}`, 3000); },
    blocks: () => [...document.querySelectorAll("#docBody > *")].map((b) => b.tagName.toLowerCase() + (b.dataset.k ? ":" + b.dataset.k : "")),
    /** A paste of plain text, as the Edit menu's Paste delivers it. */
    paste(text) { const dt = new DataTransfer(); dt.setData("text/plain", text); document.activeElement.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); return wait(40); },
    record: () => window.onEngine({ type: "state", state: "recording", device: "Test Microphone" }),
    /** The transcript line elements (`.txt`), or line i's. */
    lines: () => [...document.querySelectorAll("#segs .seg .txt")],
    line: (i) => T.lines()[i],
    texts: () => T.lines().map((t) => t.textContent),
    // Real input (AppKit events through runner.swift).
    async click(x, at) { const p = await point(x, at); await native("click", p); await wait(40); },
    async down(x, at) { const p = await point(x, at); await native("down", p); await wait(10); return p; },
    async up(p) { await native("up", p); await wait(40); },
    async type(text) { await native("type", { text }); await wait(40); },
    async key(key, mods = []) { await native("key", { key, mods }); await wait(40); },
    async shot(name) { return native("shot", { name }); },
    // Assertions.
    ok(c, msg) { if (!c) throw new Error(msg || "expected true"); },
    eq(a, b, msg) { if (show(a) !== show(b)) throw new Error(`${msg ? msg + ": " : ""}expected ${show(b)}, got ${show(a)}`); },
    async until(fn, msg, ms = 2000) {
      const end = performance.now() + ms;
      while (performance.now() < end) { if (fn()) return; await wait(20); }
      throw new Error(`timed out waiting for: ${msg || fn}`);
    },
    async __run(i) {
      if (i < 0) return JSON.stringify(tests.map((t) => t.name));
      try {
        await T.until(() => window.inited && typeof window.onEngine === "function", "the page to start", 5000);
        let timer;
        await Promise.race([tests[i].fn(), new Promise((_, no) => { timer = setTimeout(() => no(new Error("timed out after 20 s")), 20000); })]);
        clearTimeout(timer);
        await wait(50);
        if (errors.length) throw new Error("page error: " + errors.join(" | "));
        return JSON.stringify({ ok: true });
      } catch (e) {
        return JSON.stringify({ ok: false, error: String(e && e.message || e) + (errors.length ? "\npage errors: " + errors.join(" | ") : "") });
      }
    },
  };
})();
