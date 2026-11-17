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
