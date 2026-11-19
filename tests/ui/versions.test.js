// Clean versions of the transcript (ui.html, "versions"; versions.rs is faked in prelude.js).
const $ = (id) => document.getElementById(id);
const shown = (id) => getComputedStyle($(id)).display !== "none";
// Claude's answer in these tests: line 3 rewritten, line 7 (the lab notice) dropped.
const CLEAN = (x, i) => i === 3 ? "The step size is called the learning rate, η." : i === 7 ? "" : x;
async function cleanUp(suggestions) {
  await T.click("#cleanBtn");
  await T.until(() => T.cleanJob(), "the clean up starts");
  T.cleanDone(CLEAN, suggestions);
  await T.until(() => T.texts().length === T.LINES.length - 1, "the clean version is shown");
}
async function pick(label) {
  await T.click("#verBtn");
  const row = [...$("verMenu").querySelectorAll("[data-ver]")].find((b) => b.querySelector(".t").textContent.startsWith(label));
  T.ok(row, `the menu lists ${label}`);
  await T.click(row);
}

T.test("Clean up shows progress, then the clean version; the original is one pick away", async () => {
  await T.open();
  T.ok(shown("verBar"), "the version bar is under the transcript");
  T.eq($("verName").textContent, "Original");
  T.ok(!shown("cmpBtn"), "nothing to compare yet");
  await T.click("#cleanBtn");
  T.eq(T.sent("cleanUp").map((m) => [m.path, m.course, m.model]), [[T.PATH, "Machine Learning", $("aiModel").value]]);
  T.ok(shown("verProg") && !shown("verBar"), "progress replaces the bar");
  T.cleanStep(1);
  await T.until(() => /1 of 2 parts/.test($("verProgTxt").textContent), "the parts done are counted");
  T.ok($("slidesBtn").disabled, "no slides while cleaning up (they work by line number)");
  await T.click(T.line(2), { dx: 4, dy: 10 });
  T.ok(!T.line(2).isContentEditable, "lines can't be edited while cleaning up");
  await T.shot("clean-progress");
  T.cleanDone(CLEAN);
  await T.until(() => shown("verBar"), "the bar is back");
  T.eq(T.texts()[3], "The step size is called the learning rate, η.");
  T.ok(!T.texts().some((x) => /bring your laptops/.test(x)), "the dropped line is gone");
  T.eq([$("verName").textContent, $("verSub").textContent], ["Clean", "19 Nov 10:30"]);
  T.ok(shown("cmpBtn"), "Compare is offered");
  await T.shot("clean-version");
  await pick("Original");
  T.eq(T.sent("versionSet").pop().id, "");
  await T.until(() => T.texts().length === T.LINES.length, "the original is shown again");
  T.eq(T.texts(), T.LINES.map((l) => l[1]));
  T.eq($("verName").textContent, "Original");
  await pick("Clean");
  await T.until(() => T.texts()[3].endsWith("η."), "and the clean one");
});

T.test("a line being edited is saved before the clean up starts from the lines", async () => {
  await T.open();
  await T.click(T.line(2), { dx: 4, dy: 10 });
  await T.type("Now ");
  await T.click("#cleanBtn");
  const order = T.sent().filter((m) => m.cmd === "lineEdit" || m.cmd === "cleanUp").map((m) => m.cmd);
  T.eq(order, ["lineEdit", "cleanUp"]);
  T.ok(!T.line(2).isContentEditable, "the line isn't being edited any more");
});

T.test("Stop ends the clean up and leaves the transcript as it was", async () => {
  await T.open();
  await T.click("#cleanBtn");
  await T.click("#verStop");
  T.eq(T.sent("cancel").pop().kind, "clean");
  await T.until(() => shown("verBar") && !$("cleanBtn").disabled, "Clean up is offered again");
  T.eq(T.texts(), T.LINES.map((l) => l[1]));
  T.ok(!$("slidesBtn").disabled, "slides are back");
});

T.test("edits go to the version shown; new lines are cleaned up by extending it", async () => {
  await T.open();
  await cleanUp();
  await T.click(T.line(1), { dx: 4, dy: 10 });
  await T.type("So ");
  await T.click(T.line(4));
  T.eq(T.sent("lineEdit").map((m) => [m.op, m.i, m.text]), [["set", 1, "So The idea is simple: we want to minimise a loss function."]]);
  await pick("Original");
  await T.until(() => T.texts().length === T.LINES.length, "the original");
  T.eq(T.texts()[1], T.LINES[1][1], "the original isn't touched");
  await pick("Clean");
  await T.until(() => T.texts()[1].startsWith("So "), "the edit is in the version");
  // Recording goes on into the original; the new lines show after the version's.
  T.commit(70, "Questions? Okay see you on Thursday.");
  await T.until(() => T.texts().length === T.LINES.length, "the new line shows");
  T.eq($("cleanNew").textContent, "· 1 new line");
  T.clearSent();
  await T.click("#cleanBtn");
  T.ok(!$("ask").classList.contains("on"), "extending doesn't ask");
  T.cleanDone((x) => x.replace("Okay see", "See"));
  await T.until(() => T.texts().pop() === "Questions? See you on Thursday.", "the new line is cleaned");
  T.eq($("cleanNew").textContent, "", "nothing new left");
  // A version that covers the whole lecture: Clean up asks before making another.
  await T.click("#cleanBtn");
  await T.until(() => $("ask").classList.contains("on"), "asks first");
  await T.key("Escape");
  T.eq(T.sent("cleanUp").length, 1, "cancelled: no second clean up");
});

T.test("Compare shows what Claude changed, word by word, without editing", async () => {
  await T.open();
  await cleanUp();
  await T.click("#cmpBtn");
  await T.until(() => $("cmpView").querySelector(".cmp-note"), "the comparison loads");
  T.ok(!shown("segs") && shown("cmpView"), "it replaces the lines");
  T.eq($("cmpView").querySelector(".cmp-note").textContent, "Clean · 19 Nov 10:30 compared with the Original: 1 line changed, 1 removed.");
  T.eq([...$("cmpView").querySelectorAll(".seg.gone .txt")].map((t) => t.textContent), [T.LINES[7][1]]);
  const row = $("cmpView").querySelectorAll(".seg")[3];
  T.eq([row.querySelector("del")?.textContent, row.querySelector("ins")?.textContent], ["eta.", "η."]);
  await T.shot("clean-compare");
  await T.dark("clean-compare-dark");
  await T.click(row.querySelector(".txt"));
  T.ok(!document.querySelector("#cmpView [contenteditable]"), "not editable");
  await T.click("#cmpBtn");
  T.ok(shown("segs") && !shown("cmpView"), "back to the lines");
  await T.click(T.line(2), { dx: 4, dy: 10 });
  T.ok(T.line(2).isContentEditable, "editable again");
  await T.key("Escape");
});

T.test("a version is renamed and deleted from the versions menu", async () => {
  await T.open();
  await cleanUp();
  await T.click("#verBtn");
  await T.shot("versions-menu");
  await T.dark("versions-menu-dark");
  await T.click("#verRename");
  T.ok($("ask").classList.contains("text") && document.activeElement === $("askIn"), "asks for the name");
  await T.type("Pulita"); await T.key("Enter");
  await T.until(() => $("verName").textContent === "Pulita", "renamed");
  T.eq(T.sent("versionRename").map((m) => m.name), ["Pulita"]);
  await T.click("#verBtn");
  await T.click("#verDelete");
  await T.until(() => $("ask").classList.contains("on") && !$("ask").classList.contains("text"), "asks to confirm");
  await T.click("#askYes");
  await T.until(() => $("verName").textContent === "Original", "the original is shown");
  T.eq(T.texts(), T.LINES.map((l) => l[1]));
  T.ok(!shown("cmpBtn"));
});

T.test("suggested corrections wait in the Glossary until accepted or skipped", async () => {
  await T.open();
  await cleanUp([{ heard: "Grady and", correct: "gradient" }, { heard: "Ada Grad", correct: "AdaGrad" }]);
  await T.until(() => $("sgSec"), "the Glossary opens on them");
  T.ok($("moreBtn").classList.contains("badge"), "the ⋯ button has a badge");
  T.eq([...$("sgSec").querySelectorAll("[data-corr]")].map((r) => r.querySelector("del").textContent + "→" + r.querySelector("b").textContent), ["Grady and→gradient", "Ada Grad→AdaGrad"]);
  await T.shot("clean-suggestions");
  await T.click('[data-sg="0"][data-do="accept"]');
  T.eq(T.sent("glossaryAdd").map((m) => m.corrections), [[{ heard: "Grady and", correct: "gradient" }]]);
  await T.click('[data-sg="1"][data-do="skip"]');
  T.eq(T.sent("glossaryAdd").length, 1, "skipped isn't added");
  T.ok(/1 of 2 added/.test($("sgSec").textContent));
  T.ok(!$("moreBtn").classList.contains("badge"), "nothing left to review");
});

T.test("the Glossary shows every row's words when the course has material", async () => {
  await T.open();
  await cleanUp([{ heard: "Grady and", correct: "gradient" }]);
  await T.until(() => $("sgSec"), "the Glossary opens on the suggestions");
  const course = $("course").value;
  T.fire({ type: "material", course, files: [{ name: "Slides week 1.pdf", pages: 40, emptyPages: 0 }] });
  T.fire({ type: "glossary", course, corrections: [{ heard: "Ada Grad", correct: "AdaGrad" }], terms: ["backpropagation"] });
  const body = $("glossBody");
  T.eq([...body.querySelectorAll("[data-corr]")].map((r) => r.querySelector("del").textContent + "→" + r.querySelector("b").textContent), ["Grady and→gradient"]);
  T.eq([...body.querySelectorAll(".m-row .c")].map((el) => el.textContent), ["Slides week 1.pdf"]);
  T.eq([...body.querySelectorAll(".g-corr")].map((r) => r.querySelector(".h").textContent + "→" + r.querySelector(".c").textContent), ["Ada Grad→AdaGrad"]);
  T.eq([...body.querySelectorAll(".g-term > span")].map((el) => el.textContent), ["backpropagation"]);
});
