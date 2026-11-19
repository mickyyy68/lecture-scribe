# Transcribe Lecture

A macOS app that transcribes lectures live, on your Mac, and turns what was said into slides you can study from.

![Transcribe Lecture: a lecture's transcript on the left, the slides made from it on the right](docs/screenshot.png)

- **Live transcription on the Mac.** It uses NVIDIA's Parakeet V3 speech model (25 European languages; GGUF, via [`transcribe-cpp`](https://crates.io/crates/transcribe-cpp)) on the GPU with Metal. Audio never leaves the computer.
- **Slides on demand.** Press **Make slides** whenever you like, and everything said since the last time becomes slides. They can hold diagrams, tables, plots, formulas (LaTeX), pseudocode and code. Claude Code makes them, on your Claude subscription, with the model and reasoning effort you choose.
- **Faithful to the lecture.** Slides contain only what the lecturer said. Commands, numbers or code that can't be found in the transcript are marked "(?)".
- **Your language, your look.** Choose the slides' language, and pick a theme or have Claude Code design one from a description.

> **Ask before you record.** Many universities and countries require the lecturer's permission to record a lecture, even for personal study. Transcripts and audio stay on your Mac, but it is your responsibility to have that permission.

## Requirements

- macOS 13 or later. Apple Silicon is recommended (Metal).
- The speech model, Parakeet V3 0.6B. The setup page downloads it on first launch: 740 MB from [Hugging Face](https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf), licence CC-BY-4.0, checked against its SHA-256. A copy that [Handy](https://handy.computer) or the Hugging Face cache already has is used instead.
- For slides: [Claude Code](https://docs.claude.com/en/docs/claude-code), logged in once with `claude` in Terminal, and a Claude subscription (Pro or Max).

## Build

You need Rust 1.85 or later and the Xcode Command Line Tools.

```sh
./bundle.sh             # builds target/Transcribe Lecture.app
./bundle.sh --install   # also copies it to ~/Applications
```

The app is signed ad hoc, so it runs only on the Mac that built it. Set `SIGN_IDENTITY` to sign it with your Developer ID. If the linker can't read the newest macOS SDK, set `SDKROOT` to an older one.

## Using it

On first launch a short setup walks you through it: downloading the speech model, testing the microphone, checking Claude Code, choosing the course, languages and theme, and setting the slide style. You can open it again from **Settings → Open setup**.

1. Type a title and the course name if you like, then press **Start** (⌘↩). The microphone button beside Start and Continue holds the recording settings: the lecture's language, the microphone, and **Also save the audio**.
2. The transcript fills the left side as each sentence ends. It is saved continuously to `~/Documents/Lecture Transcripts/<date> <title>.md`, so a crash loses nothing. With **Also save the audio** ticked (the default), a `.wav` is kept next to it.
3. The right side is a card with two tabs, **Slides** and **Docs**. A bar floating at its bottom holds the tab's actions. On Slides, **Make slides** shows how much is new since the last time; click it whenever you want. A box opens for an optional suggestion ("more diagrams", "skip the recap"): it goes with that request only, and never overrides the rule that slides hold only what was said. Press Enter to send, or Esc to close. **⇧⌘↩** makes slides straight away. The chip in that box sets the Claude model and reasoning, the same ones as in Settings. While Claude works the bar shows its progress and **Stop**, which cancels the request and saves nothing. The slide count opens a list to jump to a slide. Click a slide's time range to jump to that part of the transcript.
4. **Present** opens the deck in your browser: press P to present, use ← → to move, and ⌘P to print a PDF.
5. Open a past lecture by clicking the lecture's name at the top left (or ⌘O): type to search by title or date, then pick one to read it, make slides for it, or press **Continue** to record more into it. The same menu shows the open lecture's file, its word count and when it was last saved, with **Show in Finder**, **New lecture** to name and record the next one, and **Rename…** to rename the open lecture: type the new name in the header and press Enter (Escape keeps the old one). Its transcript, audio, slides and Docs folder are renamed together, keeping the date they start with. Renaming waits until the recording is stopped and Claude has finished.

The deck is saved next to the transcript as `<lecture> · Slide.json` and `<lecture> · Slide.html`.

### Fixing the transcript

Click any line to edit it in place, as in a notes app. **Enter** splits the line at the cursor, and **Backspace** at the start of a line joins it to the line above. **↑** and **↓** at the start or end of a line move to the next one, and **Esc** undoes your typing in that line. Hover a line and click **+** next to its time to add a line below it, for something the model missed or a note of your own. A new line takes the time of the line above it. Empty a line to delete it. Dragging across lines still selects text to copy. Cut, copy, paste and undo work everywhere in the app (they're in the **Edit** menu).

This works during a recording too: new lines keep arriving at the end while you fix earlier ones. Each change is saved to the lecture's `.md` file when you leave the line, and the slides stay matched to their lines. Editing is off only while slides are being made.

### Checking the transcript with Claude

**Check transcript** (in the **⋯** menu) has Claude proofread the whole lecture for recognition errors: misheard terms, names, commands and acronyms ("un dip" → "UDP", "giostname" → "hostname"). It only restores what was said, and never rewrites, shortens or adds. The proposed fixes appear in the **Glossary** (from the **⋯** menu, which shows a dot while fixes wait) as before → after, and their lines get a wavy underline in the transcript. Accept or skip each one, or all at once. Nothing changes until you accept. A line you edited since the check is left alone.

Claude also proposes new glossary corrections for errors likely to come back. Accept them and they're fixed live in the course's next lectures. It uses the model from Settings, at least at High reasoning: a 30-minute lecture takes about three minutes, and **Stop** cancels it. It works during a recording too.

### Docs

The **Docs** tab holds your own documents about the lecture: notes, summaries, exercises. A lecture can have several. Click the doc's name in the bar at the bottom to switch doc, rename it, show it in Finder, or **Move to Trash** (you can put it back from Finder); **+** adds a blank one. Its name is the big title above it. Docs are saved as you type, and you can write in them while a lecture is recording.

- **Draft notes** (in the bar at the bottom) has Claude write study notes of the whole lecture into a new doc it names: sections in lecture order, each with a link to its moment, plus tables, formulas and diagrams where the professor gave them. As with slides, it only writes what was said. A box opens for an optional suggestion ("focus on the lab", "more examples"). It uses the model from Settings, at least at High reasoning, and takes a few minutes; **Stop** cancels it. It never overwrites a doc.
- **Writing** works like Notion. Type `# ` for a heading, `- ` or `1. ` for a list, `[] ` for a checklist, `> ` for a quote, ```` ``` ```` then Enter for code, `$$` then Enter for a formula, and `---` for a divider. Type `/` for the insert menu: headings, lists, table, code, formula, image, chart, divider, time link. Select text for bold, italic, strikethrough, code and link (⌘B, ⌘I, ⇧⌘X, ⌘E, ⌘K). Tab indents list items and moves between table cells. ⌘Z undoes.
- **Images**: paste or drag them in, or use `/Image`. **Formulas** are LaTeX, drawn with KaTeX; click one to edit it. **Charts** are a slide diagram (bars, plot, graph, sequence, network, sketch, timeline…) written as JSON; click one to edit it. **Time links** like `12:40` show that moment in the transcript.

Each doc is a plain Markdown file in a folder next to the lecture, `<lecture> · Docs/`, with images in `images/`, so Obsidian, Typora or any editor opens it. Formulas are `$…$` and `$$…$$`, charts are ```` ```chart ```` blocks, and time links are `[12:40](#t=00:12:40)`.

### Slide tools

Rebuild deck, Review cards and Undo last are in the **⋯** menu at the right end of the bar, with Glossary, Settings (⌘,), text size, Copy transcript and Open lectures folder. Glossary and Settings open over the card; **Done** or Esc closes them.

- **Rebuild deck** makes the whole deck again from the full lecture, more carefully. Like Review cards, it takes a suggestion first.
- **Review cards** adds 6–12 exam-style questions at the end. The answers stay hidden until you click them.
- **Undo last** removes what the last click added.
- **Glossary.** Speech models mishear technical terms, and Parakeet can't be given a word list. So each time slides are made, the model also reports misheard words ("HCB" → "DHCP") and the course's key terms. They're saved per course and fixed live in later transcripts. Remove a wrong entry with ×.
- **PDFs.** Add the course's PDFs, and the model searches them to spell terms right and to point you to pages ("Material: «Lab3.pdf» p. 4"). It never adds content from them. Text is extracted on the Mac with PDFKit. Scanned PDFs without a text layer aren't supported.

### Settings

- **Slide style.** Choose density (minimal, balanced, detailed), title style (statement, topic or question), how points are phrased, how long the notes are, and whether to favour diagrams or text. Add your own rules too. Claude makes three sample slides from about three minutes of your latest lecture (or a built-in sample lecture), so you see the result before a real lecture. Type what you'd change ("shorter titles", "always include the professor's examples"): the samples are made again and the request is saved as a rule. The style applies to every slide made afterwards.
- **Claude.** The model and reasoning effort for everything Claude makes: slides, docs, review cards and checks. Low is fast; Rebuild deck, Review cards, Draft notes and Check transcript always use at least High.
- **Language.** This sets the language of slides, review cards and drafted notes. By default it follows the lecture's language, so you can transcribe a lecture in one language and study it in another.
- **Slide theme.** Themes set the colours and fonts of the slides and the exported decks, in light and dark mode. The app window keeps its own look (warm paper, light or dark with macOS), whatever the theme.
  - A theme is a small CSS file in `~/Library/Application Support/Transcribe Lecture/themes/` that overrides the tokens in [`src/theme/default.css`](src/theme/default.css).
  - **New theme with Claude** (its own page in Settings): describe what you want, such as "navy and gold like my university's site example.edu, serif titles", and Claude Code writes the theme. It can read a website you name to take its colours.
  - From Terminal: `transcribe-lecture --make-theme "My university" "navy and gold, serif titles"`.
  - You can also edit a theme file by hand, or drop in your own.

## Privacy

- Audio and transcription stay on your Mac.
- When you make slides, draft notes or check a transcript, the transcript is sent to Anthropic through Claude Code. If the course has PDFs, the pages the model looks up are sent too.
- Claude Code runs in an empty temporary folder, without your own Claude Code settings, tools, MCP servers or `CLAUDE.md`. It uses your subscription, never an `ANTHROPIC_API_KEY` from the environment.

## Files

| What | Where |
|---|---|
| Transcripts, audio, decks, docs | `~/Documents/Lecture Transcripts/` |
| Settings, themes, speech model, glossaries, course PDFs (text) | `~/Library/Application Support/Transcribe Lecture/` |

## Command line

The binary is `target/release/transcribe-lecture` (or `Transcribe Lecture.app/Contents/MacOS/transcribe-lecture`):

```sh
transcribe-lecture --slides lecture.md [--course "Networks"] [--lang it] [--rebuild | --review] [--model opus] [--effort high] [--hint "more diagrams"]
transcribe-lecture --draft lecture.md [--course "Networks"] [--lang it] [--model opus] [--effort high] [--hint "focus on the lab"]
transcribe-lecture --check lecture.md [--course "Networks"] [--model sonnet]
transcribe-lecture --add-pdf notes.pdf --course "Networks"
transcribe-lecture --make-theme "Name" "description" [--model opus]
transcribe-lecture --download-model
transcribe-lecture --simulate audio.wav [--model path.gguf] [--lang it] [--speed 4] [--continue lecture.md]
```

`--simulate` transcribes a WAV file as if it were the microphone and writes the result to a temporary folder.

## Contributing

Read [AGENTS.md](AGENTS.md) for the code map, the file formats other code depends on, and how to test. It is written for AI coding agents and works just as well for people.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. KaTeX (`src/katex/`) is MIT-licensed; see [its license](src/katex/LICENSE). The Claude symbol (`src/claude-symbol.svg`, from [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Claude_AI_symbol.svg), CC0) is a trademark of Anthropic and is used only to show that slides are made by Claude. This project is not affiliated with or endorsed by Anthropic.
