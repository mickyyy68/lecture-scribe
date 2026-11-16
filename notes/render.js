// Renders note sections (JSON from DeepSeek) with the Protocolli & Reti classes.
// Shared by the live page and the exported .html file. All text is escaped.
(function () {
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  const inline = (s) =>
    esc(s)
      .replace(/`([^`]+)`/g, '<code class="ic">$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\(\?\)/g, '<span class="unsure" title="Correzione incerta della trascrizione">(?)</span>');
  const two = (n) => String(n + 1).padStart(2, "0");

  function cell(c) {
    if (c && typeof c === "object") {
      if ("yes" in c) return `<span class="pr-yes">${inline(c.yes)}</span>`;
      if ("no" in c) return `<span class="pr-no">${inline(c.no)}</span>`;
      return inline(c.text);
    }
    return inline(c);
  }

  function termLine(l) {
    const s = String(l);
    if (s.startsWith("$ ")) return `<span class="c">$</span> <span class="k">${esc(s.slice(2))}</span>`;
    if (/^\S+[#>] /.test(s) && s.length < 200) {
      const i = s.search(/[#>] /) + 1;
      return `<span class="c">${esc(s.slice(0, i))}</span> <span class="k">${esc(s.slice(i + 1))}</span>`;
    }
    if (s.startsWith("# ") || s.startsWith("! ")) return `<span class="c">${esc(s)}</span>`;
    return esc(s);
  }

  const R = {
    p: (b) => `<p class="n-p">${inline(b.text)}</p>`,
    list: (b) =>
      `<ol class="pr-list n-list">${(b.items || []).map((it, i) => `<li><span class="n">${two(i)}</span><span>${inline(it)}</span></li>`).join("")}</ol>`,
    def: (b) =>
      `<div class="pr-def n-def"><span class="tag">${esc(b.tag || "Definizione")}</span>` +
      `<p class="term">${inline(b.term)}</p><p class="text">${inline(b.text)}</p>` +
      (b.src ? `<p class="src">${esc(b.src)}</p>` : "") +
      `</div>`,
    table: (b) =>
      `<div class="n-scroll"><table class="pr-table n-table"><thead><tr>${(b.head || []).map((h) => `<th>${esc(h)}</th>`).join("")}</tr></thead><tbody>` +
      (b.rows || []).map((r, i) => `<tr${b.focus === i ? ' class="on"' : ""}>${(r || []).map((c) => `<td>${cell(c)}</td>`).join("")}</tr>`).join("") +
      `</tbody></table></div>`,
    term: (b) => {
      const hl = new Set(b.hl || []);
      const lines = (b.lines || []).map((l, i) => (hl.has(i) ? `<span class="hl">${termLine(l)}</span>` : termLine(l)));
      return `<div class="pr-term n-term"><div class="bar"><span>${esc(b.host || "terminale")}</span><span>${esc(b.tool || "")}</span></div><pre>${lines.join("\n")}</pre></div>`;
    },
    header: (b) => {
      const bits = `<div class="pr-bits"><span style="grid-column:1">0</span><span style="grid-column:9">8</span><span style="grid-column:17">16</span><span style="grid-column:25">24</span><span style="grid-column:32">31</span></div>`;
      const rows = (b.rows || [])
        .map(
          (r) =>
            `<div class="pr-row">${(r || [])
              .map((f) => {
                const span = Math.max(1, Math.min(32, f.opt ? 32 : Number(f.bits) || 8));
                const cls = "pr-field" + (f.on ? " on" : "") + (f.opt ? " opt" : "");
                return `<div class="${cls}" style="grid-column:span ${span}">${esc(f.name)}${f.bits && !f.opt ? ` <small>${esc(f.bits)}</small>` : ""}</div>`;
              })
              .join("")}</div>`
        )
        .join("");
      return `<div class="pr-header n-header">${b.caption ? `<p class="n-cap">${esc(b.caption)}</p>` : ""}${bits}${rows}</div>`;
    },
    stack: (b) => {
      const focus = b.focus != null;
      const lv = ["pr-l5", "pr-l4", "pr-l3", "pr-l2", "pr-l1"];
      return `<div class="pr-stack n-stack${focus ? " is-focus" : ""}">${(b.layers || [])
        .map((l, i) => `<div class="pr-layer ${lv[Math.min(i, 4)]}${focus && b.focus === i ? " on" : ""}"><span class="k">${esc(l.k || "")}</span><span>${esc(l.name)}</span><span class="p">${esc(l.p || "")}</span></div>`)
        .join("")}</div>`;
    },
    seq: (b) => {
      const hosts = (b.hosts || []).slice(0, 3);
      const msgs = (b.msgs || []).slice(0, 10);
      if (hosts.length < 2) return "";
      const W = 760, M = 90, top = 44, row = 66, slant = 26;
      const x = (i) => M + (Math.max(0, Math.min(hosts.length - 1, Number(i) || 0)) * (W - 2 * M)) / (hosts.length - 1);
      const H = top + 30 + msgs.length * row;
      let g = "";
      hosts.forEach((h, i) => {
        g += `<line class="life" x1="${x(i)}" y1="${top}" x2="${x(i)}" y2="${H}"/>`;
        g += `<rect class="node" x="${x(i) - 70}" y="4" width="140" height="${top - 8}"/><text class="host" x="${x(i)}" y="${top / 2 + 6}" text-anchor="middle">${esc(h)}</text>`;
      });
      msgs.forEach((m, k) => {
        const x0 = x(m.from), x1 = x(m.to), y0 = top + 30 + k * row, y1 = y0 + slant;
        const dir = Math.sign(x1 - x0) || 1;
        const lost = !!m.lost;
        const ex = lost ? x0 + (x1 - x0) * 0.62 : x1 - dir * 2, ey = lost ? y0 + slant * 0.62 : y1;
        const mx = (x0 + x1) / 2, my = (y0 + y1) / 2;
        g += `<circle class="dot" cx="${x0}" cy="${y0}" r="4"/>`;
        g += `<line class="msg${lost ? " lost" : ""}" x1="${x0}" y1="${y0}" x2="${ex}" y2="${ey}"/>`;
        if (lost) {
          g += `<path class="msg lost" style="stroke-dasharray:none" d="M${ex - 7} ${ey - 7}L${ex + 7} ${ey + 7}M${ex + 7} ${ey - 7}L${ex - 7} ${ey + 7}"/>`;
        } else {
          const a = Math.atan2(y1 - y0, x1 - x0), L = 11, w = 5;
          const bx = x1 - Math.cos(a) * L, by = y1 - Math.sin(a) * L;
          g += `<path class="head" d="M${x1} ${y1}L${bx - Math.sin(a) * w} ${by + Math.cos(a) * w}L${bx + Math.sin(a) * w} ${by - Math.cos(a) * w}Z"/>`;
        }
        g += `<text class="lbl" x="${mx}" y="${my - 9}" text-anchor="middle">${esc(m.label)}</text>`;
        const meta = [m.meta, lost ? "perso" : ""].filter(Boolean).join(" · ");
        if (meta || m.note)
          g += `<text class="meta" x="${mx}" y="${my + 22}" text-anchor="middle">${esc(meta)}${m.note ? `<tspan class="note" dx="${meta ? 8 : 0}">${esc(m.note)}</tspan>` : ""}</text>`;
      });
      return `<div class="n-seq">${b.caption ? `<p class="n-cap">${esc(b.caption)}</p>` : ""}<svg class="pr-seq" viewBox="0 0 ${W} ${H + 8}" width="100%" role="img">${g}</svg></div>`;
    },
    net: (b) => {
      const nodes = (b.nodes || []).slice(0, 6), links = b.links || [];
      const parts = nodes.map((n, i) =>
        `<div class="n-node"><b>${esc(n.name)}</b>${n.sub ? `<small>${esc(n.sub)}</small>` : ""}</div>` +
        (i < nodes.length - 1 ? `<div class="n-link"><span>${esc(links[i] || "")}</span></div>` : ""));
      return `<div class="n-netw">${b.caption ? `<p class="n-cap">${esc(b.caption)}</p>` : ""}<div class="n-net">${parts.join("")}</div></div>`;
    },
    quote: (b) => `<blockquote class="n-quote">${inline(b.text)}</blockquote>`,
    check: (b) => `<div class="pr-def n-def n-check"><span class="tag">Da verificare</span><p class="n-p">${inline(b.text)}</p></div>`,
  };

  function section(s, i) {
    const range = [s.from, s.to].filter(Boolean).join(" – ");
    const blocks = (s.blocks || []).map((b) => (R[b.type] || R.p)(b)).join("");
    return (
      `<section class="n-sec" id="s${i + 1}"><p class="pr-eyebrow eyebrow n-eyebrow">${esc(range)}${range ? " · " : ""}${two(i)}</p>` +
      `<h2 class="n-title">${inline(s.title)}</h2>${s.subtitle ? `<p class="n-sub">${inline(s.subtitle)}</p>` : ""}${blocks}</section>`
    );
  }

  window.renderNotes = (sections) => (sections || []).map(section).join("");

  // Follow the macOS light/dark setting (the design system needs data-theme on <html>).
  const mq = matchMedia("(prefers-color-scheme: dark)");
  const apply = () => document.documentElement.setAttribute("data-theme", mq.matches ? "dark" : "light");
  apply();
  mq.addEventListener("change", apply);
})();
