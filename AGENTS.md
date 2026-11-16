# AGENTS.md

Lecture Scribe is a macOS app (Rust, tao + wry webview). It transcribes lectures live on the Mac with Handy's Parakeet GGUF models (`transcribe-cpp`, Metal). It also turns the transcript into slides in the "Protocolli & Reti" design system through the DeepSeek API, on demand. The user is a student. Lectures, prompts and slides are in Italian; UI chrome is in English. Read `README.md` for the user-facing behaviour.

## The running app is sacred

The user records real lectures with the installed app (`~/Applications/Lecture Scribe.app`). A lost recording can't be redone.

- **Never quit, kill, relaunch or reinstall the app** unless the user says no lecture is being recorded. Check first with `pgrep -fl "Lecture Scribe.app"`.
- **`./bundle.sh` installs.** It replaces `~/Applications/Lecture Scribe.app`. Run it only when the user asks to install. Don't pipe or background it "just to build".
- To build without installing, use the command below. Use `nice` and `-j 2` while a lecture may be recording, so transcription isn't starved:

  ```sh
  SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk nice -n 20 cargo build --release -j 2
  ```

  `SDKROOT` is required: the macOS 27 SDK's `libSystem.tbd` can't be read by the current linker.
- The new binary is `target/release/lecture-scribe`. Its headless modes can be tested while the installed app keeps recording.

## Map

| File | What |
|---|---|
| `src/engine.rs` | Recording worker: utterance splitting, Parakeet calls, live partials. Rewrites the transcript `.md` atomically on every change. Applies glossary corrections. |
| `src/audio.rs` | cpal mic capture and resampling to 16 kHz mono. |
| `src/models.rs` | Finds Handy's models (`~/Library/Application Support/com.pais.handy/models`, HF cache). |
| `src/main.rs` | Window, IPC commands from `ui.html`, headless CLI modes. |
| `src/ui.html` | Whole UI: transcript left, slide panel right, glossary/PDF view. Plain HTML/JS, no build step. |
| `src/slides.rs` | DeepSeek request and tool loop, deck storage, fact check, HTML export. |
| `src/slides_prompt.txt`, `src/slides_prompt_material.txt` | System prompt (Italian). The second part is appended only when the course has PDFs. |
| `src/slides.js`, `src/slides.css` | Slide renderer. **Shared** by `ui.html` and the exported `deck.html`, so changes must work in both. |
| `src/deck.html` | Template of the exported `<lecture> · Slide.html` (present/print). |
| `src/glossary.rs` | Per-course ASR corrections and terms. |
| `src/material.rs` | Course PDFs: PDFKit extraction (objc2), index, `search_material` / `read_page` tools. |
| `src/ds/` | The user's design system, **generated elsewhere ("Non modificare a mano")**. Don't edit. Put slide-specific styling in `src/slides.css`. |
| `notes/` | Superseded Python "Appunti" companion. Don't extend it. |

`main.rs::ui()` inlines CSS/JS into `ui.html` by replacing the `/*BUNDLE_CSS*/`, `/*TOKENS_CSS*/`, `/*SLIDES_CSS*/` and `/*SLIDES_JS*/` placeholders. `slides.rs::save()` fills `{{…}}` placeholders in `deck.html` the same way.

## Persisted formats (other code and existing user data depend on them)

- **Transcript** `~/Documents/Lecture Transcripts/<date> <title>.md`:
  - The first line is `# title`.
  - Blocks are `**[hh:mm:ss]** text`, separated by blank lines.
  - The last block is the live partial.
  - `slides::parse_transcript` relies on all of this. Change the engine's output format and you break slides for old and new lectures.
- **Deck** `<lecture> · Slide.json`:
  - A list of `batches`, one per click.
  - `rebuild: true` clears the earlier slides. `replace_last: true` pops the previous slide.
  - Undo pops the last batch. Keep these semantics backward compatible.
- **Course data** in `~/Library/Application Support/Lecture Scribe/`:
  - `glossary/<slug>.json` and `material/<course-slug>/<file-slug>.json`, keyed by `glossary::slug()`. Changing `slug()` orphans existing data.
  - Also `settings.json`.
- The engine's `Corrector` reloads the glossary file when its mtime changes, so edits apply during a recording.

## DeepSeek rules

- The model is `deepseek-flash` with thinking enabled. `reasoning_effort` is `low` per click and `high` for Rebuild. The user chose these, so don't change them unasked.
- **The API key lives only in the Keychain**: service `Lecture Scribe DeepSeek`, account `deepseek`. The `DEEPSEEK_API_KEY` env var overrides it. Never print it, log it or write it to a file. `slides::call` passes the header to curl through stdin so it's not visible in `ps`. Keep it that way.
- **Prompt caching depends on request order.** The order is: system prompt (+ material index) → full transcript (append-only) → per-call tail (glossary, previous slides, instruction). Never put per-call data before the transcript. Tool rounds only append messages. Assistant messages with `tool_calls` must be sent back with their `reasoning_content`.
- **Fidelity is the main product rule.** Slides contain only what the professor said. Course PDFs may only be used to correct spelling, follow naming and cite pages, never to add content. `fact_check` marks unverifiable spans with "(?)". `attach_refs` keeps only pages DeepSeek actually saw. Don't weaken these.
- Parakeet V3 has no vocabulary biasing. The glossary is post-correction of the text, not a model hint.

## Testing

- Unit tests: `cargo test --release --lib` (same `SDKROOT`).
- UI script syntax: extract the largest `<script>` from `src/ui.html`, drop the `/*SLIDES_JS*/` placeholder, then run `node --check` on it.
- `--simulate file.wav [--model path.gguf] [--lang it] [--speed N] [--course X]` runs the engine on a WAV and writes to a temp dir.
- `--slides file.md [--course X] [--rebuild]` makes a **real, paid** DeepSeek call (≈1–2 cents) and writes the deck next to the transcript. Run it only on a **copy** in `/tmp`, never on files in `~/Documents/Lecture Transcripts`.
- `--add-pdf file.pdf --course X` writes course material. Use a throwaway course name, and delete its `glossary/` and `material/` entries afterwards.
- After a prompt change, run `--slides` on a copied lecture and read the result: slide titles, `(?)` count, `usage`, and `lookups`.

## Done means

- `cargo build` is clean, with no warnings, and `cargo test --lib` passes.
- If you touched `ui.html`, the UI script syntax check passes.
- Any change to the slide pipeline is verified on a copied transcript.
- `README.md` is updated for user-visible changes.
- Nothing is installed unless the user asked.
