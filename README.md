# Lecture Scribe

Records lectures from your microphone and transcribes them live, on this Mac, with the Parakeet GGUF models Handy downloads. It uses the same `transcribe-cpp` engine as Handy and runs on the GPU (Metal).

- **Parakeet Unified 0.6B**: English only.
- **Parakeet V3 0.6B**: 25 languages, including Italian. Choose the language next to the model.

Transcripts are written to `~/Documents/Lecture Transcripts/<date> <title>.md` as each sentence is finished, so a crash loses nothing. A `.wav` of the audio is saved next to it if you tick "Also save audio".

Build and install: `./bundle.sh`, which installs to `~/Applications/Lecture Scribe.app`.
Headless test: `target/release/lecture-scribe --simulate file.wav [--model path.gguf] [--lang it] [--speed 4]`

## Slides (DeepSeek)

The window is split: the live transcript on the left, slides on the right. **Make slides** (⇧⌘↩) sends everything transcribed since the last click to DeepSeek (`deepseek-flash`, `reasoning_effort: low`). The new slides come back in the Protocolli & Reti design system (`src/ds/`, 1280×720, Slide · Definition · CompareTable · Terminal · PacketHeader · LayerStack · SequenceDiagram, plus a simple topology chain). Click a slide's time range to jump to that part of the transcript. **Present** opens the deck in the browser (P to present, ← → to move, ⌘P prints to PDF).

The deck is saved next to the transcript as `<lecture> · Slide.json` and `<lecture> · Slide.html`. The prompt is `src/slides_prompt.txt` and the rendering is `src/slides.js` + `src/slides.css`. The API key is in the Keychain (service "Lecture Scribe DeepSeek"). Past lectures can be opened from the footer menu to make slides for them.

**Glossary.** Parakeet can't be given a word list, so every Make slides also asks DeepSeek for the recognition mistakes it saw ("HCB" → "DHCP") and the course's key terms. They're saved per course in `~/Library/Application Support/Lecture Scribe/glossary/<course>.json`. The engine fixes those words live (whole words only) in later transcripts of that course, and DeepSeek gets the list with every request. The **Glossary** button shows the list; × removes a wrong entry.

**Course PDFs.** The **PDFs** button opens the course material (the same view as the glossary). Add PDF… extracts the text locally with macOS PDFKit into `~/Library/Application Support/Lecture Scribe/material/<course>/`. DeepSeek isn't sent the PDFs whole. It gets an index (file names and each page's first line, with running headers removed) in the cached system prompt, plus two tools: `search_material` (fuzzy, so "dnsmask" finds "dnsmasq") and `read_page`. It gets up to 3 lookup rounds per click. The material is only used to spell terms right, follow the professor's naming and cite pages. Slides still contain only what was said (`src/slides_prompt_material.txt`). Page references are kept only if DeepSeek actually saw that page, and they're appended to the notes as "Materiale: «file» p. N", using PDF page numbers. The fact check accepts a word missing from the transcript when the material spells it that way and the transcript has a close form. Scanned PDFs without a text layer are rejected (no OCR yet).

Headless: `target/release/lecture-scribe --slides file.md [--course "Protocolli e reti"] [--rebuild]`, `--add-pdf file.pdf --course "…"`

`notes/` holds the earlier browser-based notes companion (Appunti). The slide panel replaces it.
