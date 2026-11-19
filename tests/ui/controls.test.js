// Buttons and dialogs: the failures here were silent in the app (nothing happened on click).

T.test("Continue asks in the page, then records into the open lecture", async () => {
  await T.open();
  await T.click("#contBtn");
  T.ok($("ask").classList.contains("on"), "the dialog is shown");
  T.ok(/Continue recording into "Test Lecture"/.test($("askMsg").textContent), $("askMsg").textContent);
  await T.shot("continue-dialog");
  await T.click("#askYes");
  T.eq(T.sent("start").map((m) => m.continue), [T.PATH]);
});

T.test("Cancel in the dialog doesn't record", async () => {
  await T.open();
  await T.click("#contBtn");
  await T.click("#askNo");
  T.ok(!$("ask").classList.contains("on"), "closed");
  T.eq(T.sent("start"), []);
});

// The page re-renders the controls several times a second while recording (levels, timer). A
// re-render that replaces a button's content makes mouse down and up land on different nodes,
// and WebKit then drops the click: Pause/Stop/… would need many tries.
T.test("buttons keep working while the page re-renders during a recording", async () => {
  await T.open();
  T.record();
  await T.wait(50);
  const p = await T.down("#pauseBtn");
  for (let i = 0; i < 5; i++) { T.fire({ type: "level", db: -20 - i }); T.fire({ type: "state", state: "recording", device: "Test Microphone" }); await T.wait(10); }
  await T.up(p);
  T.eq(T.sent("pause").length, 1, "the Pause click registered");
});

T.test("re-rendering leaves every button's content alone", async () => {
  await T.open();
  T.record();
  const buttons = [...document.querySelectorAll("button")].filter((b) => !b.closest("#segs, #slideList, #setup"));
  const before = buttons.map((b) => [...b.childNodes]);
  for (let i = 0; i < 3; i++) T.fire({ type: "state", state: "recording", device: "Test Microphone" });
  await T.wait(30);
  const changed = buttons.filter((b, i) => b.childNodes.length !== before[i].length || [...b.childNodes].some((n, j) => n !== before[i][j]));
  T.eq(changed.map((b) => b.id || b.textContent), [], "buttons whose content was replaced");
});

// The Slides tab's capsule: the slide list menu jumps to a slide, Present opens the deck.
T.test("the slide menu lists the deck and jumps to a slide; Present opens it", async () => {
  await T.open();
  const s = (title, from) => ({ title, from, left: [{ type: "list", items: ["Take the gradient", "Step the other way"] }], notes: "" });
  T.fire({ type: "slides", path: T.PATH, title: "Test Lecture", lines: 8, done: 5, lang: "en", html: "/tmp/deck.html", flagged: 0, lookups: [],
    usage: { model: "sonnet", effort: "low", prompt: 1, cached: 0, output: 1 },
    slides: [s("Gradient descent", "00:00:00"), s("The **learning rate**", "00:00:22"), s("Mini-batches", "00:00:49")] });
  await T.until(() => $("slideCount").textContent === "3 slides", "the capsule counts the slides");
  T.eq($("slidesNew").textContent, "· 3 new lines");
  await T.shot("slides-capsule");
  await T.click("#slideMenuBtn");
  T.eq([...$("slideMenu").querySelectorAll("button .t")].map((e) => e.textContent), ["Gradient descent", "The learning rate", "Mini-batches"]);
  T.ok(/3 new lines since last time/.test($("slidesNote").textContent), $("slidesNote").textContent);
  await T.shot("slides-menu");
  await T.click('#slideMenu [data-i="2"]');
  T.ok(!$("slideMenu").classList.contains("open"), "the menu closes");
  await T.click("#presentBtn");
  T.eq(T.sent("open").map((m) => m.path), ["/tmp/deck.html"]);
});

// The window has its own design tokens (app.css): a slide theme restyles the slides, never the window.
T.test("a slide theme changes the slides, not the window", async () => {
  await T.open();
  const s = (title) => ({ title, from: "00:00:00", left: [{ type: "list", items: ["Take the gradient"] }], notes: "" });
  T.fire({ type: "slides", path: T.PATH, title: "Test Lecture", lines: 8, done: 8, lang: "en", html: "", flagged: 0, lookups: [], usage: null, slides: [s("Gradient descent")] });
  await T.until(() => document.querySelector("#slideList .pr-slide"), "a slide is shown");
  const look = () => [document.body, $("card"), $("capsule"), document.querySelector("header")].map((e) => getComputedStyle(e).backgroundColor + " " + getComputedStyle(e).color);
  const before = look();
  T.fire({ type: "theme", name: "Loud", css: ":root, [data-theme=\"dark\"] { --background: #ff0000; --foreground: #00ff00; --card: #0000ff; --muted: #ffff00; --primary: #ff00ff; }", themes: ["Loud"] });
  await T.wait(50);
  T.eq(getComputedStyle(document.querySelector("#slideList .pr-slide")).backgroundColor, "rgb(255, 0, 0)", "the slide follows the theme");
  T.eq(look(), before, "body, card, capsule and header keep the window's colours");
});

// The Slide style page's options are segmented controls: the chosen one must look chosen.
T.test("the Slide style options show which one is chosen", async () => {
  await T.open();
  await T.click("#moreBtn");
  await T.click("#setBtn");
  await T.click("#styleBtn");
  await T.until(() => document.querySelector(".st-ctl .switch"), "the Slide style page");
  const sw = document.querySelector(".st-ctl .switch");
  const bg = (b) => getComputedStyle(b).backgroundColor;
  const [on, off] = [sw.querySelector("button.on"), sw.querySelector("button:not(.on)")];
  T.ok(bg(on) !== bg(off), `chosen and other options look the same (${bg(on)})`);
  await T.click(off);
  const now = document.querySelector(`.st-ctl .switch[data-k="${sw.dataset.k}"] button.on`);
  T.eq(now.dataset.v, off.dataset.v, "clicking an option chooses it");
});

// Model, language, mic and "save the audio" live in a menu beside Start/Continue, not in the header.
T.test("the recording settings beside Start choose the mic, language and audio for the recording", async () => {
  await T.open();
  T.ok(!$("recMenu").classList.contains("open"), "closed at first");
  const r = $("recSetBtn").getBoundingClientRect(), c = $("contBtn").getBoundingClientRect();
  T.ok(Math.abs(r.top - c.top) < 4 && r.left > c.right, "the button sits right after Continue");
  await T.click("#recSetBtn");
  T.ok($("recMenu").classList.contains("open"), "the menu opens");
  T.ok($("langWrap").style.display !== "none", "a multilingual model shows Language");
  await T.shot("recording-settings");
  // A picker's list sits outside the menu: choosing from it must leave the menu open.
  await T.click($("lang").picker);
  await T.click([...document.querySelectorAll(".pk-list .pk-opt")].find((d) => d.textContent === "Italian"));
  T.eq($("lang").value, "it");
  T.ok($("recMenu").classList.contains("open"), "still open after choosing a language");
  await T.click($("device").picker);
  await T.click([...document.querySelectorAll(".pk-list .pk-opt")].find((d) => d.textContent === "Test Microphone"));
  T.eq($("device").value, "Test Microphone");
  await T.click("#saveAudio");
  T.eq($("saveAudio").checked, false);
  await T.click("#title"); // a click elsewhere closes it
  T.ok(!$("recMenu").classList.contains("open"), "a click outside closes the menu");
  await T.click("#startBtn");
  const m = T.sent("start")[0];
  T.eq([m.device, m.language, m.saveAudio], ["Test Microphone", "it", false]);
  T.record();
  await T.wait(30);
  T.eq($("recSetBtn").style.display, "none", "hidden while recording");
});

// Past lectures open from a searchable menu at the top left (there's no footer any more).
T.test("the lecture picker at the top left searches past lectures and opens one", async () => {
  await T.open();
  T.ok(!document.querySelector("footer"), "no footer");
  T.ok($("title").getBoundingClientRect().left < 40 && $("title").readOnly, "the lecture's name, at the top left, is read-only");
  await T.click("#title");
  T.ok($("lecMenu").classList.contains("open"), "the menu opens");
  T.eq(document.activeElement, $("lecFilter"), "the search field has the focus");
  await T.until(() => T.sent("recentList").length, "it asks for the latest list");
  const rows = () => [...document.querySelectorAll("#lecList button")];
  T.eq(rows().map((r) => r.querySelector(".t").textContent), T.RECENT.map((x) => x.name.slice(17)));
  T.ok(rows()[0].querySelector(".ok svg") && !rows()[1].querySelector(".ok svg"), "the open lecture is ticked");
  T.eq(rows()[4].querySelector(".k").textContent.slice(0, 11), "12 Jun 2025", "an older year shows the year");
  T.ok(/Test Lecture/.test($("lecFile").textContent) && $("words").textContent === "78 words", "the open lecture's file and words");
  await T.shot("lecture-picker");
  await T.type("grad");
  T.eq(rows().map((r) => r.querySelector(".t").textContent), ["Gradient descent, part 1"]);
  T.eq(rows()[0].querySelector("mark")?.textContent, "Grad", "the match is marked");
  await T.shot("lecture-picker-search");
  await T.type("zzz");
  T.eq(rows().length, 0); T.ok(/No lecture matches/.test($("lecList").textContent));
  for (let i = 0; i < 9; i++) await T.key("Backspace");
  T.eq(rows().length, T.RECENT.length, "clearing the search lists them all again");
  await T.key("ArrowDown"); await T.key("ArrowDown");
  await T.key("Enter");
  T.ok(!$("lecMenu").classList.contains("open"), "choosing closes the menu");
  T.eq(T.sent("loadTranscript").map((m) => m.path), [T.RECENT[2].path]);
  // A click works too, and ⌘O opens it.
  await T.key("o", ["cmd"]);
  T.ok($("lecMenu").classList.contains("open"), "⌘O opens it");
  await T.click(rows()[3]);
  T.eq(T.sent("loadTranscript").map((m) => m.path), [T.RECENT[2].path, T.RECENT[3].path]);
});

T.test("while recording, the lecture picker shows the file but can't switch lecture", async () => {
  await T.open();
  T.record();
  await T.click("#lecBtn");
  T.ok(document.querySelectorAll("#lecList button").length && [...document.querySelectorAll("#lecList button")].every((b) => b.disabled), "rows disabled");
  T.ok(/Stop the recording/.test($("lecList").textContent));
  await T.key("Enter");
  T.eq(T.sent("loadTranscript"), []);
});

T.test("the Slides and Docs tabs have icons", async () => {
  await T.open();
  T.eq([$("tabSlides"), $("docsBtn")].map((b) => b.querySelector("use")?.getAttribute("href")), ["#i-slides", "#i-doc"]);
  T.eq([$("tabSlides"), $("docsBtn")].map((b) => b.textContent), ["Slides", "Docs"]);
});

T.test("with no lecture open the name is a field; New lecture gets back to one", async () => {
  await T.click("#title");
  T.ok(!$("lecMenu").classList.contains("open") && document.activeElement === $("title"), "a fresh window: the click edits the name");
  await T.type("Week 1");
  T.eq($("title").value, "Week 1");
  await T.click("#lecBtn");
  T.ok($("lecMenu").classList.contains("open"), "the chevron still opens the picker");
  await T.key("Escape");
  await T.open();
  await T.click("#title");
  await T.click("#lecNew");
  T.ok(!$("lecMenu").classList.contains("open"), "closed");
  T.eq([$("title").value, $("title").readOnly, document.activeElement === $("title"), document.querySelectorAll("#segs .seg").length, $("contBtn").style.display],
    ["", false, true, 0, "none"], "an empty, named-by-you lecture");
  await T.type("Week 3");
  await T.shot("new-lecture");
  await T.click("#startBtn");
  T.eq(T.sent("start").map((m) => [m.title, m.continue]), [["Week 3", undefined]]);
});

// Rename (in the lecture picker): the name becomes a field; Enter renames the lecture and its files.
T.test("Rename in the lecture picker renames the open lecture and keeps its doc open", async () => {
  T.addDoc("Notes", "Hello\n");
  await T.openDocs();
  const doc = Docs.doc;
  await T.click("#title");
  T.ok($("lecRename").style.display !== "none" && !$("lecRename").disabled, "Rename is offered for the open lecture");
  await T.shot("lecture-picker-rename");
  await T.click("#lecRename");
  T.ok(!$("lecMenu").classList.contains("open") && !$("title").readOnly && document.activeElement === $("title"), "the name is a field, focused");
  T.eq([$("title").selectionStart, $("title").selectionEnd], [0, "Test Lecture".length], "with the old name selected");
  await T.type("Gradient descent");
  await T.shot("renaming");
  await T.key("Enter");
  T.eq(T.sent("renameLecture").map((m) => [m.path, m.title]), [[T.PATH, "Gradient descent"]]);
  const to = T.PATH.replace(/[^/]*$/, "Gradient descent.md");
  await T.until(() => $("lecFile").textContent === "Gradient descent", "the file's new name");
  T.ok($("title").readOnly && $("title").value === "Gradient descent", "back to the picker, with the new name");
  T.eq(Docs.doc, doc.replace("Test Lecture · Docs", "Gradient descent · Docs"), "the open doc stays open, in the renamed folder");
  T.eq(T.sent("loadTranscript"), [], "the lecture isn't reloaded");
  T.eq(T.sent("slidesState").at(-1).path, to, "the slides follow the new path");
  T.eq(T.$("#segs").children.length, T.LINES.length, "the lines stay");
  await T.click("#title");
  await T.until(() => document.querySelector("#lecList button .t")?.textContent === "Gradient descent", "the picker lists the new name");
  // Later edits and Continue use the new path.
  await T.key("Escape");
  await T.click("#contBtn"); await T.click("#askYes");
  T.eq(T.sent("start").map((m) => m.continue), [to]);
});

T.test("Escape, an empty name or a failed rename keep the old name", async () => {
  await T.open();
  const rename = async (text, key) => { await T.click("#title"); await T.click("#lecRename"); if (text != null) await T.type(text); await T.key(key); };
  await rename("Something else", "Escape");
  T.ok($("title").value === "Test Lecture" && $("title").readOnly, "Escape: unchanged");
  await rename(null, "Backspace"); await T.key("Enter");
  T.eq($("title").value, "Test Lecture", "an empty name: unchanged");
  await rename("Test Lecture", "Enter");
  T.eq(T.sent("renameLecture"), [], "nothing sent when nothing changed");
  await rename("Fail", "Enter");
  await T.until(() => /Permission denied/.test(document.body.textContent), "the error is shown");
  T.eq([$("title").value, $("lecFile").textContent], ["Test Lecture", "Test Lecture"], "the old name is back");
});

T.test("Rename isn't offered without a lecture, nor while recording", async () => {
  await T.click("#lecBtn");
  T.eq($("lecRename").style.display, "none", "no lecture open: no Rename");
  await T.key("Escape");
  await T.open(); T.record();
  await T.click("#lecBtn");
  T.ok($("lecRename").disabled && /Stop the recording/.test($("lecRename").title), "disabled while recording");
});
