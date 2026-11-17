// Check transcript: Claude's proposed fixes, reviewed in the Glossary tab (ui.html, "check transcript").
const result = (extra = {}) => ({
  type: "checkResult", path: T.PATH, usage: {}, lookups: [], terms: ["learning rate"],
  fixes: [
    { i: 2, t: 14, old: T.LINES[2][1], text: "We take the gradient and step in the opposite direction.", why: "move → step" },
    { i: 4, t: 31, old: T.LINES[4][1], text: "If η is too large we overshoot, if it's too small it takes forever.", why: "eta → η" },
  ],
  corrections: [{ heard: "grandient", correct: "gradient" }],
  ...extra,
});
async function checked(extra) {
  await T.open();
  await T.click("#moreBtn");
  await T.click("#checkBtn");
  T.fire(result(extra));
  await T.until(() => document.querySelector("#ckSec"), "the review list");
}
const edits = () => T.sent("lineEdit").map((m) => [m.i, m.text]);

T.test("Check transcript in the ⋯ menu asks Claude and shows progress in Glossary, with Stop", async () => {
  await T.open();
  await T.click("#moreBtn");
  await T.click("#checkBtn");
  T.eq(T.sent("checkTranscript").map((m) => m.path), [T.PATH]);
  T.ok($("glossView").style.display !== "none", "Glossary tab open");
  T.ok(/reading the transcript/.test($("glossView").textContent), "progress shown");
  T.ok($("checkBtn").disabled, "can't start a second check meanwhile");
  T.ok($("moreBtn").classList.contains("badge"), "a dot on ⋯ while it runs");
  await T.click("#ckStop");
  T.eq(T.sent("cancel").map((m) => m.kind), ["check"]);
  T.fire({ type: "checkError", path: T.PATH, message: "Stopped." });
  await T.until(() => $("ckStart"), "back to the start");
  T.ok(!document.querySelector(".toast.err") && !$("checkBtn").disabled, "not an error; can check again");
});

T.test("fixes are shown as a word diff, and their lines are marked", async () => {
  await checked();
  const fix = document.querySelector("[data-fix='0']");
  T.eq(fix.querySelector("del").textContent, "move"); T.eq(fix.querySelector("ins").textContent, "step");
  T.ok(fix.innerHTML.indexOf("<del>") < fix.innerHTML.indexOf("<ins>"), "the old word comes before the new one");
  T.eq(wordDiff("the sin packet opens", "the SYN packet opens"), "the <del>sin</del> <ins>SYN</ins> packet opens");
  T.eq(wordDiff("a b c", "a x y c"), "a <del>b</del> <ins>x y</ins> c");
  T.ok($("segs").children[2].classList.contains("has-fix") && $("segs").children[4].classList.contains("has-fix"), "lines 2 and 4 underlined");
  T.eq(T.sent("glossaryAdd").map((m) => m.terms), [["learning rate"]], "terms are added without asking");
  T.eq(edits(), [], "nothing changes before accepting");
  await T.shot("check-review");
});

T.test("Accept saves that line; Skip leaves the other", async () => {
  await checked();
  await T.click("[data-fix='0'] [data-do='accept']");
  await T.click("[data-fix='1'] [data-do='skip']");
  T.eq(edits(), [[2, "We take the gradient and step in the opposite direction."]]);
  T.eq(T.line(2).textContent, "We take the gradient and step in the opposite direction.");
  T.eq(T.line(4).textContent, T.LINES[4][1]);
  T.ok(!document.querySelector(".seg.has-fix"), "no marks left");
});

T.test("Accept all applies every fix and adds the corrections", async () => {
  await checked();
  await T.click("#ckAll");
  T.eq(edits().map((e) => e[0]), [2, 4]);
  T.eq(T.sent("glossaryAdd").filter((m) => m.corrections).map((m) => m.corrections), [[{ heard: "grandient", correct: "gradient" }]]);
  T.ok(/Reviewed: 2 of 2 fixes applied, 1 of 1 corrections added/.test($("ckSec").textContent), $("ckSec").textContent);
});

T.test("Skip all changes nothing", async () => {
  await checked();
  await T.click("#ckNone");
  T.eq(edits(), []); T.eq(T.sent("glossaryAdd").filter((m) => m.corrections), []);
});

T.test("a line you edited since the check isn't overwritten", async () => {
  await checked();
  await T.click(T.line(2), { dx: 4, dy: 10 }); await T.type("So "); await T.click(T.line(0));
  T.clearSent();
  await T.click("[data-fix='0'] [data-do='accept']");
  T.eq(edits(), []);
  T.ok(/Line changed since/.test(document.querySelector("[data-fix='0']").textContent));
});

T.test("a fix still finds its line after lines were added above it", async () => {
  await checked();
  await T.click("#segs .seg:nth-child(1) .add"); await T.type("A note."); await T.click(T.line(6));
  T.clearSent();
  await T.click("[data-fix='0'] [data-do='accept']");
  T.eq(edits(), [[3, "We take the gradient and step in the opposite direction."]]);
});

T.test("clicking a fix's time shows its line", async () => {
  await checked();
  await T.click("[data-fix='1'] a");
  T.ok($("segs").children[4].classList.contains("flash"), "line 4 flashes");
});

T.test("an error is shown with a way to retry", async () => {
  await T.open();
  await T.click("#moreBtn"); await T.click("#checkBtn");
  T.fire({ type: "checkError", path: T.PATH, message: "Claude Code isn't signed in." });
  T.eq($("ckErr").textContent, "Claude Code isn't signed in.");
  T.clearSent(); await T.click("#ckStart");
  T.eq(T.sent("checkTranscript").length, 1);
});
