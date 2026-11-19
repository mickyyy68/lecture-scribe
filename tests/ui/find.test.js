// Find (⌘F) in the transcript and in docs (ui.html, "find").
const $ = (id) => document.getElementById(id);
const boxes = () => [...document.querySelectorAll(".find-layer i")];
const near = (a, b) => Math.abs(a.left - b.left) < 3 && Math.abs(a.top - b.top) < 3;
// Where the k-th match of `word` is in `root` (to check the boxes sit on the text).
function textRect(root, word, k = 0) {
  const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT); let n, seen = 0;
  while ((n = w.nextNode())) {
    for (let at = n.data.toLowerCase().indexOf(word); at >= 0; at = n.data.toLowerCase().indexOf(word, at + 1)) {
      if (seen++ === k) { const r = document.createRange(); r.setStart(n, at); r.setEnd(n, at + word.length); return r.getBoundingClientRect(); }
    }
  }
}

T.test("⌘F finds in the transcript; Enter, ⇧Enter and ⌘G step through; Esc closes", async () => {
  await T.open();
  await T.click(T.line(0)); await T.key("Escape");
  await T.key("f", ["cmd"]);
  T.ok($("findBar").classList.contains("on") && $("left").contains($("findBar")), "the bar opens over the transcript");
  T.eq(document.activeElement, $("findIn"));
  await T.type("GRADIENT");
  T.eq($("findN").textContent, "1 of 3", "found without case");
  T.eq(boxes().length, 3);
  T.ok(near(document.querySelector(".find-layer i.cur").getBoundingClientRect(), textRect($("segs"), "gradient", 0)), "the box is on the first match");
  await T.shot("find-transcript");
  T.ok(document.querySelector(".find-layer i.cur").getBoundingClientRect().top > $("findBar").getBoundingClientRect().bottom, "the bar doesn't cover the match");
  await T.dark("find-transcript-dark");
  await T.key("Enter");
  T.eq($("findN").textContent, "2 of 3");
  T.ok(near(document.querySelector(".find-layer i.cur").getBoundingClientRect(), textRect($("segs"), "gradient", 1)), "on the second");
  await T.key("Enter", ["shift"]); T.eq($("findN").textContent, "1 of 3");
  await T.key("g", ["cmd"]); await T.key("g", ["cmd"]); T.eq($("findN").textContent, "3 of 3");
  await T.key("g", ["cmd"]); T.eq($("findN").textContent, "1 of 3", "wraps around");
  await T.key("G", ["cmd", "shift"]); T.eq($("findN").textContent, "3 of 3", "⇧⌘G goes back");
  // Boxes follow the text when it scrolls.
  $("scroller").scrollTop = 40; await T.wait(30);
  T.ok(near(document.querySelector(".find-layer i.cur").getBoundingClientRect(), textRect($("segs"), "gradient", 2)), "still on the text after scrolling");
  await T.key("Escape");
  T.ok(!$("findBar").classList.contains("on") && boxes().length === 0, "closed, no boxes left");
  T.eq(T.texts(), T.LINES.map((l) => l[1]), "the text is untouched");
});

T.test("matches follow new lines and edits; no match says so", async () => {
  await T.open();
  await T.key("f", ["cmd"]);
  await T.type("lab");
  T.eq($("findN").textContent, "1 of 1");
  T.commit(70, "The lab is in room B.");
  await T.until(() => $("findN").textContent === "1 of 2", "a new line's match is counted");
  await T.type("xyz");
  T.eq($("findN").textContent, "No results");
  T.eq(boxes().length, 0);
  await T.key("Escape");
});

T.test("in a doc, ⌘F finds in the doc and leaves it exactly as it was", async () => {
  const md = "# Gradient descent\n\nThe gradient points uphill.\n\n- step against the gradient\n";
  T.addDoc("Notes", md);
  await T.openDocs();
  await T.click("#docBody p");
  await T.key("f", ["cmd"]);
  T.ok($("docsView").contains($("findBar")), "the bar is over the doc");
  await T.type("gradient");
  T.eq($("findN").textContent, "1 of 3");
  T.ok(near(document.querySelector(".find-layer i.cur").getBoundingClientRect(), textRect($("docBody"), "gradient", 0)), "the box is on the heading's match");
  await T.shot("find-doc");
  await T.key("Enter");
  T.eq($("findN").textContent, "2 of 3");
  T.ok(!$("docBody").querySelector("i, mark"), "nothing was put in the doc");
  await T.key("Escape");
  // Typing in the doc saves exactly what the student wrote.
  await T.click("#docBody p"); await T.key("ArrowRight", ["cmd"]); await T.type(" Really.");
  await T.saved(md.replace("uphill.", "uphill. Really."));
  // Back in the transcript, ⌘F searches the transcript.
  await T.click(T.line(0)); await T.key("Escape");
  await T.key("f", ["cmd"]);
  T.ok($("left").contains($("findBar")), "the bar moved to the transcript");
  T.eq($("findN").textContent, "1 of 3", "and searched it");
  await T.key("Escape");
});

T.test("while comparing versions, find searches the comparison", async () => {
  await T.open();
  await T.click("#cleanBtn");
  await T.until(() => T.cleanJob(), "cleaning");
  T.cleanDone((x, i) => i === 7 ? "" : x);
  await T.until(() => T.texts().length === T.LINES.length - 1, "the version");
  await T.key("f", ["cmd"]); await T.type("laptops");
  T.eq($("findN").textContent, "No results", "the dropped line isn't in the version");
  await T.click("#cmpBtn");
  await T.until(() => $("findN").textContent === "1 of 1", "it is in the comparison");
  await T.key("Escape");
});
