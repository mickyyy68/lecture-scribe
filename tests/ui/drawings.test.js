// SVG drawings (the slides' "svg" block, ```svg in docs): Claude writes the SVG, so the page must
// draw only drawing (slides.js sanitizeSvg), fit it to its space, follow the theme, and tell the
// app what's wrong with it once drawn (checkSvg, answered to `svgCheck`).

const slidesEv = (slides) => ({ type: "slides", path: T.PATH, title: "Test Lecture", lines: 8, done: 8, lang: "en", html: "", flagged: 0, lookups: [], usage: null, slides });
const slide = (title, left, right) => ({ title, from: "00:00:00", layout: right ? "two" : "one", left, right: right || [], notes: "" });
// The colour a CSS colour value computes to, here.
const colorOf = (v, inside) => { const p = document.createElement("span"); p.style.color = v; (inside || document.body).append(p); const c = getComputedStyle(p).color; p.remove(); return c; };

const HOSTILE = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 540 300" width="9000" onload="window.pwned='onload'" style="position:fixed;inset:0">
  <script>window.pwned = "script"</script>
  <script><![CDATA[window.pwned = "cdata"]]></script>
  <style>@import url(https://example.com/x.css); body { display: none } svg { position: fixed; top: 0 } rect.box { fill: var(--card); stroke: url(https://example.com/s.svg#p) }
    @keyframes spin { to { transform: rotate(1turn) } } .spin { animation: spin 4s linear infinite } #box { stroke-width: 3 !important }</style>
  <defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 0L10 5L0 10z" style="fill:var(--primary)"/></marker></defs>
  <rect class="box" id="box" x="10" y="10" width="100" height="50" onclick="window.pwned='onclick'"/>
  <foreignObject width="100" height="100"><img xmlns="http://www.w3.org/1999/xhtml" src="x" onerror="window.pwned='img'"/></foreignObject>
  <a href="javascript:window.pwned='link'"><text x="20" y="160">link</text></a>
  <image href="https://example.com/a.png" width="10" height="10"/>
  <use href="https://example.com/sprite.svg#icon"/>
  <use xlink:href="#box" x="200"/>
  <animate xlink:href="#box" attributeName="href" to="javascript:window.pwned='animate'"/>
  <set attributeName="onmouseover" to="window.pwned='set'"/>
  <line x1="10" y1="100" x2="200" y2="100" marker-end="url(#arrow)" style="stroke:var(--diagram-link)"/>
  <rect x="300" y="10" width="50" height="50" style="fill:url(https://example.com/x);stroke:var(--foreground);position:fixed"/>
  <title><style>body { display: none }</style>A box</title>
  <text x="20" y="260">$x_1$ label</text>
  <iframe src="javascript:parent.pwned='iframe'"/>
  <g class="spin"><circle cx="400" cy="200" r="20"/></g>
</svg>`;
// Another drawing on the same page with the same ids: each must use its own.
const SAME_IDS = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 540 200"><defs><marker id="arrow"><path d="M0 0L4 2L0 4z"/></marker></defs>
  <line x1="10" y1="50" x2="200" y2="50" marker-end="url(#arrow)" style="stroke:var(--foreground)"/><rect id="box" x="300" y="20" width="40" height="40"/></svg>`;

T.test("a drawing keeps only drawing: no scripts, handlers, links, outside URLs or page-wide styles", async () => {
  await T.open();
  window.pwned = undefined;
  T.fire(slidesEv([slide("Hostile", [{ type: "svg", alt: "A box and an arrow.", svg: HOSTILE }]), slide("Same ids", [{ type: "svg", alt: "x", svg: SAME_IDS }])]));
  await T.until(() => document.querySelectorAll("#slideList .s-svg svg").length === 2, "both drawings are drawn");
  const [a, b] = document.querySelectorAll("#slideList .s-svg svg");
  await T.click(a.querySelector("rect.box"));
  await T.wait(300);
  T.eq(window.pwned, undefined, "no payload ran");
  for (const svg of [a, b]) {
    T.eq(svg.querySelectorAll("script, foreignObject, image, a, iframe, img").length, 0, "elements that run or fetch things");
    for (const el of [svg, ...svg.querySelectorAll("*")])
      for (const at of el.attributes) {
        if (at.name.startsWith("xmlns")) continue;
        T.ok(!/^on/i.test(at.name), `event attribute ${at.name} on <${el.localName}>`);
        T.ok(!/javascript:|https?:|example\.com/i.test(at.value), `<${el.localName} ${at.name}="${at.value}">`);
        if (at.localName === "href") T.ok(at.value.startsWith("#"), `href ${at.value}`);
      }
    T.ok(!/@import|example\.com|!important/.test([...svg.querySelectorAll("style")].map((s) => s.textContent).join("")), "the drawing's <style> keeps no imports, outside URLs or !important");
  }
  // Its styles only reach itself, and can't move it out of its box.
  T.ok(getComputedStyle(document.body).display !== "none" && $("card").getBoundingClientRect().width > 0, "the page is still shown");
  T.eq(getComputedStyle(a).position, "static", "the drawing stays in the slide");
  T.ok(a.getBoundingClientRect().width < document.querySelector("#slideList .pr-slide").getBoundingClientRect().width, "and in its width");
  T.eq(getComputedStyle(a.querySelector("rect.box")).fill, colorOf("var(--card)", a.closest(".pr-slide")), "a <style> rule still styles its own shapes");
  T.ok(/spin-s\d+/.test(getComputedStyle(a.querySelector(".spin")).animationName), "its keyframes are renamed to its own");
  T.eq(a.querySelector("title").textContent, "A box", "a title is text only");
  T.ok([...a.querySelectorAll("text")].some((t) => t.textContent === "x₁ label"), "math in a label becomes Unicode");
  // Same ids in two drawings: each line ends on its own marker, each <use> shows its own box.
  const mk = (svg) => svg.querySelector("line").getAttribute("marker-end").match(/#([^)]+)/)[1];
  T.ok(mk(a) !== mk(b) && a.querySelector(`marker[id="${mk(a)}"]`) && b.querySelector(`marker[id="${mk(b)}"]`), "markers resolve inside their own drawing");
  T.ok(a.querySelector(`[id="${a.querySelector("use[href]").getAttribute("href").slice(1)}"]`), "<use> points into its own drawing");
  T.eq(a.querySelectorAll("use:not([href])").length, 1, "the <use> of an outside file points nowhere");
  await T.shot("svg-hostile");
});

// Builds and animated parts in the app's slide list: every step shows, in light and dark.
const LOOP = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 540 300">
  <style>.pkt { animation: move 3s linear infinite } @keyframes move { from { transform: translateX(0) } to { transform: translateX(300px) } }</style>
  <rect x="20" y="110" width="120" height="70" style="fill:var(--card);stroke:var(--foreground)"/>
  <text x="80" y="150" text-anchor="middle">Client</text>
  <line x1="140" y1="145" x2="400" y2="145" style="stroke:var(--diagram-link);stroke-width:2"/>
  <g data-step="1"><rect x="400" y="110" width="120" height="70" style="fill:var(--primary)"/><text x="460" y="150" text-anchor="middle" style="fill:var(--primary-foreground)">Server</text></g>
  <g data-step="2"><circle class="pkt" cx="150" cy="145" r="8" style="fill:var(--warning)"/><text x="270" y="130" text-anchor="middle" style="fill:var(--muted-foreground)">SYN</text></g>
</svg>`;
const WIDE = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1120 300"><rect x="10" y="10" width="1100" height="280" rx="12" style="fill:none;stroke:var(--border)"/>
  <path d="M40 250 C 300 40, 600 40, 1080 250" style="fill:none;stroke:var(--chart-4);stroke-width:4"/><text x="560" y="90" text-anchor="middle">the loss</text></svg>`;

T.test("drawings fill their column or the slide, show every build step, and follow the theme", async () => {
  await T.open();
  T.fire(slidesEv([
    slide("Handshake", [{ type: "list", items: ["The client opens", "The server answers"] }], [{ type: "svg", alt: "Client and server.", caption: "Three-way handshake", svg: LOOP }]),
    slide("Loss", [{ type: "svg", alt: "A curve.", svg: WIDE }]),
  ]));
  await T.until(() => document.querySelectorAll("#slideList .s-svg svg").length === 2, "both drawings are drawn");
  const [col, full] = document.querySelectorAll("#slideList .s-svg svg");
  const share = (svg) => svg.getBoundingClientRect().width / svg.closest(".pr-slide").getBoundingClientRect().width;
  T.ok(share(col) > 0.4 && share(col) < 0.43, `a 540-wide drawing fills its column (${share(col).toFixed(3)} of the slide)`);
  T.ok(share(full) > 0.86, `a 1120-wide drawing fills the slide (${share(full).toFixed(3)})`);
  T.ok(col.getBoundingClientRect().bottom <= col.closest(".pr-slide").querySelector(".pr-foot").getBoundingClientRect().top, "above the footer");
  T.eq(col.closest(".pr-slide").querySelector(".s-cap")?.textContent, "Three-way handshake", "caption");
  T.eq(document.querySelectorAll("#slideList .s-later").length, 0, "outside presenting, every step shows");
  const label = col.querySelectorAll("text")[0], slideEl = col.closest(".pr-slide");
  T.eq(getComputedStyle(label).fill, colorOf("var(--foreground)", slideEl), "an unstyled label is in the theme's ink");
  await T.shot("svg-light");
  const light = getComputedStyle(label).fill;
  document.documentElement.dataset.theme = "dark"; await T.wait(60);
  try {
    T.ok(getComputedStyle(label).fill !== light && getComputedStyle(label).fill === colorOf("var(--foreground)", slideEl), "and in dark, the dark ink");
    await T.shot("svg-dark");
  } finally { document.documentElement.dataset.theme = "light"; }
});

// What svg.rs gets back when it asks about new drawings.
T.test("the check reports labels outside or overlapping, tiny text, literal colours and broken SVG", async () => {
  await T.open();
  const bad = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 540 200"><rect x="20" y="20" width="120" height="60" fill="#333"/>
    <text x="30" y="60">Client</text><text x="34" y="62">Server</text><text x="500" y="120">Router</text></svg>`;
  const tiny = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2000 600"><text x="100" y="100" style="fill:var(--foreground)">small</text></svg>`;
  T.fire({ type: "svgCheck", id: 41, items: [{ svg: bad, width: 540 }, { svg: LOOP, width: 540 }, { svg: tiny, width: 540 }, { svg: "<svg><g></svg>", width: 540 }, { svg: "<svg xmlns='http://www.w3.org/2000/svg'><text>x</text></svg>", width: 540 }] });
  await T.until(() => T.sent("svgChecked").length, "the page answers");
  const [m] = T.sent("svgChecked");
  T.eq(m.id, 41);
  const [b, ok, small, broken, noBox] = m.problems;
  T.eq(b.map((p) => p.fatal), [true, true, false], b.map((p) => p.text).join(" / "));
  T.ok(/"Router" goes outside the viewBox \(0 0 540 200\)/.test(b[0].text), b[0].text);
  T.ok(/"Client" and "Server" overlap/.test(b[1].text), b[1].text);
  T.ok(/literal colours \(#333\)/.test(b[2].text), b[2].text);
  T.eq(ok, [], "a clean drawing (with builds and motion) has nothing to fix");
  T.ok(small.length === 1 && !small[0].fatal && /shows at 4\.3px/.test(small[0].text), JSON.stringify(small));
  T.ok(broken.length === 1 && broken[0].fatal && /isn't valid SVG/.test(broken[0].text), JSON.stringify(broken));
  T.ok(noBox.length === 1 && /needs a viewBox/.test(noBox[0].text), JSON.stringify(noBox));
});

T.test("a slide whose drawing can't be drawn shows its alt sentence", async () => {
  await T.open();
  T.fire(slidesEv([slide("Broken", [{ type: "svg", alt: "The router forwards the packet.", svg: "<svg><g></svg>" }])]));
  await T.until(() => document.querySelector("#slideList .pr-slide"), "the slide is shown");
  T.eq(document.querySelector("#slideList .pr-body").textContent, "The router forwards the packet.");
});

T.test("Docs: /Drawing adds an SVG drawing; its editor previews it and lists what's wrong", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  await T.type("/draw");
  await T.until(() => $("dslash").style.display === "block", "the menu opens");
  T.eq([...$("dslash").querySelectorAll(".ds-i span")].map((e) => e.textContent), ["Drawing"]);
  await T.key("Enter");
  await T.until(() => $("dpop").style.display !== "none" && $("dpop").querySelector(".dp-chart svg"), "the editor opens with a sample drawing");
  T.eq($("dpop").querySelectorAll(".dp-err").length, 0, "the sample has nothing wrong");
  await T.shot("docs-svg-edit");
  await T.key("ArrowUp", ["cmd"]); // select it all (Cmd-A comes from the app's menu bar, which the tests don't have)
  await T.key("ArrowDown", ["cmd", "shift"]);
  await T.type(`<svg viewBox="0 0 720 100"><text x="690" y="50">far away</text></svg>`);
  await T.until(() => /"far away" goes outside/.test($("dpop").querySelector(".dp-err")?.textContent || ""), "the problem is listed under the preview");
  await T.click('#dpop [data-x="ok"]');
  await T.until(() => T.docs[Docs.doc]?.md.includes('```svg\n<svg viewBox="0 0 720 100"><text x="690" y="50">far away</text></svg>\n```'), "saved as a ```svg block");
  T.ok(document.querySelector("#docBody .chartb[data-lang=svg] svg"), "drawn in the doc");
});
