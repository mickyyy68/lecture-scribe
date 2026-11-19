/* Docs: a lecture's own documents (Markdown files beside it, see docs.rs), in a clean editor.
   Inlined into ui.html after its main script, whose helpers it uses ($, send, esc, toast, ask,
   curPath, goToTime, engName, slideLangCode).

   The editor is one contenteditable (#docBody). Its children are the blocks:
     <p> <h1–3> <blockquote> <pre data-lang>   text blocks
     <div class="li" data-k="ul|ol|todo" data-in="0–4" [data-done]>   list items, flat like Notion
     <hr> <figure data-src> <div class="mathb" data-tex> <div class="chartb" data-src> <div class="tbl">
                                              atomic blocks (contenteditable=false; table cells
                                              and captions are editable inside)
   Inline: <b> <i> <s> <code> <a href>, time links <a class="tl" href="#t=hh:mm:ss"> and
   formulas <span class="im" data-tex>. Enter, Backspace at a block's start, Tab, paste and
   undo are handled here; typing inside a block is the browser's. After every change the blocks
   are cleaned (stray spans and styles WebKit adds), and the doc is saved as Markdown shortly
   after. Undo is our own stack of snapshots, since our block edits aren't in WebKit's. */
const Docs = (() => {
  const B = () => $("docBody");
  let lect = null, list = [], doc = null, name = "", images = {}, saved = "", saveTimer = null, seq = 0, opening = null, focusNext = false, renaming = null;
  let drafting = false, draftErr = "";
  const lastOpen = {};
  let undo = [], redo = [], lastKind = "", lastT = 0, selAtomic = null, slash = null, imgId = 0, lastCell = null;

  /* ---------- blocks ---------- */
  const mk = (tag, cls, html) => { const e = document.createElement(tag); if (cls) e.className = cls; if (html != null) e.innerHTML = html; return e; };
  const fromHtml = (html) => { const d = mk("div", null, html); return d.firstElementChild; };
  const ATOMS = ["hr", "figure", "math", "chart", "table"];
  function kind(n) {
    if (!n || n.nodeType !== 1) return null;
    const t = n.tagName;
    if (t === "P" || t === "H1" || t === "H2" || t === "H3" || t === "BLOCKQUOTE" || t === "PRE") return t.toLowerCase();
    if (t === "HR") return "hr";
    if (t === "FIGURE") return "figure";
    if (t === "DIV") for (const [c, k] of [["li", "li"], ["mathb", "math"], ["chartb", "chart"], ["tbl", "table"]]) if (n.classList.contains(c)) return k;
    return null;
  }
  const isText = (n) => { const k = kind(n); return !!k && !ATOMS.includes(k); };
  const isAtom = (n) => ATOMS.includes(kind(n));
  function blockOf(node) {
    let n = node && node.nodeType === 3 ? node.parentNode : node;
    while (n && n.parentNode !== B()) n = n.parentNode;
    return n || null;
  }
  // The editable cell or caption the node is in, if any.
  const hostOf = (node) => { const e = node && (node.nodeType === 1 ? node : node.parentElement); return e?.closest?.("#docBody td, #docBody th, #docBody figcaption") || null; };
  const idx = (n) => Array.prototype.indexOf.call(n.parentNode.childNodes, n);
  const empty = (b) => !b.textContent.replace(/\u200b/g, "") && !b.querySelector(".im,.tl");
  function fill(b) { if (!b.textContent && !b.querySelector("br,.im,.tl,img")) b.append(mk("br")); }
  function trimBr(b) { if (b.lastChild?.nodeName === "BR") b.lastChild.remove(); }
  const newP = () => mk("p", null, "<br>");

  /* ---------- Markdown → HTML ---------- */
  const texHtml = (tex, display) => {
    try { return katex.renderToString(tex, { displayMode: display, throwOnError: false, output: "html", strict: "ignore" }); } catch { return `<code>${esc(tex)}</code>`; }
  };
  const imath = (tex) => `<span class="im" contenteditable="false" data-tex="${esc(tex)}">${texHtml(tex, false)}</span>`;
  const unesc = (s) => s.replace(/\\(.)/g, "$1");
  function link(text, href) {
    if (/^#t=\d+:\d\d:\d\d$/.test(href)) return `<a class="tl" contenteditable="false" href="${esc(href)}">${esc(unesc(text))}</a>`;
    return `<a href="${esc(href)}">${inl(text)}</a>`;
  }
  // Inline Markdown: `code`, $math$, [text](href), **bold**, *italic*/_italic_, ~~strike~~, \escapes, line breaks.
  function inl(s) {
    let out = "", i = 0;
    const close = (d, from) => { for (let j = from; j <= s.length - d.length; j++) { if (s[j] === "\\") { j++; continue; } if (s.startsWith(d, j)) return j; } return -1; };
    const pair = (d, tag) => {
      if (!s.startsWith(d, i) || /\s/.test(s[i + d.length] || " ")) return false;
      const j = close(d, i + d.length);
      if (j <= i + d.length || /\s/.test(s[j - 1])) return false;
      out += `<${tag}>${inl(s.slice(i + d.length, j))}</${tag}>`; i = j + d.length; return true;
    };
    while (i < s.length) {
      const c = s[i], rest = s.slice(i);
      let m;
      if (c === "\\" && /[\\`*_{}[\]()#+\-.!|~$>]/.test(s[i + 1] || "")) { out += esc(s[i + 1]); i += 2; continue; }
      if (c === "\n") { out += "<br>"; i++; continue; }
      if (c === "`" && (m = /^(`+)([\s\S]*?[^`])\1(?!`)/.exec(rest))) { out += `<code>${esc(m[2].replace(/^ ([\s\S]*) $/, "$1"))}</code>`; i += m[0].length; continue; }
      if (c === "$" && (m = /^\$([^\s$](?:[^$\n]*?[^\s$\\])?)\$(?!\d)/.exec(rest))) { out += imath(m[1]); i += m[0].length; continue; }
      if (c === "[" && (m = /^\[((?:\\.|[^\]\\])*)\]\(([^)\s]*)\)/.exec(rest))) { out += link(m[1], m[2]); i += m[0].length; continue; }
      if (pair("**", "b") || pair("~~", "s")) continue;
      if ((c === "*" || (c === "_" && !/\w/.test(s[i - 1] || ""))) && pair(c, "i")) continue;
      out += esc(c); i++;
    }
    return out;
  }
  const LIST = /^(\s*)([-*+]|\d+[.)])\s+(\[[ xX]\]\s+)?(.*)$/;
  const startsBlock = (l, next) => /^\s*(#{1,6}\s|```|~~~|\$\$|>|!\[.*\]\(.*\)\s*$)/.test(l) || LIST.test(l) || /^\s*([-*_])(\s*\1){2,}\s*$/.test(l) || (/^\s*\|/.test(l) && /^\s*\|?\s*:?-{2,}/.test(next || ""));
  const cells = (l) => l.trim().replace(/^\|/, "").replace(/(?<!\\)\|\s*$/, "").split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, "|"));
  const TBL_TOOLS = `<div class="tbl-tools"><button data-t="row" title="Add a row">+ Row</button><button data-t="col" title="Add a column">+ Column</button><button data-t="delrow" title="Remove the row of the selected cell">− Row</button><button data-t="delcol" title="Remove the column of the selected cell">− Column</button><button data-t="del">Delete table</button></div>`;
  function tableHtml(rows) {
    const n = Math.max(1, ...rows.map((r) => r.length));
    const tr = (r, tag) => `<tr>${Array.from({ length: n }, (_, j) => `<${tag} contenteditable="true">${inl(r[j] || "")}</${tag}>`).join("")}</tr>`;
    return `<div class="tbl" contenteditable="false"><table><thead>${tr(rows[0], "th")}</thead><tbody>${rows.slice(1).map((r) => tr(r, "td")).join("")}</tbody></table>${TBL_TOOLS}</div>`;
  }
  const mathHtml = (tex) => `<div class="mathb" contenteditable="false" data-tex="${esc(tex)}">${tex.trim() ? texHtml(tex, true) : `<span class="hint-empty">Empty formula. Click to write it in LaTeX</span>`}</div>`;
  function chartInner(src) {
    let b;
    try { b = JSON.parse(src); } catch (e) { return { err: "The chart isn't valid JSON: " + e.message }; }
    if (!b || !CHARTS.includes(b.type)) return { err: `A chart is one diagram block of type ${CHARTS.join(", ")}.` };
    try { return { html: `<div class="cw"><div class="pr-slide dchart">${window.renderBlocks([b])}</div></div>` }; } catch (e) { return { err: "The chart couldn't be drawn: " + e.message }; }
  }
  const CHARTS = ["seq", "net", "graph", "plot", "bars", "sketch", "timeline", "arch", "stack", "header"];
  function chartHtml(src) {
    const r = chartInner(src);
    return `<div class="chartb" contenteditable="false" data-src="${esc(src)}">${r.html || `<div class="cerr">${esc(r.err)}</div>`}</div>`;
  }
  const figHtml = (src, alt) => `<figure contenteditable="false" data-src="${esc(src)}"><img src="${esc(images[src] || "")}" alt=""><figcaption contenteditable="true">${esc(alt)}</figcaption></figure>`;
  const codeHtml = (code, lang) => `<pre data-lang="${esc(lang)}">${esc(code)}\n</pre>`;

  function parse(md) {
    const L = md.replace(/\r\n?/g, "\n").split("\n"), out = [];
    let i = 0, m;
    while (i < L.length) {
      const line = L[i];
      if (!line.trim()) { i++; continue; }
      if ((m = /^\s*(```+|~~~+)\s*([\w+-]*)\s*$/.exec(line))) {
        const buf = []; i++;
        while (i < L.length && !L[i].trim().startsWith(m[1])) buf.push(L[i++]);
        i++;
        out.push(m[2] === "chart" ? chartHtml(buf.join("\n")) : codeHtml(buf.join("\n"), m[2])); continue;
      }
      if ((m = /^\s*\$\$(.*)$/.exec(line))) {
        const first = m[1]; let tex;
        if (first.trim() && /\$\$\s*$/.test(first)) { tex = first.replace(/\$\$\s*$/, ""); i++; }
        else {
          const buf = first.trim() ? [first] : []; i++;
          while (i < L.length && !/\$\$\s*$/.test(L[i])) buf.push(L[i++]);
          if (i < L.length) { const last = L[i].replace(/\$\$\s*$/, ""); if (last.trim()) buf.push(last); i++; }
          tex = buf.join("\n");
        }
        out.push(mathHtml(tex.trim())); continue;
      }
      if ((m = /^\s{0,3}(#{1,6})\s+(.*?)(\s+#+)?\s*$/.exec(line))) { const n = Math.min(3, m[1].length); out.push(`<h${n}>${inl(m[2]) || "<br>"}</h${n}>`); i++; continue; }
      if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) { out.push(`<hr contenteditable="false">`); i++; continue; }
      if ((m = /^\s*!\[((?:\\.|[^\]\\])*)\]\(([^)\s]+)\)\s*$/.exec(line))) { out.push(figHtml(m[2], unesc(m[1]))); i++; continue; }
      if (/^\s*\|/.test(line) && /^\s*\|?\s*:?-{2,}/.test(L[i + 1] || "")) {
        const rows = [cells(line)]; i += 2;
        while (i < L.length && /^\s*\|/.test(L[i])) rows.push(cells(L[i++]));
        out.push(tableHtml(rows)); continue;
      }
      if (/^\s*>/.test(line)) {
        const buf = []; while (i < L.length && /^\s*>/.test(L[i])) buf.push(L[i++].replace(/^\s*>\s?/, ""));
        out.push(`<blockquote>${inl(buf.join("\n").trim()) || "<br>"}</blockquote>`); continue;
      }
      if ((m = LIST.exec(line))) {
        const indent = Math.min(4, Math.floor(m[1].replace(/\t/g, "  ").length / 2));
        let text = m[4]; i++;
        while (i < L.length && L[i].trim() && /^\s{2,}/.test(L[i]) && !startsBlock(L[i], L[i + 1])) text += "\n" + L[i++].trim();
        const k = m[3] ? "todo" : /\d/.test(m[2]) ? "ol" : "ul";
        out.push(`<div class="li" data-k="${k}" data-in="${indent}"${m[3] && /x/i.test(m[3]) ? " data-done" : ""}>${inl(text) || "<br>"}</div>`); continue;
      }
      const buf = [line.trim()]; i++;
      while (i < L.length && L[i].trim() && !startsBlock(L[i], L[i + 1])) buf.push(L[i++].trim());
      out.push(`<p>${inl(buf.join("\n")) || "<br>"}</p>`);
    }
    return out.join("");
  }

  /* ---------- HTML → Markdown ---------- */
  const escMd = (t) => t.replace(/\u00a0/g, " ").replace(/\u200b/g, "").replace(/[\\`*[\]$]/g, "\\$&").replace(/~~/g, "\\~\\~")
    .replace(/_/g, (_, o, s) => (/\w/.test(s[o - 1] || "") && /\w/.test(s[o + 1] || "") ? "_" : "\\_"));
  function mdIn(node) {
    let out = "";
    const kids = [...node.childNodes];
    kids.forEach((n, k) => {
      if (n.nodeType === 3) { out += escMd(n.data); return; }
      if (n.nodeType !== 1) return;
      const t = n.tagName;
      if (t === "BR") { if (k < kids.length - 1) out += "\n"; return; }
      if (n.classList.contains("im")) { out += "$" + n.dataset.tex + "$"; return; }
      if (t === "A") {
        const href = (n.getAttribute("href") || "").replace(/[()\s]/g, encodeURIComponent);
        out += n.classList.contains("tl") ? `[${n.textContent}](${href})` : `[${mdIn(n)}](${href})`; return;
      }
      if (t === "CODE") {
        const c = n.textContent.replace(/\u00a0/g, " ");
        if (c) out += c.includes("`") ? "`` " + c + " ``" : "`" + c + "`";
        return;
      }
      const d = { B: "**", STRONG: "**", I: "*", EM: "*", S: "~~", DEL: "~~", STRIKE: "~~" }[t];
      const inner = mdIn(n);
      if (!d || !inner.trim()) { out += inner; return; }
      const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(inner);
      out += m[1] + d + m[2] + d + m[3];
    });
    return out;
  }
  // A paragraph line that would read as another block gets escaped.
  function guard(s) {
    if (/^(#{1,6}(\s|$)|>|[-+](\s|$)|\||~~~)/.test(s) || /^([-_]\s*){3,}$/.test(s)) return "\\" + s;
    const m = /^(\d+)([.)])(\s|$)/.exec(s);
    return m ? m[1] + "\\" + s.slice(m[1].length) : s;
  }
  function serialize() {
    const parts = [];
    let prevList = null;
    for (const b of B().children) {
      const k = kind(b);
      let s = null;
      const inner = () => mdIn(b).replace(/\s+$/, "");
      switch (k) {
        case "p": s = inner().split("\n").map((l) => guard(l.trim())).join("\n").trim(); break;
        case "h1": case "h2": case "h3": { const t = inner().replace(/\n/g, " ").trim(); s = t && "#".repeat(+k[1]) + " " + t; break; }
        case "blockquote": { const t = inner().trim(); s = t && t.split("\n").map((l) => "> " + l).join("\n"); break; }
        case "pre": s = "```" + (b.dataset.lang || "") + "\n" + b.textContent.replace(/\u00a0/g, " ").replace(/\n$/, "") + "\n```"; break;
        case "li": {
          const ind = "  ".repeat(+b.dataset.in || 0);
          const mark = b.dataset.k === "ol" ? (b.dataset.n || 1) + "." : b.dataset.k === "todo" ? `- [${b.hasAttribute("data-done") ? "x" : " "}]` : "-";
          s = ind + mark + " " + inner().trim().split("\n").join("\n" + ind + "  "); break;
        }
        case "hr": s = "---"; break;
        case "math": s = "$$\n" + b.dataset.tex + "\n$$"; break;
        case "chart": s = "```chart\n" + b.dataset.src + "\n```"; break;
        case "figure": if (b.dataset.src) s = `![${escMd(b.querySelector("figcaption")?.textContent || "").replace(/\n/g, " ").trim()}](${b.dataset.src})`; break;
        case "table": {
          const rows = [...b.querySelectorAll("tr")].map((tr) => [...tr.children].map((c) => mdIn(c).replace(/\s+/g, " ").replace(/\|/g, "\\|").trim()));
          if (rows.length) s = [rows[0], rows[0].map(() => "---"), ...rows.slice(1)].map((r) => "| " + r.join(" | ") + " |").join("\n");
          break;
        }
      }
      if (!s) { prevList = null; continue; }
      // Items of one list on consecutive lines; a new kind of list starts after a blank line.
      const sameList = k === "li" && prevList && (prevList.dataset.k === b.dataset.k || +b.dataset.in > 0);
      parts.push((parts.length ? (sameList ? "\n" : "\n\n") : "") + s);
      prevList = k === "li" ? b : null;
    }
    return parts.length ? parts.join("") + "\n" : "";
  }

  /* ---------- caret ---------- */
  // Where a point is: [node, offset] at `o` characters into `root`. Formulas and time links count
  // as their text, and a point inside one goes after it.
  function locate(root, o) {
    let left = o, res = null;
    (function walk(el) {
      for (const n of el.childNodes) {
        if (res) return;
        if (n.nodeType === 3) { if (left <= n.length) { res = [n, left]; return; } left -= n.length; }
        else if (n.nodeType === 1 && n.matches(".im,.tl")) {
          const len = n.textContent.length;
          if (left === 0) { res = [el, idx(n)]; return; }
          if (left <= len) { res = [el, idx(n) + 1]; return; }
          left -= len;
        } else if (n.nodeType === 1) walk(n);
      }
    })(root);
    if (res) return res;
    const n = root.childNodes.length;
    return [root, root.lastChild?.nodeName === "BR" ? n - 1 : n];
  }
  function offsetIn(root, node, off) { const r = document.createRange(); r.selectNodeContents(root); try { r.setEnd(node, off); } catch { return 0; } return r.toString().length; }
  function caretOffset(root) { const s = getSelection(); return s.rangeCount ? offsetIn(root, s.focusNode, s.focusOffset) : 0; }
  function editHost(root) { return root.closest("td,th,figcaption") || B(); }
  function caretIn(root, o = 0, scroll = true) {
    const [n, k] = locate(root, o);
    editHost(root).focus({ preventScroll: true });
    getSelection().setBaseAndExtent(n, k, n, k);
    if (scroll) (root.nodeType === 1 ? root : root.parentElement).scrollIntoView({ block: "nearest" });
  }
  function caretPos() {
    const s = getSelection();
    if (!s.rangeCount || !B().contains(s.anchorNode)) return null;
    const pos = (node, off) => {
      const b = blockOf(node); if (!b || b === B()) return { b: Math.min(off, B().children.length - 1), c: -1, o: 0 };
      const h = hostOf(node), root = h || b;
      return { b: [...B().children].indexOf(b), c: h ? [...b.querySelectorAll("td,th,figcaption")].indexOf(h) : -1, o: offsetIn(root, node, off) };
    };
    return { a: pos(s.anchorNode, s.anchorOffset), f: pos(s.focusNode, s.focusOffset) };
  }
  function setCaret(p) {
    if (!p) return;
    const pt = (x) => { const b = B().children[x.b]; if (!b) return null; const root = x.c >= 0 ? b.querySelectorAll("td,th,figcaption")[x.c] : b; return root && (isAtom(root) ? null : [root, locate(root, x.o)]); };
    const a = pt(p.a), f = pt(p.f);
    if (!a || !f) return;
    editHost(a[0]).focus({ preventScroll: true });
    getSelection().setBaseAndExtent(a[1][0], a[1][1], f[1][0], f[1][1]);
  }

  /* ---------- undo ---------- */
  const snapshot = () => ({ html: B().innerHTML, sel: caretPos() });
  function checkpoint(k = "x") {
    const now = performance.now();
    if (k !== "x" && k === lastKind && now - lastT < 1500) { lastT = now; return; }
    lastKind = k; lastT = now;
    const s = snapshot();
    if (undo.length && undo[undo.length - 1].html === s.html) return;
    undo.push(s); if (undo.length > 150) undo.shift();
    redo = [];
  }
  function restore(s) { B().innerHTML = s.html; renumber(); setCaret(s.sel); lastKind = ""; changed(true); requestAnimationFrame(fitCharts); }
  function doUndo() {
    const cur = snapshot();
    while (undo.length && undo[undo.length - 1].html === cur.html) undo.pop();
    if (!undo.length) return;
    redo.push(cur); restore(undo.pop());
  }
  function doRedo() { if (!redo.length) return; undo.push(snapshot()); restore(redo.pop()); }

  /* ---------- cleaning ---------- */
  // Keeps only our inline tags; WebKit adds <span style>, <font>, <strong>, <div> when editing.
  function cleanInline(el) {
    let dirty = false;
    for (let n of [...el.childNodes]) {
      if (n.nodeType === 3) continue;
      if (n.nodeType !== 1) { n.remove(); dirty = true; continue; }
      if (n.matches(".im,.tl")) continue;
      const to = { STRONG: "B", EM: "I", DEL: "S", STRIKE: "S" }[n.tagName];
      if (to) { const r = mk(to); r.append(...n.childNodes); n.replaceWith(r); n = r; dirty = true; }
      const t = n.tagName;
      if (t === "BR") continue;
      if (["B", "I", "S", "CODE", "A"].includes(t)) {
        for (const a of [...n.attributes]) if (!(t === "A" && a.name === "href")) { n.removeAttribute(a.name); dirty = true; }
        if (cleanInline(n)) dirty = true;
        continue;
      }
      if (t === "IMG" || t === "STYLE" || t === "SCRIPT") { n.remove(); dirty = true; continue; }
      cleanInline(n);
      const block = /^(DIV|P|H\d|LI|BLOCKQUOTE|PRE|TR|TD|TH)$/.test(t);
      if (block && n.previousSibling && n.previousSibling.nodeName !== "BR") n.before(mk("br"));
      n.replaceWith(...n.childNodes); dirty = true;
    }
    return dirty;
  }
  function normalize() {
    const body = B(), pos = caretPos();
    let dirty = false;
    for (const n of [...body.childNodes]) {
      if (n.nodeType === 3) { if (n.data.trim()) { const p = mk("p"); n.replaceWith(p); p.append(n); } else n.remove(); dirty = true; continue; }
      if (n.nodeType !== 1) { n.remove(); dirty = true; continue; }
      const k = kind(n);
      if (!k) {
        const p = mk("p");
        if (/^(B|I|S|CODE|A|SPAN|STRONG|EM|FONT|BR)$/.test(n.tagName)) { n.replaceWith(p); p.append(n); } else { p.append(...n.childNodes); n.replaceWith(p); }
        cleanInline(p); fill(p); dirty = true; continue;
      }
      if (n.hasAttribute("style")) n.removeAttribute("style");
      if (k === "pre") { if ([...n.children].some((c) => c.tagName !== "BR")) { n.textContent = n.innerText; dirty = true; } }
      else if (k === "table") n.querySelectorAll("td,th").forEach((c) => { if (cleanInline(c)) dirty = true; });
      else if (k === "figure") { const c = n.querySelector("figcaption"); if (c && cleanInline(c)) dirty = true; }
      else if (isText(n) && cleanInline(n)) dirty = true;
    }
    if (!body.lastElementChild || !isText(body.lastElementChild) || kind(body.lastElementChild) === "pre") { body.append(newP()); }
    if (dirty && pos) setCaret(pos);
  }
  function renumber() {
    const count = [];
    for (const b of B().children) {
      if (kind(b) !== "li") { count.length = 0; continue; }
      const d = +b.dataset.in || 0; count.length = d + 1;
      if (b.dataset.k === "ol") { count[d] = (count[d] || 0) + 1; b.dataset.n = count[d]; } else { count[d] = 0; delete b.dataset.n; }
    }
  }
  function fitCharts() {
    for (const c of B().querySelectorAll(".chartb")) {
      const w = c.querySelector(".cw"), d = c.querySelector(".dchart"); if (!w || !d || !c.clientWidth) continue;
      const k = Math.min(1, c.clientWidth / d.offsetWidth);
      d.style.transform = `scale(${k})`; w.style.height = Math.ceil(d.offsetHeight * k) + "px";
    }
  }
  const PH = { p: "Type / for commands", h1: "Heading 1", h2: "Heading 2", h3: "Heading 3", blockquote: "Quote" };
  function placeholders() {
    const body = B(), s = getSelection(), cur = s.rangeCount && body.contains(s.anchorNode) ? blockOf(s.anchorNode) : null;
    const only = body.children.length <= 2 && body.firstElementChild && kind(body.firstElementChild) === "p" && empty(body.firstElementChild) && (!body.children[1] || empty(body.children[1]));
    for (const b of body.children) {
      const k = kind(b), on = PH[k] && empty(b) && (b === cur || (only && b === body.firstElementChild) || k !== "p");
      b.classList.toggle("ph", !!on);
      if (on) b.dataset.ph = only && b === body.firstElementChild ? "Start writing, or type / for headings, lists, tables, formulas, charts…" : PH[k];
      if (!b.classList.length) b.removeAttribute("class");
    }
  }

  /* ---------- changes and saving ---------- */
  function changed(noCheck) {
    if (!noCheck) normalize();
    renumber(); placeholders();
    clearTimeout(saveTimer); saveTimer = setTimeout(saveNow, 700);
    status(serialize() === saved ? "Saved" : "Edited");
  }
  function saveNow() {
    clearTimeout(saveTimer);
    if (!doc || !lect) return;
    const md = serialize();
    if (md === saved) return;
    saved = md;
    send("docSave", { path: lect, doc, md, seq: ++seq });
    status("Saving…");
  }
  function status(s) { const el = $("docSavedTxt"); if (el && doc) el.textContent = s; }

  /* ---------- editing commands ---------- */
  function convert(b, to, k) {
    const n = to === "li" ? mk("div", "li") : mk(to);
    if (to === "li") { n.dataset.k = k; n.dataset.in = b.dataset.in || 0; }
    if (to === "pre") n.textContent = b.textContent + "\n"; else n.append(...b.childNodes);
    fill(n); b.replaceWith(n); return n;
  }
  function selectAtomic(el) {
    unselect(); selAtomic = el; el.classList.add("sel");
    B().focus({ preventScroll: true });
    getSelection().setBaseAndExtent(B(), idx(el), B(), idx(el));
  }
  function unselect() { selAtomic?.classList.remove("sel"); if (selAtomic && !selAtomic.classList.length) selAtomic.removeAttribute("class"); selAtomic = null; }
  function removeAtomic(el) {
    checkpoint();
    const next = el.nextElementSibling, prev = el.previousElementSibling;
    el.remove(); unselect(); normalize();
    const to = next && isText(next) ? next : prev && isText(prev) ? prev : B().lastElementChild;
    if (to) caretIn(to, to === prev ? to.textContent.length : 0);
    changed();
  }
  // Puts an atomic block after block b (in place of b when it's an empty paragraph).
  function placeAtomic(b, el) {
    if (b && kind(b) === "p" && empty(b)) b.replaceWith(el); else if (b) b.after(el); else B().append(el);
    if (!el.nextElementSibling || !isText(el.nextElementSibling)) el.after(newP());
    return el;
  }
  function enter(e, b) {
    const k = kind(b);
    if (k === "pre") {
      e.preventDefault();
      if (e.shiftKey || e.metaKey) { checkpoint(); const p = newP(); b.after(p); caretIn(p); return changed(); }
      checkpoint(); document.execCommand("insertText", false, "\n"); return changed();
    }
    e.preventDefault();
    if (e.shiftKey) { checkpoint(); document.execCommand("insertLineBreak"); return changed(); }
    const txt = b.textContent.replace(/\u00a0/g, " ").trim();
    if (k === "p" && /^```[\w+-]*$/.test(txt)) { checkpoint(); const pre = mk("pre"); pre.dataset.lang = txt.slice(3); pre.textContent = "\n"; b.replaceWith(pre); if (!pre.nextElementSibling) pre.after(newP()); caretIn(pre); return changed(); }
    if (k === "p" && txt === "$$") { checkpoint(); b.innerHTML = "<br>"; const m = placeAtomic(b, fromHtml(mathHtml(""))); changed(); return editAtomic(m); }
    if (k === "li" && empty(b)) { checkpoint(); if (+b.dataset.in) b.dataset.in = +b.dataset.in - 1; else caretIn(convert(b, "p")); return changed(); }
    checkpoint();
    const sel = getSelection();
    if (!sel.isCollapsed) document.execCommand("delete");
    const r = sel.getRangeAt(0), tail = document.createRange();
    tail.setStart(r.startContainer, r.startOffset); tail.setEnd(b, b.childNodes.length);
    const frag = tail.extractContents();
    let nb;
    if (k === "li") { nb = b.cloneNode(false); nb.removeAttribute("data-done"); }
    else if (empty(b) && k !== "p") { nb = b.cloneNode(false); const p = mk("p"); b.replaceWith(p); b = p; } // heading moves down
    else nb = mk("p");
    nb.append(frag);
    for (const x of [b, nb]) { x.querySelectorAll("b,i,s,code,a:not(.tl)").forEach((e2) => { if (!e2.textContent && !e2.querySelector(".im,.tl")) e2.remove(); }); if (x.firstChild?.nodeName === "BR" && x.childNodes.length > 1) x.firstChild.remove(); fill(x); }
    b.after(nb); caretIn(nb, 0); changed();
  }
  function backspace(e, b) {
    const k = kind(b);
    e.preventDefault();
    if (k === "li" && +b.dataset.in) { checkpoint(); b.dataset.in = +b.dataset.in - 1; return changed(); }
    if (k !== "p" && k !== "pre") { checkpoint(); caretIn(convert(b, "p")); return changed(); }
    if (k === "pre" && b.textContent.replace(/\n$/, "")) return;
    const prev = b.previousElementSibling;
    if (!prev) { if (k === "pre") { checkpoint(); caretIn(convert(b, "p")); changed(); } return; }
    if (isAtom(prev)) { if (empty(b) && b.nextElementSibling) { checkpoint(); b.remove(); } selectAtomic(prev); return changed(); }
    checkpoint();
    if (kind(prev) === "pre") {
      const code = prev.textContent.replace(/\n$/, ""), o = code.length;
      prev.textContent = code + b.textContent.replace(/\n$/, "") + "\n"; b.remove(); caretIn(prev, o); return changed();
    }
    trimBr(prev);
    const o = prev.textContent.length;
    if (k === "pre") prev.append(b.textContent.replace(/\n$/, "")); else { trimBr(b); prev.append(...b.childNodes); }
    b.remove(); prev.normalize(); fill(prev); caretIn(prev, o); changed();
  }
  function tab(e, b) {
    e.preventDefault();
    const h = hostOf(getSelection().anchorNode);
    if (h && h.matches("td,th")) {
      const all = [...b.querySelectorAll("td,th")], i = all.indexOf(h) + (e.shiftKey ? -1 : 1);
      if (i >= all.length) { checkpoint(); tableOp(b, "row", h); const c = b.querySelector("tbody tr:last-child").children[0]; caretIn(c, 0); return changed(); }
      if (i >= 0) caretIn(all[i], all[i].textContent.length);
      return;
    }
    if (kind(b) === "li") { checkpoint(); b.dataset.in = Math.max(0, Math.min(4, (+b.dataset.in || 0) + (e.shiftKey ? -1 : 1))); changed(); }
    else if (kind(b) === "pre" && !e.shiftKey) { checkpoint(); document.execCommand("insertText", false, "  "); changed(); }
  }
  function tableOp(tbl, op, cell) {
    const rows = [...tbl.querySelectorAll("tr")], n = rows[0].children.length;
    const ci = cell ? [...cell.parentNode.children].indexOf(cell) : n - 1, ri = cell ? rows.indexOf(cell.parentNode) : rows.length - 1;
    const td = (tag) => { const c = mk(tag); c.contentEditable = "true"; return c; };
    if (op === "row") { const tr = mk("tr"); for (let j = 0; j < n; j++) tr.append(td("td")); (ri <= 0 ? tbl.querySelector("tbody").prepend(tr) : rows[ri].after(tr)); }
    if (op === "col") rows.forEach((tr, r) => tr.children[ci].after(td(r ? "td" : "th")));
    if (op === "delrow" && ri > 0) rows[ri].remove();
    if (op === "delcol" && n > 1) rows.forEach((tr) => tr.children[ci].remove());
  }
  function shortcut() {
    const s = getSelection(), b = blockOf(s.anchorNode);
    if (!b || hostOf(s.anchorNode)) return false;
    const k = kind(b), before = offsetIn(b, s.focusNode, s.focusOffset), pre = b.textContent.slice(0, before).replace(/\u00a0/g, " ");
    let to = null;
    if (k === "p") {
      const m = /^(#{1,3}|[-*+]|\d+[.)]|\[ ?\]|[-*] \[ ?\]|>) $/.exec(pre);
      if (m) { const t = m[1]; to = t[0] === "#" ? ["h" + t.length] : t === ">" ? ["blockquote"] : t.includes("[") ? ["li", "todo"] : /\d/.test(t) ? ["li", "ol"] : ["li", "ul"]; }
    } else if (k === "li" && b.dataset.k === "ul" && /^\[ ?\] $/.test(pre)) to = ["li", "todo"];
    if (!to) return false;
    checkpoint();
    const r = document.createRange(); r.setStart(...locate(b, 0)); r.setEnd(...locate(b, pre.length)); r.deleteContents();
    let n = b;
    if (to[0] === "li" && k === "li") b.dataset.k = to[1]; else n = convert(b, to[0], to[1]);
    fill(n); caretIn(n, 0, false);
    return true;
  }

  /* ---------- floating: selection toolbar, slash menu, popover ---------- */
  function caretRect() {
    const s = getSelection(); if (!s.rangeCount) return null;
    const r = s.getRangeAt(0), rects = r.getClientRects();
    const rc = rects.length ? rects[0] : r.getBoundingClientRect();
    if (rc && (rc.width || rc.height)) return rc;
    const b = blockOf(s.anchorNode); return b && b !== B() ? b.getBoundingClientRect() : null;
  }
  function place(el, rc, above) {
    el.style.display = "block";
    const w = el.offsetWidth, h = el.offsetHeight;
    let top = above ? rc.top - h - 8 : rc.bottom + 6;
    if (above && top < 8) top = rc.bottom + 8;
    if (!above && top + h > innerHeight - 8) top = Math.max(8, rc.top - h - 6);
    el.style.top = top + "px";
    el.style.left = Math.max(8, Math.min(rc.left, innerWidth - w - 8)) + "px";
  }
  function toolbar() {
    const t = $("dtool"), s = getSelection();
    const on = s.rangeCount && !s.isCollapsed && B().contains(s.anchorNode) && B().contains(s.focusNode) && document.activeElement && B().contains(document.activeElement) || (s.rangeCount && !s.isCollapsed && hostOf(s.anchorNode));
    if (!on || $("dpop").style.display === "block") { t.style.display = "none"; return; }
    const r = s.getRangeAt(0).getBoundingClientRect();
    t.style.display = "block";
    place(t, { top: r.top, bottom: r.bottom, left: r.left + r.width / 2 - t.offsetWidth / 2 }, true);
  }
  function format(x) {
    const s = getSelection(); if (!s.rangeCount || !B().contains(s.anchorNode)) return;
    if (x === "link") return linkPop();
    if (s.isCollapsed) { if (x === "bold" || x === "italic") document.execCommand(x); return; }
    checkpoint();
    if (x === "code") toggleCode(); else document.execCommand(x === "strike" ? "strikeThrough" : x);
    changed(); toolbar();
  }
  function toggleCode() {
    const s = getSelection(), r = s.getRangeAt(0);
    const codeOf = (n) => (n.nodeType === 1 ? n : n.parentElement).closest("code");
    const c = codeOf(r.startContainer);
    if (c && c === codeOf(r.endContainer)) { const p = c.parentNode, i = idx(c), n = c.childNodes.length; c.replaceWith(...c.childNodes); s.setBaseAndExtent(p, i, p, i + n); return; }
    if (blockOf(r.startContainer) !== blockOf(r.endContainer)) return;
    const code = mk("code"); code.textContent = r.toString(); r.deleteContents(); r.insertNode(code);
    const nr = document.createRange(); nr.selectNodeContents(code); s.removeAllRanges(); s.addRange(nr);
  }

  const CMDS = [
    ["Text", "p", "Plain text"], ["Heading 1", "h1", "Big section heading"], ["Heading 2", "h2", "Section heading"], ["Heading 3", "h3", "Sub-heading"],
    ["Bulleted list", "ul", "- "], ["Numbered list", "ol", "1. "], ["Checklist", "todo", "[] "], ["Quote", "blockquote", "> "],
    ["Code", "code", "```"], ["Formula", "math", "LaTeX, $$"], ["Table", "table", "Rows and columns"], ["Image", "image", "From a file, or paste one"],
    ["Chart", "chart", "A diagram, as in the slides"], ["Divider", "hr", "---"], ["Time link", "time", "A moment of the lecture"],
  ];
  function openSlash() {
    const s = getSelection(), b = blockOf(s.anchorNode);
    if (!b || !isText(b) || kind(b) === "pre" || hostOf(s.anchorNode)) return;
    const at = offsetIn(b, s.focusNode, s.focusOffset), before = b.textContent.slice(0, at);
    if (!/(^|\s)\/$/.test(before.replace(/\u00a0/g, " "))) return;
    slash = { b, start: at - 1, i: 0, items: CMDS };
    renderSlash();
  }
  function slashQuery() {
    const s = getSelection(); if (!slash || !s.rangeCount || blockOf(s.anchorNode) !== slash.b) return null;
    const at = offsetIn(slash.b, s.focusNode, s.focusOffset), t = slash.b.textContent;
    if (at <= slash.start || t[slash.start] !== "/") return null;
    return t.slice(slash.start + 1, at);
  }
  function updateSlash() {
    const q = slashQuery();
    if (q == null || /\s/.test(q) || q.length > 20) return closeSlash();
    const items = CMDS.filter(([l]) => l.toLowerCase().split(" ").some((w) => w.startsWith(q.toLowerCase())) || l.toLowerCase().startsWith(q.toLowerCase()));
    if (!items.length) return closeSlash();
    slash.items = items; slash.i = Math.min(slash.i, items.length - 1); renderSlash();
  }
  function renderSlash() {
    const m = $("dslash");
    m.innerHTML = slash.items.map(([l, , h], i) => `<div class="ds-i${i === slash.i ? " act" : ""}" data-i="${i}"><span>${esc(l)}</span><small>${esc(h)}</small></div>`).join("");
    const rc = caretRect(); if (rc) place(m, rc, false);
    m.querySelector(".act")?.scrollIntoView({ block: "nearest" });
  }
  function closeSlash() { slash = null; $("dslash").style.display = "none"; }
  function slashKey(e) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { slash.i = (slash.i + (e.key === "ArrowDown" ? 1 : -1) + slash.items.length) % slash.items.length; renderSlash(); }
    else if (e.key === "Enter" || e.key === "Tab") choose(slash.items[slash.i][1]);
    else if (e.key === "Escape") closeSlash();
    else return false;
    e.preventDefault(); e.stopPropagation(); return true;
  }
  function choose(cmd) {
    const q = slashQuery() ?? "", sl = slash; closeSlash();
    const b = sl.b, end = Math.min(b.textContent.length, sl.start + 1 + q.length);
    checkpoint();
    const r = document.createRange(); r.setStart(...locate(b, sl.start)); r.setEnd(...locate(b, Math.max(sl.start + 1, end))); r.deleteContents();
    b.normalize(); fill(b);
    const blank = empty(b);
    const textBlock = (tag, k) => {
      let n;
      if (blank) n = kind(b) === "li" && tag === "li" ? b : convert(b, tag, k);
      else { n = tag === "li" ? mk("div", "li") : mk(tag, null, "<br>"); if (tag === "li") { n.dataset.k = k; n.dataset.in = 0; n.append(mk("br")); } b.after(n); }
      if (tag === "li") n.dataset.k = k;
      caretIn(n, blank ? 0 : 0); return n;
    };
    switch (cmd) {
      case "p": case "h1": case "h2": case "h3": case "blockquote": textBlock(cmd); break;
      case "ul": case "ol": case "todo": textBlock("li", cmd); break;
      case "code": { const pre = mk("pre"); pre.dataset.lang = ""; pre.textContent = "\n"; if (blank) b.replaceWith(pre); else b.after(pre); if (!pre.nextElementSibling) pre.after(newP()); caretIn(pre); break; }
      case "hr": { const hr = placeAtomic(b, fromHtml(`<hr contenteditable="false">`)); caretIn(hr.nextElementSibling); break; }
      case "table": { const t = placeAtomic(b, fromHtml(tableHtml([["", ""], ["", ""], ["", ""]]))); caretIn(t.querySelector("th")); break; }
      case "math": changed(); return editAtomic(placeAtomic(b, fromHtml(mathHtml(""))));
      case "chart": changed(); return editAtomic(placeAtomic(b, fromHtml(chartHtml(CHART_SAMPLE))));
      case "image": { const id = ++imgId; pending[id] = { at: b, blank }; send("docPickImage", { path: lect, doc, id }); break; }
      case "time": changed(); return timePop();
    }
    changed();
  }
  const CHART_SAMPLE = JSON.stringify({ type: "bars", caption: "Title", items: [{ label: "A", value: 3 }, { label: "B", value: 5 }] }, null, 2);

  let popDone = null;
  // A small box under `rc` to edit a value: formula, chart JSON, link or time.
  function openPop(rc, o) {
    closePop(true);
    const p = $("dpop");
    p.innerHTML = `<div class="dp-l"></div>${o.area ? `<textarea class="dp-in field-in" spellcheck="false"></textarea>` : `<input class="dp-in field-in" spellcheck="false">`}<div class="dp-prev"></div>` +
      `<div class="dp-row">${(o.extra || []).map(([l], i) => `<button data-x="${i}">${esc(l)}</button>`).join("")}${o.onDelete ? `<button data-x="del">${esc(o.delLabel || "Delete")}</button>` : ""}<span class="spacer"></span><button data-x="no">Cancel</button><button data-x="ok" class="go">Done</button></div>`;
    p.querySelector(".dp-l").textContent = o.label;
    const inp = p.querySelector(".dp-in"); inp.value = o.value || ""; if (o.placeholder) inp.placeholder = o.placeholder;
    if (o.mono) inp.classList.add("mono");
    const prev = () => { const out = p.querySelector(".dp-prev"); const h = o.preview ? o.preview(inp.value) : ""; out.innerHTML = h || ""; out.style.display = h ? "" : "none"; };
    inp.oninput = prev; prev();
    const finish = (ok) => { if (popDone !== finish) return; popDone = null; p.style.display = "none"; if (ok) o.onDone(inp.value); else o.onCancel?.(); };
    popDone = finish;
    inp.onkeydown = (e) => {
      if (e.key === "Escape") { e.preventDefault(); finish(false); }
      else if (e.key === "Enter" && (!o.area || e.metaKey)) { e.preventDefault(); finish(true); }
      e.stopPropagation();
    };
    p.onclick = (e) => {
      const x = e.target.closest("[data-x]")?.dataset.x; if (x == null) return;
      if (x === "ok") finish(true); else if (x === "no") finish(false);
      else if (x === "del") { popDone = null; p.style.display = "none"; o.onDelete(); }
      else o.extra[+x][1](inp.value, () => finish(false));
    };
    $("dtool").style.display = "none";
    place(p, rc, false);
    inp.focus(); if (!o.area) inp.select();
  }
  function closePop(ok) { if (popDone) popDone(ok); }
  function editAtomic(el) {
    const math = el.classList.contains("mathb");
    unselect();
    openPop(el.getBoundingClientRect(), math
      ? { label: "Formula (LaTeX)", value: el.dataset.tex, area: true, mono: true, placeholder: "\\frac{1}{n} \\sum_i x_i", preview: (v) => v.trim() && texHtml(v, true),
          onDone: (v) => { checkpoint(); const n = fromHtml(mathHtml(v.trim())); el.replaceWith(n); after(n); }, onDelete: () => removeAtomic(el) }
      : { label: "Chart: one diagram block as JSON, in the slides' format (bars, plot, graph, seq, net, sketch, timeline, arch, stack, header)", value: el.dataset.src, area: true, mono: true,
          preview: (v) => { const r = chartInner(v); return r.err ? `<p class="dp-err">${esc(r.err)}</p>` : `<div class="dp-chart">${r.html}</div>`; },
          onDone: (v) => { checkpoint(); const n = fromHtml(chartHtml(v.trim())); el.replaceWith(n); after(n); requestAnimationFrame(fitCharts); }, onDelete: () => removeAtomic(el) });
    if (!math) requestAnimationFrame(() => { const c = $("dpop").querySelector(".dp-chart"); fitPreview(c); $("dpop").querySelector(".dp-in").addEventListener("input", () => requestAnimationFrame(() => fitPreview($("dpop").querySelector(".dp-chart")))); });
    function after(n) { const nx = n.nextElementSibling; if (nx && isText(nx)) caretIn(nx, 0, false); changed(); }
  }
  function fitPreview(c) {
    if (!c) return;
    const d = c.querySelector(".dchart"), w = c.querySelector(".cw"); if (!d || !w) return;
    const k = Math.min(1, c.clientWidth / d.offsetWidth); d.style.transform = `scale(${k})`; w.style.height = Math.ceil(d.offsetHeight * k) + "px";
  }
  function editInlineMath(el) {
    const p = el.parentNode, i = idx(el);
    openPop(el.getBoundingClientRect(), { label: "Formula (LaTeX)", value: el.dataset.tex, mono: true, preview: (v) => v.trim() && texHtml(v, false),
      onDone: (v) => { checkpoint(); if (!v.trim()) el.remove(); else el.replaceWith(fromHtml(imath(v.trim()))); B().focus(); getSelection().setBaseAndExtent(p, Math.min(i + 1, p.childNodes.length), p, Math.min(i + 1, p.childNodes.length)); changed(); },
      onDelete: () => { checkpoint(); el.remove(); changed(); } });
  }
  function linkPop(a) {
    const saved = caretPos(), s = getSelection();
    a = a || (s.rangeCount && (s.anchorNode.nodeType === 1 ? s.anchorNode : s.anchorNode.parentElement)?.closest?.("#docBody a:not(.tl)"));
    const rc = a ? a.getBoundingClientRect() : caretRect(); if (!rc) return;
    openPop(rc, { label: "Link", value: a ? a.getAttribute("href") : "", placeholder: "https://…",
      extra: a ? [["Open", (v, close) => { close(); send("open", { path: v }); }]] : [],
      onDelete: a ? () => { checkpoint(); a.replaceWith(...a.childNodes); changed(); } : null, delLabel: "Remove link",
      onDone: (v) => {
        v = v.trim(); if (v && !/^[a-z]+:|^#/i.test(v)) v = "https://" + v;
        setCaret(saved); checkpoint();
        if (a) { if (v) a.setAttribute("href", v); else a.replaceWith(...a.childNodes); }
        else if (v) { if (getSelection().isCollapsed) { const n = mk("a"); n.href = v; n.setAttribute("href", v); n.textContent = v; getSelection().getRangeAt(0).insertNode(n); } else document.execCommand("createLink", false, v); }
        changed();
      } });
  }
  function timePop() {
    const saved = caretPos();
    const live = typeof state !== "undefined" && state !== "idle" && segs.length ? fmt(segs[segs.length - 1].t) : "";
    openPop(caretRect() || B().getBoundingClientRect(), { label: "Time in the lecture (mm:ss or h:mm:ss)", value: live.replace(/^00:/, ""), placeholder: "12:40",
      preview: (v) => (v.trim() && !hms(v) ? `<p class="dp-err">Write it as 12:40 or 1:02:30</p>` : ""),
      onDone: (v) => {
        const t = hms(v); if (!t) return;
        setCaret(saved); checkpoint();
        const a = fromHtml(link(short(t), "#t=" + t)), r = getSelection().getRangeAt(0);
        r.insertNode(document.createTextNode(" ")); r.insertNode(a);
        const p = a.parentNode, i = idx(a) + 2; getSelection().setBaseAndExtent(p, i, p, i);
        changed();
      } });
  }
  function hms(v) {
    const m = /^\s*(?:(\d+):)?(\d{1,2}):(\d\d)\s*$/.exec(v); if (!m || +m[3] > 59) return null;
    return [m[1] || 0, m[2], m[3]].map((x) => String(+x).padStart(2, "0")).join(":");
  }
  const short = (t) => t.replace(/^00:/, "").replace(/^0(\d:)/, "$1");

  /* ---------- images ---------- */
  const pending = {};
  function insertImageFile(file, at) {
    if (!doc) return;
    const id = ++imgId, rd = new FileReader();
    rd.onload = () => {
      checkpoint();
      const s = getSelection(), b = at || (s.rangeCount ? blockOf(s.anchorNode) : null);
      const fig = placeAtomic(b && b !== B() && b.parentNode === B() ? b : B().lastElementChild, fromHtml(figHtml("", "")));
      fig.querySelector("img").src = rd.result; fig.classList.add("up"); fig.dataset.id = id;
      changed();
      send("docImage", { path: lect, doc, id, data: rd.result, ext: (file.type.split("/")[1] || "png").replace("jpeg", "jpg") });
    };
    rd.readAsDataURL(file);
  }
  function imageSaved(e) {
    const fig = B().querySelector(`figure[data-id="${e.id}"]`);
    if (fig) { images[e.src] = e.url; fig.dataset.src = e.src; fig.removeAttribute("data-id"); fig.classList.remove("up"); if (!fig.classList.length) fig.removeAttribute("class"); return changed(true); }
    const p = pending[e.id]; delete pending[e.id];
    if (!p || e.cancelled || !e.src || e.doc !== doc) return;
    checkpoint();
    images[e.src] = e.url;
    const at = p.at && p.at.parentNode === B() ? p.at : B().lastElementChild;
    placeAtomic(at, fromHtml(figHtml(e.src, "")));
    changed();
  }

  /* ---------- events ---------- */
  function bind() {
    const body = B();
    body.addEventListener("keydown", (e) => {
      if (e.isComposing) return;
      if (slash && slashKey(e)) return;
      const meta = e.metaKey || e.ctrlKey, key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      if (meta && key === "z") { e.preventDefault(); return e.shiftKey ? doRedo() : doUndo(); }
      if (meta && !e.shiftKey && !e.altKey && { b: 1, i: 1, k: 1, e: 1 }[key]) { e.preventDefault(); return format({ b: "bold", i: "italic", k: "link", e: "code" }[key]); }
      if (meta && e.shiftKey && key === "x") { e.preventDefault(); return format("strike"); }
      if (selAtomic) {
        if (e.key === "Backspace" || e.key === "Delete") { e.preventDefault(); return removeAtomic(selAtomic); }
        if (e.key === "Enter") { e.preventDefault(); const a = selAtomic; unselect(); if (a.matches(".mathb,.chartb")) return editAtomic(a); const nx = a.nextElementSibling; if (nx) caretIn(nx, 0); return; }
        if (e.key === "ArrowDown" || e.key === "ArrowRight") { e.preventDefault(); const nx = selAtomic.nextElementSibling; unselect(); if (nx) caretIn(nx, 0); return; }
        if (e.key === "ArrowUp" || e.key === "ArrowLeft") { e.preventDefault(); const pv = selAtomic.previousElementSibling; unselect(); if (pv && isText(pv)) caretIn(pv, pv.textContent.length); return; }
        if (!meta) { e.preventDefault(); return; }
      }
      const s = getSelection(); if (!s.rangeCount) return;
      const b = blockOf(s.anchorNode); if (!b || b === B()) return;
      const h = hostOf(s.anchorNode);
      if (e.key === "Tab") return tab(e, b);
      if (h) { // a table cell or caption: no blocks inside
        if (e.key === "Enter") {
          e.preventDefault();
          if (h.matches("td,th")) { const row = h.parentNode, below = row.parentNode.tagName === "THEAD" ? b.querySelector("tbody tr") : row.nextElementSibling; const ci = [...row.children].indexOf(h); if (below) caretIn(below.children[ci], 0); else { const nx = b.nextElementSibling; if (nx && isText(nx)) caretIn(nx, 0); } }
          else { const nx = b.nextElementSibling; if (nx && isText(nx)) caretIn(nx, 0); }
        } else if (e.key === "Backspace" && s.isCollapsed && caretOffset(h) === 0) e.preventDefault();
        return;
      }
      if (e.key === "Enter") return enter(e, b);
      if (e.key === "Backspace" && s.isCollapsed && caretOffset(b) === 0 && s.focusNode !== B()) return backspace(e, b);
      if (e.key === "ArrowDown" && kind(b) === "pre" && caretOffset(b) >= b.textContent.replace(/\n$/, "").length && !b.nextElementSibling) { e.preventDefault(); const p = newP(); b.after(p); caretIn(p); }
    });
    body.addEventListener("beforeinput", (e) => {
      if (e.inputType === "historyUndo" || e.inputType === "historyRedo") { e.preventDefault(); return e.inputType === "historyUndo" ? doUndo() : doRedo(); }
      if (e.inputType === "insertParagraph") { e.preventDefault(); const b = blockOf(getSelection().anchorNode); if (b && b !== B() && !hostOf(getSelection().anchorNode)) enter({ preventDefault() {}, shiftKey: false, metaKey: false }, b); return; }
      if (e.inputType === "formatBold" || e.inputType === "formatItalic") checkpoint();
      if (selAtomic) unselect();
      checkpoint(e.inputType === "insertText" ? "type" : e.inputType.startsWith("delete") ? "del" : "x");
    });
    body.addEventListener("input", (e) => {
      if (e.inputType === "insertText" && e.data === " " && shortcut()) return changed();
      if (e.inputType === "insertText" && e.data === "-") {
        const b = blockOf(getSelection().anchorNode);
        if (b && kind(b) === "p" && b.textContent.replace(/\u00a0/g, " ").trim() === "---") { checkpoint(); const hr = fromHtml(`<hr contenteditable="false">`); b.replaceWith(hr); if (!hr.nextElementSibling || !isText(hr.nextElementSibling)) hr.after(newP()); caretIn(hr.nextElementSibling); return changed(); }
      }
      if (e.inputType === "insertText" && e.data === "/") openSlash();
      else if (slash) updateSlash();
      changed();
    });
    body.addEventListener("paste", (e) => {
      const dt = e.clipboardData; if (!dt || !doc) return;
      e.preventDefault();
      const s = getSelection(), file = [...dt.items].find((i) => i.kind === "file" && i.type.startsWith("image/"))?.getAsFile();
      const h = s.rangeCount && hostOf(s.anchorNode), b = s.rangeCount && blockOf(s.anchorNode);
      if (file && !h) return insertImageFile(file);
      const text = dt.getData("text/plain"); if (!text) return;
      checkpoint();
      if (h || !b || b === B() || kind(b) === "pre" || (!/\n/.test(text.trim()) && !startsBlock(text.trim(), ""))) {
        document.execCommand("insertText", false, h ? text.replace(/\s*\n\s*/g, " ") : kind(b) === "pre" ? text : text.replace(/\s*\n\s*/g, " "));
        return changed();
      }
      pasteBlocks(b, text);
    });
    body.addEventListener("dragover", (e) => { if ([...(e.dataTransfer?.items || [])].some((i) => i.kind === "file")) e.preventDefault(); });
    body.addEventListener("drop", (e) => {
      const files = [...(e.dataTransfer?.files || [])].filter((f) => f.type.startsWith("image/")); if (!files.length || !doc) return;
      e.preventDefault();
      const r = document.caretRangeFromPoint(e.clientX, e.clientY), b = r && blockOf(r.startContainer);
      for (const f of files) insertImageFile(f, b && b !== B() ? b : null);
    });
    body.addEventListener("mousedown", (e) => {
      const li = e.target.closest?.(".li[data-k=todo]");
      if (li && e.target === li) {
        const r = li.getBoundingClientRect(), x0 = r.left + (+li.dataset.in || 0) * 24;
        if (e.clientX < x0 + 24) { e.preventDefault(); checkpoint(); li.toggleAttribute("data-done"); return changed(); }
      }
      if (e.target.closest?.(".tl, .im, .mathb, .chartb, hr, figure img, .tbl-tools")) e.preventDefault();
    });
    body.addEventListener("click", (e) => {
      const t = e.target;
      const tl = t.closest(".tl"); if (tl) { e.preventDefault(); return goToTime(tl.getAttribute("href").slice(3)); }
      const a = t.closest("a"); if (a) { e.preventDefault(); return linkPop(a); }
      const im = t.closest(".im"); if (im) return editInlineMath(im);
      const at = t.closest(".mathb, .chartb"); if (at) return editAtomic(at);
      if (t.tagName === "HR" || t.matches("figure img")) return selectAtomic(t.closest("hr, figure"));
      const tb = t.closest(".tbl-tools button");
      if (tb) {
        const tbl = tb.closest(".tbl"); checkpoint();
        if (tb.dataset.t === "del") return removeAtomic(tbl);
        tableOp(tbl, tb.dataset.t, lastCell && tbl.contains(lastCell) && lastCell.isConnected ? lastCell : null);
        return changed();
      }
      if (selAtomic && !t.closest(".sel")) unselect();
    });
    body.addEventListener("focusin", (e) => { if (e.target.matches?.("td,th")) lastCell = e.target; });
    body.addEventListener("focusout", () => setTimeout(() => { if (!B().contains(document.activeElement)) { saveNow(); closeSlash(); } }, 0));
    document.addEventListener("selectionchange", () => {
      if (!doc) return;
      toolbar();
      if (slash) { const q = slashQuery(); if (q == null) closeSlash(); }
      placeholders();
    });
    for (const id of ["dtool", "dslash"]) $(id).addEventListener("mousedown", (e) => e.preventDefault());
    $("dtool").addEventListener("click", (e) => { const x = e.target.closest("[data-f]")?.dataset.f; if (x) format(x); });
    $("dslash").addEventListener("click", (e) => { const d = e.target.closest(".ds-i"); if (d && slash) choose(slash.items[+d.dataset.i][1]); });
    addEventListener("mousedown", (e) => { if (popDone && !$("dpop").contains(e.target)) closePop(true); }, true);
    $("docScroll").addEventListener("scroll", () => { $("dtool").style.display = "none"; if (slash) renderSlash(); });
    new ResizeObserver(() => fitCharts()).observe(body);
    // A click below the last block puts the caret in it.
    $("docScroll").addEventListener("mousedown", (e) => {
      if (!doc || e.target.closest("#docBody, #docTitle")) return;
      const last = B().lastElementChild, r = last?.getBoundingClientRect();
      if (last && r && e.clientY > r.bottom) { e.preventDefault(); caretIn(last, last.textContent.length); }
    });

    $("docTitle").addEventListener("keydown", (e) => { if (e.key === "Enter" || (e.key === "ArrowDown")) { e.preventDefault(); const f = B().firstElementChild; if (f && isText(f)) caretIn(f, 0); else B().focus(); } });
    $("docTitle").addEventListener("change", () => rename($("docTitle").value));
    $("docSel").onchange = () => { const p = $("docSel").value; if (p && p !== doc) { saveNow(); open(p); } };
    $("docNew").onclick = () => newDoc();
    $("docDel").onclick = async () => {
      if (!doc) return;
      if (!await ask(`Move “${name}” to the Trash? You can put it back from the Trash in Finder.`, "Move to Trash")) return;
      const d = doc; clearTimeout(saveTimer); saved = serialize(); doc = null;
      send("docDelete", { path: lect, doc: d });
    };
    $("draftBtn").onclick = () => draft();
    $("docsView").addEventListener("click", (e) => {
      if (e.target.id === "dcDraft") draft();
      if (e.target.id === "dcBlank") newDoc();
    });
    $("docHint").addEventListener("keydown", (e) => { if (e.key === "Enter") { e.preventDefault(); draft(); } });
  }

  function pasteBlocks(b, text) {
    const s = getSelection();
    if (!s.isCollapsed) document.execCommand("delete");
    const r = s.getRangeAt(0), tail = document.createRange();
    tail.setStart(r.startContainer, r.startOffset); tail.setEnd(b, b.childNodes.length);
    const frag = tail.extractContents();
    const nodes = [...mk("div", null, parse(text)).children];
    let last = b;
    if (nodes[0] && kind(nodes[0]) === "p" && kind(b) !== "pre") { const first = nodes.shift(); trimBr(b); b.append(...first.childNodes); }
    for (const n of nodes) { last.after(n); last = n; }
    let end = isText(last) && kind(last) !== "pre" ? last : null;
    const o = end ? (trimBr(end), end.textContent.length) : 0;
    if (frag.textContent || frag.querySelector?.(".im,.tl")) {
      if (!end) { end = mk("p"); last.after(end); }
      trimBr(end); end.append(frag);
    }
    if (empty(b) && b !== last && nodes.length) b.remove(); else fill(b);
    if (end) { fill(end); caretIn(end, o); }
    changed(); requestAnimationFrame(fitCharts);
  }

  /* ---------- docs of the lecture ---------- */
  function newDoc() { if (!lect) return; saveNow(); focusNext = true; send("docCreate", { path: lect, name: "Untitled", md: "" }); }
  function open(p) { opening = p; send("docRead", { path: lect, doc: p }); }
  function rename(v) {
    v = v.trim();
    if (!doc || !v || v === name) { $("docTitle").value = name; return; }
    saveNow(); renaming = doc;
    send("docRename", { path: lect, doc, name: v });
  }
  function load(e) {
    doc = e.doc; name = e.name; images = e.images || {}; lastOpen[lect] = doc; opening = null;
    B().innerHTML = parse(e.md);
    normalize(); renumber(); undo = []; redo = []; unselect(); closeSlash();
    saved = serialize();
    $("docTitle").value = name === "Untitled" ? "" : name;
    render(); placeholders();
    status("Saved");
    requestAnimationFrame(fitCharts);
    if (focusNext) { focusNext = false; $("docTitle").focus(); }
  }
  function draft() {
    if (!lect || drafting || $("draftBtn").disabled) return;
    drafting = true; draftErr = "";
    send("docDraft", { path: lect, course: $("course").value, model: $("aiModel").value, effort: $("effort").value, lang: slideLangCode(), hint: $("docHint").value.trim() });
    render();
  }
  function lecture(p) {
    if (p === lect) return;
    saveNow();
    lect = p; list = []; doc = null; name = ""; draftErr = ""; undo = []; redo = []; B().innerHTML = "";
    render();
    if (p) send("docsList", { path: p });
  }
  function on(e) {
    if (e.type === "docDrafted" || e.type === "docDraftError") {
      drafting = false;
      if (e.path !== lect) { if (e.type === "docDrafted") toast("Notes are ready in the lecture's Docs folder"); render(); return; }
      if (e.type === "docDraftError") { draftErr = e.message; toast(e.message, true); return render(); }
      $("docHint").value = "";
      list = e.docs;
      const nm = e.doc.split("/").pop().replace(/\.md$/, "");
      const busy = doc && B().contains(document.activeElement);
      if (!busy) { saveNow(); open(e.doc); }
      toast(busy ? `Notes ready: “${nm}”. Open them from the list` : `Notes ready: “${nm}”`);
      return render();
    }
    if (e.path !== lect) return;
    switch (e.type) {
      case "docs": {
        list = e.docs || [];
        if (renaming && e.open && renaming === doc) { doc = e.open; name = list.find((d) => d.path === e.open)?.name || name; lastOpen[lect] = doc; renaming = null; $("docTitle").value = name; break; }
        renaming = null;
        const has = (p) => p && list.some((d) => d.path === p);
        const want = e.open || (has(doc) ? doc : null) || (has(lastOpen[lect]) ? lastOpen[lect] : null) || list[list.length - 1]?.path || null;
        if (!want) { doc = null; B().innerHTML = ""; }
        else if (want !== doc && want !== opening) open(want);
        break;
      }
      case "doc": if (e.doc === opening) load(e); break;
      case "docSaved": if (e.doc === doc && e.seq === seq) status(serialize() === saved ? "Saved" : "Edited"); break;
      case "docImage": imageSaved(e); break;
      case "docError": toast(e.message, true); status("Not saved"); break;
    }
    render();
  }
  function render() {
    const sel = $("docSel"), key = JSON.stringify(list);
    if (sel.dataset.key !== key) { sel.dataset.key = key; sel.innerHTML = ""; for (const d of list) sel.add(new Option(d.name, d.path)); }
    if (sel.value !== (doc || "")) sel.value = doc || "";
    $("docHead").style.display = lect && list.length ? "" : "none";
    $("docDel").disabled = !doc;
    $("docPage").style.display = doc ? "" : "none";
    B().contentEditable = doc ? "true" : "false";
    const em = $("docEmpty");
    em.style.display = doc || (lect && list.length) ? "none" : "";
    if (!lect) em.innerHTML = "Docs about the lecture you're recording or have open go here: your notes, summaries, exercises.";
    else if (!list.length) em.innerHTML = `No docs for this lecture yet.<div class="dc-start"><button id="dcDraft" class="go">Draft notes with Claude</button><button id="dcBlank">Blank doc</button></div><span class="dc-sub">${esc(engName())} writes study notes of the whole lecture into a new doc: sections, time links, tables, formulas and diagrams, only from what was said. Add a suggestion below if you like.</span>`;
    bar();
    $("docNew").disabled = !lect;
    const st = $("docStatus");
    st.innerHTML = drafting ? `<span class="new">${esc(engName())} is writing notes for the whole lecture… (a few minutes)</span>` : draftErr ? `<span class="err"></span>` : "";
    if (!drafting && draftErr) st.firstChild.textContent = draftErr;
    $("docProg").classList.toggle("on", drafting);
    if (!doc) $("docSavedTxt").textContent = "";
  }

  // The Draft button needs lines to work from: updated on every page render.
  function bar() {
    const noText = !segs.length;
    if ($("dcDraft")) $("dcDraft").disabled = drafting || noText;
    $("draftBtn").disabled = !lect || drafting || noText;
  }

  bind();
  return { bar, lecture, on, render, saveNow, fitCharts, parse, serialize, get doc() { return doc; }, get drafting() { return drafting; } };
})();
