// The Docs tab (docs.js): a lecture's own Markdown docs in a block editor.
const $ = (id) => document.getElementById(id);
const body = () => $("docBody");
async function clickEnd(sel) { const e = T.$(sel); const r = e.getBoundingClientRect(); await T.click(e, { dx: r.width - 2, dy: Math.min(12, r.height / 2) }); }

T.test("an empty lecture offers Draft notes in the capsule; + makes a blank doc that is named and saved", async () => {
  await T.openDocs();
  T.ok(/No docs yet/.test($("docEmpty").textContent), "says there are none");
  T.ok($("draftBtn").classList.contains("hl") && !$("draftBtn").disabled, "Draft notes is the highlighted action");
  T.eq($("docName").textContent, "No docs");
  await T.shot("docs-empty");
  await T.click("#docNew");
  await T.until(() => Docs.doc && document.activeElement === $("docTitle"), "the new doc opens with its title focused");
  T.eq(T.sent("docCreate").length, 1);
  await T.type("My notes"); await T.key("Enter");
  await T.until(() => T.sent("docRename").length === 1, "renamed");
  await T.until(() => Docs.doc.endsWith("/My notes.md"), "the doc follows its new name");
  T.eq($("docName").textContent, "My notes", "the capsule names the open doc");
  await T.type("Hello world");
  await T.saved("Hello world\n", "typed text is saved by itself");
  await T.until(() => $("docSavedTxt").textContent === "Saved", "shows Saved");
});

T.test("Markdown shortcuts make headings, lists, checklists and quotes", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  for (const [text, enters] of [["# Title", 1], ["Some text", 1], ["- one", 1], ["two", 2], ["1. first", 1], ["second", 2], ["[] task", 2], ["> said", 1]]) {
    await T.type(text);
    for (let i = 0; i < enters; i++) await T.key("Enter");
  }
  await T.saved("# Title\n\nSome text\n\n- one\n- two\n\n1. first\n2. second\n\n- [ ] task\n\n> said\n");
  T.eq(T.blocks().slice(0, 9), ["h1", "p", "div:ul", "div:ul", "div:ol", "div:ol", "div:todo", "blockquote", "p"]);
  await T.shot("docs-shortcuts");
});

const RICH = `# Lecture notes

Intro with **bold**, *italic*, ~~old~~, \`dig -x\`, a [link](https://example.com), a formula $x_i^2$ and a time [0:22](#t=00:00:22).

## Lists [0:40](#t=00:00:40)

- one
  - nested
- two

1. first
2. second

- [x] done
- [ ] todo

> **Exam:** this is in the exam.

\`\`\`python
for x in xs:
    print(x)
\`\`\`

$$
\\theta \\leftarrow \\theta - \\eta \\nabla L
$$

\`\`\`svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 720 120">
  <rect x="20" y="20" width="160" height="80" style="fill:var(--card);stroke:var(--foreground)"/>
  <g data-step="1"><text x="100" y="66" text-anchor="middle">step</text></g>
</svg>
\`\`\`

| Protocol | Port |
| --- | --- |
| DNS | \`53\` |
| HTTP | 80 |

\`\`\`chart
{"type":"bars","caption":"Steps","items":[{"label":"A","value":3},{"label":"B","value":5}]}
\`\`\`

---

![A caption](images/a.png)

Escapes: 2\\*3, \\$5, snake_case, \\_x\\_ and a line
with a break.
`;

T.test("a doc with every block type loads and saves unchanged", async () => {
  const png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
  T.addDoc("Rich", RICH, { "images/a.png": png });
  await T.openDocs();
  T.eq(Docs.serialize(), RICH, "serialised as read");
  T.ok(body().querySelector(".mathb .katex") && body().querySelector(".im .katex"), "formulas drawn");
  T.ok(body().querySelector(".chartb .dchart svg, .chartb .dchart div"), "chart drawn");
  T.ok(body().querySelector("figure img").src.startsWith("data:image/png"), "image shown");
  T.eq(body().querySelectorAll("td").length, 4);
  // An edit saves the whole doc back the same, plus the edit.
  await clickEnd("#docBody h2");
  await T.type("!");
  await T.saved(RICH.replace("## Lists [0:40](#t=00:00:40)", "## Lists [0:40](#t=00:00:40)!"));
  await T.shot("docs-rich");
  body().querySelector(".tbl").scrollIntoView({ block: "center" }); await T.wait(50);
  await T.shot("docs-rich-2");
});

T.test("Cmd+B and the selection toolbar format the selected words", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  await T.type("make this bold");
  for (let i = 0; i < 4; i++) await T.key("ArrowLeft", ["shift"]);
  await T.until(() => $("dtool").style.display === "block", "the toolbar appears over the selection");
  await T.shot("docs-toolbar");
  await T.key("b", ["cmd"]);
  await T.saved("make this **bold**\n");
  await T.click('#dtool [data-f="italic"]');
  await T.saved("make this ***bold***\n");
  await T.click('#dtool [data-f="code"]');
  await T.saved("make this `bold`\n");
});

T.test("Enter splits a paragraph, Backspace at its start joins it back", async () => {
  T.addDoc("Notes", "first second\n");
  await T.openDocs();
  const p = body().firstElementChild, r = document.createRange();
  // Caret before "second": click on it, then go to the word's start.
  await T.click(p, { dx: 2, dy: 10 });
  for (let i = 0; i < 6; i++) await T.key("ArrowRight");
  await T.key("Enter");
  await T.saved("first\n\nsecond\n");
  T.eq(T.blocks().slice(0, 2), ["p", "p"]);
  await T.key("Backspace");
  await T.saved("first second\n");
  void r;
});

T.test("/ opens the insert menu; it filters as you type and inserts a table", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  await T.type("/");
  await T.until(() => $("dslash").style.display === "block", "the menu opens");
  await T.shot("docs-slash");
  await T.type("tab");
  T.eq([...$("dslash").querySelectorAll(".ds-i span")].map((e) => e.textContent), ["Table"]);
  await T.key("Enter");
  T.ok(body().querySelector(".tbl"), "a table");
  await T.type("Name"); await T.key("Tab"); await T.type("Port"); await T.key("Tab"); await T.type("DNS"); await T.key("Tab"); await T.type("53");
  await T.saved("| Name | Port |\n| --- | --- |\n| DNS | 53 |\n|  |  |\n");
  T.eq(body().textContent.includes("/"), false, "the typed /tab is gone");
});

T.test("Cmd+Z undoes typing and block changes; Shift+Cmd+Z redoes", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  await T.type("abc"); await T.key("Enter"); await T.wait(1600); await T.type("def");
  await T.saved("abc\n\ndef\n");
  await T.key("z", ["cmd"]);
  await T.saved("abc\n", "typing undone");
  await T.key("z", ["cmd"]);
  T.eq(T.blocks().length <= 2 && body().firstElementChild.textContent, "abc", "the split undone");
  await T.key("z", ["cmd", "shift"]); await T.key("z", ["cmd", "shift"]);
  await T.saved("abc\n\ndef\n", "redone");
});

T.test("pasting Markdown makes blocks; pasting one line keeps it in the paragraph", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  await T.type("Start: ");
  await T.paste("## Part\n\n- a\n- b");
  await T.saved("Start:\n\n## Part\n\n- a\n- b\n");
  await T.type(" and more");
  await T.paste("plain words");
  await T.saved("Start:\n\n## Part\n\n- a\n- b and moreplain words\n");
});

T.test("a time link shows its moment in the transcript", async () => {
  T.addDoc("Notes", "See [0:31](#t=00:00:31).\n");
  await T.openDocs();
  await T.click("#docBody a.tl");
  const row = T.line(4).parentElement;
  await T.until(() => row.classList.contains("flash"), "the line at 0:31 flashes");
  T.eq(T.sent("open").length, 0, "nothing opened in the browser");
});

T.test("$$ then Enter writes a formula in a box with a preview", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  await T.click("#docBody");
  await T.type("$$"); await T.key("Enter");
  await T.until(() => $("dpop").style.display === "block" && document.activeElement.matches("#dpop textarea"), "the formula box opens");
  await T.type("x^2 + y^2");
  T.ok($("dpop").querySelector(".dp-prev .katex"), "live preview");
  await T.shot("docs-formula");
  await T.click('#dpop [data-x="ok"]');
  await T.saved("$$\nx^2 + y^2\n$$\n");
  T.ok(body().querySelector(".mathb .katex"), "drawn in the doc");
});

T.test("a checklist item is ticked by clicking its box", async () => {
  T.addDoc("Notes", "- [ ] buy milk\n");
  await T.openDocs();
  await T.click("#docBody .li", { dx: 8, dy: 12 });
  await T.saved("- [x] buy milk\n");
});

const shown = (id) => getComputedStyle($(id)).display !== "none";

T.test("Draft notes opens the composer; Enter sends the suggestion, the capsule shows progress, Claude's doc opens", async () => {
  await T.openDocs();
  await T.click("#draftBtn");
  await T.until(() => shown("composer") && document.activeElement === $("hintIn"), "the composer opens, ready to type");
  T.ok(!shown("capsule"), "it replaces the capsule");
  T.eq($("cmpGoTxt").textContent, "Draft notes");
  await T.type("focus on the lab");
  await T.shot("docs-composer");
  await T.key("Enter");
  const m = T.sent("docDraft")[0];
  T.eq([m.path, m.hint, m.model], [T.PATH, "focus on the lab", "sonnet"]);
  T.ok(shown("capProg") && /writing notes/.test($("progTxt").textContent) && !shown("composer"), "the capsule shows the progress");
  await T.shot("docs-drafting");
  const p = T.addDoc("Gradient descent", "## Idea [0:06](#t=00:00:06)\n\nWe minimise a loss.\n");
  T.fire({ type: "docDrafted", path: T.PATH, doc: p, docs: [{ name: "Gradient descent", path: p }] });
  await T.until(() => Docs.doc === p && body().textContent.includes("We minimise"), "Claude's doc opens");
  T.eq($("docTitle").value, "Gradient descent");
  T.ok(shown("capsule") && !shown("capProg") && !$("draftBtn").disabled, "ready again");
  await T.click("#draftBtn");
  T.eq($("hintIn").value, "", "the suggestion was used up");
});

T.test("the doc menu switches doc; Move to Trash asks, then moves the doc to the Trash", async () => {
  T.addDoc("Old", "x\n"); T.addDoc("Keep", "y\n");
  await T.openDocs();
  await T.until(() => Docs.doc.endsWith("Keep.md"), "the newest doc opens");
  await T.click("#docMenuBtn");
  T.eq([...$("docMenu").querySelectorAll("[data-open] .t")].map((e) => e.textContent), ["Old", "Keep"]);
  await T.shot("docs-menu");
  await T.click('#docMenu [data-open="0"]');
  await T.until(() => Docs.doc.endsWith("Old.md") && $("docName").textContent === "Old", "Old opens");
  await T.click("#docMenuBtn"); await T.click('#docMenu [data-open="1"]');
  await T.until(() => Docs.doc.endsWith("Keep.md"), "back to Keep");
  await T.click("#docMenuBtn");
  await T.click('#docMenu [data-act="trash"]');
  T.ok($("ask").classList.contains("on") && /Trash/.test($("askMsg").textContent), "asks first");
  await T.click("#askYes");
  await T.until(() => Docs.doc && Docs.doc.endsWith("Old.md"), "the other doc opens");
  T.eq(T.sent("docDelete").map((m) => m.doc.split("/").pop()), ["Keep.md"]);
  T.eq(T.sent("docSave").length, 0, "nothing written back to the deleted doc");
});

T.test("Make slides opens the composer and sends the suggestion; Esc closes it without sending", async () => {
  await T.open();
  T.eq($("slidesNew").textContent, "· 8 new lines", "how much is new");
  await T.click("#slidesBtn");
  await T.until(() => document.activeElement === $("hintIn"), "composer");
  T.ok(/00:00:00 → 00:00:58/.test($("cmpInfo").textContent), $("cmpInfo").textContent);
  await T.type("more"); await T.key("Escape");
  T.ok(!shown("composer") && shown("capsule"), "closed");
  T.eq(T.sent("slides").length, 0);
  await T.click("#slidesBtn");
  T.eq($("hintIn").value, "more", "the text is kept until sent");
  await T.type(" diagrams");
  await T.shot("slides-composer");
  await T.click("#cmpGo");
  T.eq(T.sent("slides").map((m) => [m.mode, m.hint]), [["next", "more diagrams"]]);
});

T.test("Stop cancels Claude's job; the capsule comes back and no error is shown", async () => {
  await T.open();
  await T.key("Enter", ["cmd", "shift"]);
  T.eq(T.sent("slides").map((m) => m.hint), [""], "⇧⌘↩ makes slides straight away");
  await T.until(() => shown("capProg"), "progress");
  await T.wait(1400);
  T.ok(/making slides from 8 new lines/.test($("progTxt").textContent) && /^0:0[12]$/.test($("progTime").textContent), $("progTxt").textContent + " " + $("progTime").textContent);
  await T.click("#capStop");
  T.eq(T.sent("cancel").map((m) => m.kind), ["slides"]);
  T.fire({ type: "slidesError", path: T.PATH, message: "Stopped." });
  await T.until(() => shown("capsule") && !shown("capProg"), "the capsule is back");
  T.ok(!document.querySelector(".toast.err"), "not shown as an error");
});

T.test("a draft running on the Docs tab shows as a dot there while you're on Slides", async () => {
  await T.openDocs();
  await T.click("#draftBtn"); await T.key("Enter");
  await T.click("#tabSlides");
  T.ok($("docsBtn").classList.contains("job") && shown("capsule"), "dot on Docs; the Slides capsule is free");
  await T.click("#docsBtn");
  T.ok(shown("capProg"), "progress on Docs");
  await T.click("#capStop");
  T.eq(T.sent("cancel").map((m) => m.kind), ["draft"]);
});

T.test("⋯ opens Settings as a sheet over the card; the model set there is the composer's", async () => {
  await T.open();
  await T.click("#moreBtn");
  await T.shot("more-menu");
  await T.click("#setBtn");
  T.ok(shown("setView") && !shown("capsule") && !$("moreMenu").classList.contains("open"), "the Settings sheet covers the card");
  await T.click($("aiModel").picker);
  await T.click([...document.querySelectorAll(".pk-opt")].find((o) => o.textContent === "Opus"));
  await T.shot("settings-claude");
  await T.click("#themeNew");
  T.ok(shown("themePage") && document.activeElement === $("themeName"), "New theme is a page of its own");
  await T.click("#setBack");
  await T.click("#setView .sh-done");
  T.ok(!shown("setView") && shown("capsule"), "Done goes back");
  await T.click("#slidesBtn");
  T.eq($("cmpModel").textContent, "Opus · Low");
  await T.click("#cmpModel");
  await T.click('#modelMenu [data-sel="effort"][data-v="high"]');
  T.eq([$("effort").value, $("cmpModel").textContent], ["high", "Opus · High"], "the chip changes the shared setting");
  await T.key(",", ["cmd"]);
  T.ok(shown("setView"), "⌘, opens Settings");
  await T.key("Escape"); await T.key("Escape");
  T.ok(!shown("setView"), "Esc closes it");
});

T.test("docs can be written while recording", async () => {
  T.addDoc("Notes", "");
  await T.openDocs();
  T.record();
  await T.click("#docBody");
  await T.type("live note");
  T.fire({ type: "commit", t: 70, text: "A new line arrives." });
  await T.type("s");
  await T.saved("live notes\n");
  T.ok(document.activeElement === body(), "focus stays in the doc");
});
