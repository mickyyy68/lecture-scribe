// Editing transcript lines in place (ui.html, "editing lines by hand").
const edits = () => T.sent("lineEdit").map((m) => [m.op, m.i, m.text]);

T.test("clicking a line edits it, with the caret where you clicked", async () => {
  await T.open();
  const t = T.line(2);
  await T.click(t, { dx: 4, dy: 10 });
  T.ok(t.isContentEditable && document.activeElement === t, "line 2 is being edited");
  T.eq(caretOffset(t), 0, "caret at the start");
  await T.type("Now ");
  T.eq(t.textContent.slice(0, 8), "Now We t");
  await T.shot("editing-line");
});

T.test("leaving a line saves it once; unchanged lines aren't saved", async () => {
  await T.open();
  await T.click(T.line(0)); await T.click(T.line(1));
  T.eq(edits(), [], "nothing changed, nothing sent");
  await T.click(T.line(3), { dx: 4, dy: 10 });
  await T.type("Note: ");
  await T.click(T.line(5));
  T.eq(edits(), [["set", 3, "Note: The step size is called the learning rate, eta."]]);
  T.ok(!T.line(3).isContentEditable, "line 3 is plain again");
});

T.test("Enter splits the line at the caret, Backspace at its start joins it back", async () => {
  await T.open();
  const t = T.line(1);
  await T.click(t); caretAt(t, 12); // after "The idea is "
  await T.key("Enter");
  T.eq(T.texts().slice(1, 3), ["The idea is", "simple: we want to minimise a loss function."]);
  T.eq(document.activeElement, T.line(2), "the new line has the caret");
  T.eq(caretOffset(T.line(2)), 0);
  await T.key("Backspace");
  T.eq(T.texts().length, T.LINES.length, "joined back");
  T.eq(T.line(1).textContent, "The idea is simple: we want to minimise a loss function.");
  T.eq(caretOffset(T.line(1)), 12, "caret at the join");
  await T.key("Escape");
});

T.test("a split is saved as an edit plus a new line", async () => {
  await T.open();
  await T.click(T.line(4)); caretAt(T.line(4), 33); // "If eta is too large we overshoot,|"
  await T.key("Enter");
  await T.click(T.line(0));
  T.eq(edits(), [["set", 4, "If eta is too large we overshoot,"], ["insert", 5, "if it's too small it takes forever."]]);
  T.eq(T.texts().length, T.LINES.length + 1);
});

T.test("+ adds a line below; an empty new line is dropped", async () => {
  await T.open();
  const plus = $("segs").children[2].querySelector(".add").getBoundingClientRect(), time = $("segs").children[2].querySelector(".ts span").getBoundingClientRect();
  T.ok(plus.right <= time.left, "the + doesn't cover the time");
  await T.click("#segs .seg:nth-child(3) .add");
  T.eq(document.activeElement, T.line(3), "new line below line 2 has the caret");
  await T.type("(Board: the update rule)");
  await T.click(T.line(0));
  T.eq(edits(), [["insert", 3, "(Board: the update rule)"]]);
  T.eq(T.line(3).parentElement.querySelector(".ts span").textContent, "00:00:14", "it takes the time of the line above");
  T.clearSent();
  await T.click("#segs .seg:nth-child(1) .add");
  await T.click(T.line(5));
  T.eq(edits(), [], "nothing typed, nothing saved");
  T.eq(T.texts().length, T.LINES.length + 1);
});

T.test("Esc puts the line back", async () => {
  await T.open();
  await T.click(T.line(0), { dx: 4, dy: 10 });
  await T.type("XXX ");
  await T.key("Escape");
  T.eq(T.line(0).textContent, T.LINES[0][1]);
  T.eq(edits(), []);
});

T.test("emptying a line deletes it", async () => {
  await T.open();
  const t = T.line(7);
  await T.click(t); t.textContent = "";
  await T.click(T.line(0));
  T.eq(edits().at(-1), ["set", 7, ""]);
  T.eq(T.texts().length, T.LINES.length - 1);
});

T.test("↑ and ↓ move between lines at their edges", async () => {
  await T.open();
  await T.click(T.line(3)); caretAt(T.line(3), 0);
  await T.key("ArrowUp");
  T.eq(document.activeElement, T.line(2)); T.eq(caretOffset(T.line(2)), T.LINES[2][1].length, "at the end of the line above");
  await T.key("ArrowDown");
  T.eq(document.activeElement, T.line(3)); T.eq(caretOffset(T.line(3)), 0);
});

T.test("editing works while recording, and new lines don't disturb it", async () => {
  await T.open();
  T.record();
  await T.click(T.line(1), { dx: 4, dy: 10 });
  await T.type("So ");
  T.fire({ type: "commit", t: 70, text: "A line that arrives while you type." });
  T.fire({ type: "level", db: -20 });
  await T.wait(50);
  T.eq(document.activeElement, T.line(1), "still editing line 1");
  T.ok(T.line(1).textContent.startsWith("So The idea"), "typing kept");
  await T.type("x");
  await T.click(T.line(0));
  T.eq(edits(), [["set", 1, "So xThe idea is simple: we want to minimise a loss function."]]);
  T.eq(T.texts().at(-1), "A line that arrives while you type.");
});

T.test("lines can't be edited while slides are being made", async () => {
  await T.open();
  slidesBusy = true; renderSlidesBar();
  await T.click(T.line(1));
  T.ok(!T.line(1).isContentEditable, "not editable");
  slidesBusy = false; renderSlidesBar();
  await T.click(T.line(1));
  T.ok(T.line(1).isContentEditable, "editable again");
});

T.test("dragging across lines selects text instead of editing", async () => {
  await T.open();
  const p = await T.down(T.line(1), { dx: 10, dy: 10 });
  const r = T.line(3).getBoundingClientRect();
  await T.up({ x: r.left + 30, y: r.top + 10 }); // no mouse-moved events: select programmatically, as a drag would
  getSelection().setBaseAndExtent(T.line(1).firstChild, 2, T.line(3).firstChild, 5);
  T.line(1).dispatchEvent(new MouseEvent("mouseup", { bubbles: true, clientX: p.x, clientY: p.y }));
  T.ok(!T.line(1).isContentEditable, "a selection isn't turned into editing");
});
