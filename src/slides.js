// Renders the DeepSeek deck (JSON) as 1280×720 slides with the Protocolli & Reti classes.
// Used by the app's slide panel and by the exported "· Slide.html" deck. All text is escaped.
(function () {
  // Diagram labels are plain text: drop markdown backticks before escaping.
  const plain = (s) => String(s ?? "").replace(/`/g, "");
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  const inline = (s) =>
    esc(s)
      .replace(/`([^`]+)`/g, '<code class="ic">$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\(\?\)/g, '<span class="unsure" title="Correzione incerta della trascrizione">(?)</span>');
  const two = (n) => String(n).padStart(2, "0");

  function cell(c) {
    if (c && typeof c === "object") {
      if ("yes" in c) return `<span class="pr-yes">${inline(c.yes)}</span>`;
      if ("no" in c) return `<span class="pr-no">${inline(c.no)}</span>`;
      return inline(c.text);
    }
    return inline(c);
  }

  const unsure = (h) => h.replace(/\(\?\)/g, '<span class="unsure" title="Non trovato nella trascrizione">(?)</span>');
  function termLine(l) { return unsure(termLine0(l)); }
  function termLine0(l) {
    const s = String(l);
    if (s.startsWith("$ ")) return `<span class="c">$</span> <span class="k">${esc(s.slice(2))}</span>`;
    const m = s.match(/^(\S+[#>]) (.*)$/);
    if (m && s.length < 160) return `<span class="c">${esc(m[1])}</span> <span class="k">${esc(m[2])}</span>`;
    if (s.startsWith("# ") || s.startsWith("! ")) return `<span class="c">${esc(s)}</span>`;
    return esc(s);
  }

  const B = {
    p: (b) => `<p class="body-lg s-p">${inline(b.text)}</p>`,
    list: (b) =>
      `<ol class="pr-list body s-list">${(b.items || []).slice(0, 6).map((it, i) => `<li><span class="n">${two(i + 1)}</span><span>${inline(it)}</span></li>`).join("")}</ol>`,
    def: (b) =>
      `<div class="pr-def s-def"><span class="tag">${esc(b.tag || "Definizione")}</span>` +
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
    quote: (b) => `<p class="quote s-quote">${inline(b.text)}</p>`,
    notice: (b) => `<div class="pr-def s-def s-notice"><span class="tag">Avviso</span><p class="s-noticetext">${inline(b.text)}</p></div>`,
    check: (b) => `<div class="pr-def s-def s-check"><span class="tag">Da verificare</span><p class="s-checktext">${inline(b.text)}</p></div>`,
  };
  const blocks = (arr) => (arr || []).map((b) => (B[b && b.type] || B.p)(b || {})).join("");

  // One slide. i is 1-based; foot = text on the left of the footer.
  function slide(s, i, n, foot) {
    const end = `<div class="pr-foot"><span>${esc(foot)}</span><span>${two(i)} / ${two(n)}</span></div><div class="pr-progress" style="width:${((i / n) * 100).toFixed(1)}%"></div>`;
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

  // Follow the macOS light/dark setting (the design system needs data-theme on <html>).
  const mq = matchMedia("(prefers-color-scheme: dark)");
  const apply = () => document.documentElement.setAttribute("data-theme", mq.matches ? "dark" : "light");
  apply();
  mq.addEventListener("change", apply);
})();
