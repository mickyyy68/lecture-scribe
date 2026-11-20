// Injected by screenshot.swift before the page's own script, after `window.SHOT = {data, cut,
// slide, device}`. It plays the Rust side for one real lecture (data is `transcript-lecture
// --dump-lecture`), then stages the window as it looks `cut` seconds into recording that lecture:
// the transcript up to then, a sentence being heard, the deck up to slide `slide` on screen.
(() => {
  const { data, cut, slide, device } = window.SHOT;
  const all = data.transcript.lines;
  const lines = all.filter(([t]) => t <= cut);
  const next = all[lines.length]; // being heard: its first words show as the live line
  const slides = data.slides.slides.slice(0, slide);
  const fire = (e) => setTimeout(() => window.onEngine(e), 0);
  // The recording menu isn't open in the picture, but keep other microphones' names out of the page.
  const init = { ...data.init, devices: [device], defaultDevice: device, settings: { ...data.init.settings, device: "" } };

  window.ipc = { postMessage(raw) {
    const m = JSON.parse(raw);
    switch (m.cmd) {
      case "ready":
        fire(init);
        // Its file may be called just "Lecture"; the header shows the deck's title instead.
        fire({ ...data.transcript, title: data.slides.title || data.transcript.title, lines });
        setTimeout(stage, 300);
        break;
      case "preload": fire({ type: "modelReady", path: m.model, ms: 1 }); break;
      case "slidesState":
        // Made by Make slides half a minute ago, so a few lines are new since.
        fire({ ...data.slides, slides, lines: lines.length, done: lines.filter(([t]) => t <= cut - 30).length, lastT: null });
        break;
    }
  } };

  // Page globals (`elapsed`, `follow`, …) are top-level `let`s of the page's script.
  function stage() {
    window.onEngine({ type: "state", state: "recording", device });
    elapsed = cut + 3; // the timer runs from here
    window.onEngine({ type: "level", db: -22 });
    // Still: a blinking dot or a sliding meter could be caught half way. And wherever the pointer
    // happens to be, nothing is hovered or focused.
    document.head.insertAdjacentHTML("beforeend", "<style>*, *::before, *::after { animation: none !important; transition: none !important; } body { pointer-events: none; }</style>");
    document.activeElement?.blur();
    if (next) window.onEngine({ type: "partial", text: next[1].split(/\s+/).slice(0, 6).join(" ") });
    follow = true; scrollLive();
    document.getElementById("si" + (slide - 1))?.scrollIntoView({ block: "center" });
    window.SHOT.ready = true;
  }
})();
