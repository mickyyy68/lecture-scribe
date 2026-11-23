// The exported deck (`transcribe-lecture --dump-deck tests/ui/deck/slides.json`): presenting,
// and builds. Slide 1's drawing has two builds (data-step 1 and 2); slide 2 has none.

const $ = (id) => document.getElementById(id);
const later = () => document.querySelectorAll("#pfr .s-later").length;
const title = () => document.querySelector("#pfr .slide-title")?.textContent;

T.test("presenting reveals a drawing's builds one key at a time; going back shows a slide whole", async () => {
  T.eq(document.querySelectorAll("#deck .s-svg svg").length, 1, "the drawing is in the deck");
  T.eq(document.querySelectorAll("#deck .s-later").length, 0, "the deck itself shows every step (and so does printing it)");
  await T.key("p");
  await T.until(() => $("present").classList.contains("on"), "presenting");
  T.eq([title(), later()], ["Three-way handshake", 2], "opens before the first build");
  T.eq(getComputedStyle(document.querySelector("#pfr [data-step='2']")).opacity, "0", "a later build is invisible");
  await T.shot("deck-step-0");
  await T.key("ArrowRight");
  T.eq([title(), later()], ["Three-way handshake", 1], "→ reveals step 1");
  await T.key("ArrowRight");
  T.eq([title(), later()], ["Three-way handshake", 0], "→ reveals step 2");
  await T.shot("deck-step-2");
  await T.key("ArrowRight");
  T.eq([title(), later()], ["After the handshake", 0], "→ then the next slide");
  await T.key("ArrowLeft");
  T.eq([title(), later()], ["Three-way handshake", 0], "← back to the slide, whole");
  await T.key("ArrowLeft");
  T.eq([title(), later()], ["Three-way handshake", 1], "← hides the last build");
  await T.click("#pfr");
  T.eq(later(), 0, "a click moves forward too");
  await T.key("Escape");
  T.ok(!$("present").classList.contains("on"), "Esc leaves");
});
