# AGENTS.md

Transcribe Lecture is a macOS app written in Rust, using tao and wry for the window and webview. It transcribes lectures live on the Mac with Parakeet GGUF models (`transcribe-cpp`, Metal). On demand, it turns the transcript into slides through Claude Code (`claude -p`) on the user's Claude subscription. The UI is in English. Slides are written in the language chosen in Settings, which by default is the lecture's language. Read `README.md` for the user-facing behaviour.

## Never interrupt a recording

People record real lectures with the installed app, and a lost recording can't be redone.

- **Never quit, kill, relaunch or reinstall the app** unless the user says no lecture is being recorded.
  - Check first with `pgrep -fl "Transcribe Lecture.app|Lecture Scribe.app"`.
  - "Lecture Scribe" is the app's old name, and that build may still be installed and running.
- **Install with `scripts/install.sh`, only when the user asks to install or restart.** It builds, and if the app is open it quits it (the app saves on quit) and starts the new one. It refuses, changing nothing, while the app is recording or paused: the app writes its state to `<data dir>/state`. Don't quit or reinstall the app any other way.
- `./bundle.sh` only builds `target/Transcribe Lecture.app` (`--install` copies it but refuses while the app is open).
- While a lecture may be recording, build with `nice` and `-j 2` so transcription isn't starved:

  ```sh
  nice -n 20 cargo build --release -j 2
  ```

  On a Mac whose linker can't read the newest SDK, prefix the build with an older SDK, e.g. `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk`.
- The binary is `target/release/transcribe-lecture`. Its headless modes can be tested while the installed app keeps recording.

## Map

| File | What |
|---|---|
| `src/engine.rs` | Recording worker (hand edits to the lecture being recorded go through it: `Cmd::EditLine`): utterance splitting, Parakeet calls, live partials, Continue (appending to an existing transcript and WAV). Rewrites the transcript `.md` atomically on every change. Applies glossary corrections. |
| `src/audio.rs` | cpal mic capture and resampling to 16 kHz mono. |
| `src/models.rs` | Parakeet V3 only: finds it (`<data dir>/models`, Handy's folder, HF cache) and downloads it (curl, resumable, SHA-256 checked). |
| `src/paths.rs` | Data folder, transcripts folder, settings. |
| `src/main.rs` | Window, menu bar, IPC commands from `ui.html`, headless CLI modes. |
| `src/ui.html` | The whole UI: transcript on the left; on the right a card (`#card`) with the Slides and Docs tabs, the capsule at its bottom that holds the tab's actions (it becomes the composer for a suggestion, and the progress with Stop while Claude works; see "the right side" in the script), the ⋯ menu, and the Glossary/PDF and Settings sheets over the card; and the first-run setup (`#setup`, shown until `settings.onboarded`). Plain HTML/JS, no build step. Write menus as plain `<select>`: `Picker` turns every select into the themed picker and keeps the `<select>` as the source of truth (`.value`, options, `onchange`). |
| `src/slides.rs` | `run_claude` and the shared `claude_command`, deck storage, fact check and HTML export. |
| `src/slides_prompt.txt`, `src/slides_prompt_material.txt` | The system prompt, in English. `{course}` and `{language}` are filled in per request. The second file is appended only when the course has PDFs. `slides::style_rules` appends the student's slide style (`settings.style`) as a STYLE section; each control's default adds nothing, so keep the defaults in `style_rules` and `STYLE` in `ui.html` in sync. |
| `src/sample_lecture.md` | Built-in English transcript excerpt for the Slide style page's samples (`slides::sample`) when there's no lecture yet. |
| `src/slides.js`, `src/slides.css` | Slide renderer. **Shared** by `ui.html` and the exported deck, so changes must work in both. Fixed labels ("Definition", "sketch", …) come from `LABELS`, keyed by `<html lang>`, with English as the fallback. |
| `src/deck.html` | Template of the exported `<lecture> · Slide.html` (present/print). |
| `src/app.css` | The window's design system ("warm paper"): every `--app-*` token of the chrome and the Docs editor (type, surfaces, ink, accent, status, corners, elevation, motion), light in `:root` and dark in `[data-theme="dark"]`. Inlined into `ui.html` as `/*APP_CSS*/`. |
| `src/theme/base.css` | Slide structure (`.pr-*` classes and the type scale). Uses tokens only, never literal colours or fonts. |
| `src/theme/default.css` | The default slide theme: every slide token, light and dark. System fonts only, so it works offline. |
| `src/claude-symbol.svg` | Claude symbol shown beside the model menu (Wikimedia Commons, CC0; Anthropic trademark, so use it only to mean "made by Claude"). Inlined into `ui.html` as `/*CLAUDE_SVG*/`. |
| `src/theme.rs`, `src/theme_prompt.txt` | Custom themes: list, load, delete, and `create` (Claude Code writes one from a description). |
| `src/glossary.rs` | Per-course ASR corrections and terms. |
| `src/check.rs`, `src/check_prompt.txt` | Check transcript: Claude Code proofreads a lecture and proposes fixed lines plus glossary corrections. It writes nothing: the UI ("check transcript" in `ui.html`) shows them in the Glossary tab and applies accepted ones as `lineEdit` / `glossaryAdd`. `check::fixes` drops fixes that rewrite more than about a third of a line. |
| `src/docs.rs`, `src/docs.js`, `src/docs_prompt.txt` | Docs: the student's Markdown docs in `<lecture> · Docs/` (`docs::handle` answers the page's `doc*` commands; only files in that folder; delete goes to the Trash) and **Draft notes** (`docs::draft`, Claude names the doc by its first `# ` line). `docs.js` is the editor, inlined after `ui.html`'s script as `/*DOCS_JS*/`: one contenteditable whose children are the blocks, our own Enter/Backspace/Tab/paste/undo, Markdown parse and serialise. Its chart formats come from `slides_prompt.txt` (`docs::chart_formats`), drawn by `slides.js` (`window.renderBlocks`). |
| `src/material.rs` | Course PDFs: PDFKit extraction (objc2), index, `search_material`/`read_page` tools, and the MCP server Claude Code uses for them (`--mcp-material`). |
| `assets/icon.svg`, `assets/AppIcon.icns` | The app icon. Edit the SVG, then run `scripts/make_icon.sh` (headless Chrome + `iconutil`) to regenerate the `.icns`, which `bundle.sh` copies into the app. |
| `src/katex/` | KaTeX, vendored with fonts inlined. Generated by `scripts/vendor_katex.py`; don't edit. Exported decks include it only when they contain math. |

`main.rs::ui()` inlines CSS and JS into `ui.html` by replacing its `/*…*/` placeholders. The chosen custom theme goes into `<style id="themeCss">` at runtime, so switching themes doesn't reload the page. `slides.rs::save()` fills the `{{…}}` placeholders in `deck.html` the same way, including `{{THEME}}` and `{{LANG}}`.

## Design tokens and themes

Two separate sets of tokens:

- **The window** (`src/app.css`, `--app-*`): the app's own design system. Everything in `ui.html`'s `<style>` and the Docs editor uses these, never the slide tokens, so a slide theme can't make the window unreadable.
  - Use the existing tokens. If a component needs a new value, add a token to `app.css` with a light and a dark value; no literal colours, fonts or shadows in `ui.html` or `docs.js`.
  - Buttons are secondary by default; `.go` is the primary action, `.rec` records, `.ghost` has no box. Floating things (menus, the capsule, dialogs, toasts) use `--app-raised` and `--app-sh2`/`--app-sh3`; segmented controls (`.tabs`, `.switch`) use `--app-seg-on`.
  - Dark mode: `slides.js` sets `data-theme` on `<html>` from the macOS setting, which switches both sets. Look at screenshots in both (the UI runner follows the system appearance; a test can set `data-theme` itself).
- **Slides** (`src/theme/default.css`, overridden by the chosen theme): slides in the card, the style page's samples, charts in docs, and exported decks.
  - A theme sets CSS custom properties only, in `:root` and `[data-theme="dark"]`.
  - If you add a slide token, give it a value in `default.css` and use it in `base.css` or `slides.css`. Existing user themes don't define it, so they keep the default.
  - Don't put literal colours or fonts in `base.css` or `slides.css`. A theme couldn't change them.
  - `base.css` is loaded into the window too: its bare class names (such as `.eyebrow`) reach the chrome, so override what they set where the chrome uses the same name.
- `theme::validate` rejects any answer containing `<`, so a theme can't break out of its `<style>` element. Keep that check.

## Persisted formats (existing user data depends on them)

- **Transcript** `~/Documents/Lecture Transcripts/<date> <title>.md`:
  - The first line is `# title`.
  - Blocks are `**[hh:mm:ss]** text`, separated by blank lines.
  - The last block is the live partial.
  - Continue appends a `_Continued …_` marker paragraph (older transcripts may have `_Ripresa: …_`), which the parser skips.
  - `slides::parse_transcript` relies on all of this. Change the engine's output format and you break slides for old and new lectures.
- **Deck** `<lecture> · Slide.json`. The `· Slide` suffix is kept for existing decks.
  - A list of `batches`, one per click.
  - `rebuild: true` clears the earlier slides. `replace_last: true` pops the previous slide.
  - `review: true` batches hold the review cards. The latest one replaces earlier ones. They are always shown after the lecture slides, and they keep `to_line` at the previous mark so they don't consume new lines.
  - `lang` is the slides' ISO language code.
  - Undo pops the last batch. Keep these semantics backward compatible.
  - Batches point at transcript lines by index (`from_line`, `to_line`). Hand edits (`slides::edit_body`) that add or remove a line call `slides::shift_deck` to move those indices.
- **Docs** `<lecture> · Docs/<name>.md` plus `images/`. Plain Markdown that other editors open: GitHub tables, `$…$`/`$$…$$`, ```` ```chart ```` blocks holding one slide diagram block as JSON, time links `[mm:ss](#t=hh:mm:ss)`. A doc loaded and saved without edits must come back byte for byte (the "every block type" UI test).
- **Data folder** `paths::data_dir()`, which is `~/Library/Application Support/Transcribe Lecture/`.
  - It falls back to the legacy `…/Lecture Scribe/` when only that one exists. Keep the fallback.
  - It holds `settings.json` and `themes/<name>.css`.
  - It also holds `glossary/<slug>.json` and `material/<course-slug>/<file-slug>.json`, keyed by `glossary::slug()`. Changing `slug()` orphans existing data.
- The engine's `Corrector` reloads the glossary file when its mtime changes, so edits apply during a recording.

## Slide engine rules

- **Claude Code** is the only engine. The user picks the model (`slides::MODELS`, default `sonnet`) and reasoning effort (`slides::EFFORTS`, default `low`) once, in Settings → Claude (`#aiModel`, `#effort`), for slides, docs and checks. Rebuild, Review, Draft notes and Check run at least at `high`. It runs `claude -p` on the user's subscription. Use `slides::claude_command`, which keeps the isolation:
  - It removes `ANTHROPIC_API_KEY` and related variables from the environment.
  - It passes `--strict-mcp-config` and `--setting-sources project`.
  - It runs in an empty temp working directory.
  - Slides also get `--tools ""` plus the material MCP server. Themes get `WebFetch` only.
  - Don't add `--bare`: it breaks subscription login.
- **Prompt caching depends on request order.** The order is:
  1. System prompt (+ material index). This is fixed per course and language.
  2. Full transcript (append-only).
  3. The per-call tail: glossary, previous slides, instruction.

  Never put per-call data before the transcript.
- **Fidelity is the main product rule.**
  - Slides contain only what the lecturer said.
  - Course PDFs may only be used to correct spelling, follow naming and cite pages, never to add content.
  - `fact_check` marks unverifiable spans with "(?)". `attach_refs` keeps only pages the model actually saw.
  - Don't weaken these.
- **Stop**: main.rs runs each Claude job inside `slides::job("slides" | "draft" | "check", …)`; `run_claude` records its claude process for the thread's job, and the page's `cancel` command kills it. The job then reports `slides::STOPPED` ("Stopped."), which the page shows as a stop, not an error. Run any new Claude job the same way.
- The student's free-text suggestion (`hint`: Make slides, Rebuild, Review cards, Draft notes; typed in the composer) goes in the per-call tail, after the transcript, and the prompt says fidelity comes first.
- Model output is normalised before saving: a single block in `left`/`right` becomes a list. `slides.js` also tolerates the old form, for old decks.
- Parakeet V3 has no vocabulary biasing. The glossary is post-correction of the text, not a model hint.

## Testing

**Run `scripts/verify.sh` before saying a change works.** About 25 s, and safe while a lecture is recording (low priority; it never touches the installed app, the user's lectures or settings). `--quick` skips the engine tests; `--ui NAME` runs only matching UI tests.

It runs:
- **Release build with no warnings** (`--all-targets`) and **unit tests** (`cargo test --release --lib`).
- **UI script syntax**: `node --check` on the largest `<script>` of `ui.html`.
- **UI tests in WebKit** (`tests/ui/`): `transcribe-lecture --dump-ui` prints the exact page the window shows; `tests/ui/runner.swift` loads it in a WKWebView (the engine the app uses, unlike Chrome) and drives it with **real AppKit mouse and key events**, so a click only counts if mouse down and up land on the same element, as for the user. `prelude.js` plays the Rust side: a fake lecture, the `init`/`slides`/`lineEdited` replies, and a record of every IPC command (`T.sent(cmd)`). It also fails any test where the page calls `confirm`/`alert`/`prompt`, which silently do nothing in wry. Tests are `T.test(name, async fn)` in `tests/ui/*.test.js`, each on a fresh page; `T.shot(name)` writes `target/ui-test/shots/<name>.png`. **Look at the screenshots** of anything visual you changed.
- **Engine tests** (`tests/engine.rs`, `#[ignore]`d because they need the Parakeet model): real transcription of speech made with `say`, hand edits during a recording, and Continue.

When you fix a bug, add the test that would have caught it, and see it fail before the fix. Cover user-visible UI behaviour through `T.click`/`T.type`/`T.key`, not by calling page functions.

Not covered by `verify.sh`, so check by hand (or tell the user it's unchecked):
- The menu bar (Cmd-C/V/X/A/Z come from `main.rs::app_menu`), microphone capture, and anything else outside the page.
- Slide rendering: fill `src/deck.html` with sample slides and screenshot it with headless Chrome (`--headless=new --screenshot`). Check light and dark, the default theme and a custom one, and `{{LANG}}` set to `en` and to another language.

Headless modes of `target/release/transcribe-lecture`:
- `--simulate file.wav [--model path.gguf] [--lang it] [--speed N] [--course X] [--continue file.md]` runs the engine on a WAV and writes to a temp dir. With `--continue`, run it on a **copy**.
- `--slides file.md [--course X] [--lang xx] [--rebuild | --review] [--model haiku] [--effort low]` makes a **real** Claude Code call on the user's subscription quota, and it writes the deck next to the transcript. Run it only on a **copy** in `/tmp`, never on files in `~/Documents/Lecture Transcripts`.
- `--download-model` downloads 740 MB into the data folder. To test it without touching the user's copy, run it with `HOME` set to a throwaway folder.
- `--make-theme NAME DESCRIPTION` makes a real Claude call and writes into the user's themes folder. Delete test themes afterwards.
- `--add-pdf file.pdf --course X` writes course material. Use a throwaway course name, and delete its `glossary/` and `material/` entries afterwards.
- `--dump-ui` prints the page (for the UI tests).
- `--draft file.md [--course X] [--lang it] [--model sonnet] [--effort high] [--hint "…"]` makes a **real** Claude Code call and writes a doc into `<lecture> · Docs/`. Run it on a copy.
- `--check file.md [--course X] [--model sonnet] [--effort high]` makes a **real** Claude Code call and prints the proposed fixes and corrections. It writes nothing, but still run it on a copy.
- After a prompt change, run `--slides` on a copied lecture and read the result: slide titles, language, the `(?)` count, `usage` and `lookups`.

## Done means

- `scripts/verify.sh` passes (all of it, not `--quick`, if you touched the engine), and new behaviour has a test.
- Any change to the slide pipeline is verified on a copied transcript.
- `README.md` is updated for user-visible changes.
- Nothing is installed unless the user asked; then use `scripts/install.sh`.
