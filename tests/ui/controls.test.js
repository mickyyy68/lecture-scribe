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
