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
