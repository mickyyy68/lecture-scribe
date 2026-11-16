#!/usr/bin/env python3
"""Appunti: turns the live Lecture Scribe transcript into notes with DeepSeek, on demand.

Reads ~/Documents/Lecture Transcripts/<lecture>.md (Lecture Scribe rewrites it every second)
and, when you click "Riscrivi appunti", sends the sentences not yet covered to DeepSeek.
Notes are saved next to the transcript as "<lecture> · Appunti.{json,md,html}".

Standard library only. Run: python3 appunti.py [--port 8765] [--no-open]
"""

import html
import os
import json
import re
import subprocess
import sys
import threading
import urllib.error
import urllib.request
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

HERE = Path(__file__).resolve().parent
TRANSCRIPTS = Path(os.environ.get("APPUNTI_DIR") or Path.home() / "Documents" / "Lecture Transcripts")
SETTINGS = Path.home() / "Library" / "Application Support" / "Lecture Scribe" / "appunti.json"
SUFFIX = " · Appunti"
API_URL = "https://api.deepseek.com/chat/completions"
MODEL = "deepseek-flash"  # DeepSeek V4.1 Flash
EFFORT = "low"
KEYCHAIN = ("Lecture Scribe DeepSeek", "deepseek")

LINE = re.compile(r"^\*\*\[(\d\d:\d\d:\d\d)\]\*\* (.+?)\n\n", re.M)
locks: dict[str, threading.Lock] = {}
busy: set[str] = set()


# ---------- settings, key ----------

def load_settings() -> dict:
    try:
        return json.loads(SETTINGS.read_text())
    except Exception:
        return {}


def save_settings(s: dict) -> None:
    SETTINGS.parent.mkdir(parents=True, exist_ok=True)
    SETTINGS.write_text(json.dumps(s, ensure_ascii=False, indent=2))


def api_key() -> str:
    if os.environ.get("DEEPSEEK_API_KEY"):
        return os.environ["DEEPSEEK_API_KEY"]
    r = subprocess.run(
        ["security", "find-generic-password", "-s", KEYCHAIN[0], "-a", KEYCHAIN[1], "-w"],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        raise RuntimeError("Chiave DeepSeek non trovata nel Portachiavi (servizio «Lecture Scribe DeepSeek»).")
    return r.stdout.strip()


# ---------- files ----------

def transcripts() -> list[Path]:
    if not TRANSCRIPTS.is_dir():
        return []
    fs = [p for p in TRANSCRIPTS.glob("*.md") if not p.stem.endswith(SUFFIX)]
    return sorted(fs, key=lambda p: p.stat().st_mtime, reverse=True)


def resolve(name: str | None) -> Path | None:
    fs = transcripts()
    if name:
        for p in fs:
            if p.name == name:
                return p
    return fs[0] if fs else None


def notes_path(t: Path, ext: str) -> Path:
    return t.with_name(t.stem + SUFFIX + ext)


def read_transcript(t: Path) -> tuple[str, list[tuple[str, str]]]:
    """Title and finished lines. The live partial line ends with a single newline, so it is skipped."""
    text = t.read_text(encoding="utf-8", errors="replace")
    m = re.match(r"# (.+)", text)
    return (m.group(1).strip() if m else t.stem), LINE.findall(text)


def load_notes(t: Path) -> dict:
    try:
        return json.loads(notes_path(t, ".json").read_text())
    except Exception:
        return {"batches": []}


def sections_of(notes: dict) -> list[dict]:
    out: list[dict] = []
    for b in notes["batches"]:
        if b.get("replace_last") and out:
            out.pop()
        out.extend(b["sections"])
    return out


def write_atomic(p: Path, s: str) -> None:
    tmp = p.with_name(p.name + ".tmp")
    tmp.write_text(s, encoding="utf-8")
    tmp.replace(p)


def save_notes(t: Path, notes: dict, title: str) -> None:
    write_atomic(notes_path(t, ".json"), json.dumps(notes, ensure_ascii=False, indent=1))
    secs = sections_of(notes)
    write_atomic(notes_path(t, ".md"), to_markdown(title, secs))
    write_atomic(notes_path(t, ".html"), to_html(title, t.stem, secs))


# ---------- export ----------

def to_markdown(title: str, secs: list[dict]) -> str:
    out = [f"# {title} · Appunti\n"]
    for s in secs:
        rng = " – ".join(x for x in (s.get("from"), s.get("to")) if x)
        out.append(f"\n## {s.get('title', '')}\n")
        if rng:
            out.append(f"_{rng}_\n")
        if s.get("subtitle"):
            out.append(f"\n{s['subtitle']}\n")
        for b in s.get("blocks", []):
            ty = b.get("type")
            if ty == "list":
                out.append("\n" + "\n".join(f"{i + 1}. {x}" for i, x in enumerate(b.get("items", []))) + "\n")
            elif ty == "def":
                src = f" — `{b['src']}`" if b.get("src") else ""
                out.append(f"\n> **{b.get('tag', 'Definizione').upper()} · {b.get('term', '')}**  \n> _{b.get('text', '')}_{src}\n")
            elif ty == "table":
                cell = lambda c: c.get("yes", c.get("no", c.get("text", ""))) if isinstance(c, dict) else str(c)
                head = b.get("head", [])
                out.append("\n| " + " | ".join(head) + " |\n|" + "---|" * len(head) + "\n")
                out.extend("| " + " | ".join(cell(c) for c in r) + " |\n" for r in b.get("rows", []))
            elif ty == "term":
                out.append("\n```\n" + "\n".join(b.get("lines", [])) + "\n```\n")
            elif ty == "header":
                rows = b.get("rows", [])
                out.append("\n" + "\n".join("| " + " | ".join(f"{f.get('name')} ({f.get('bits', '…')})" for f in r) + " |" for r in rows) + "\n")
            elif ty == "stack":
                out.append("\n" + "\n".join(f"- {l.get('k', '')} {l.get('name', '')} {l.get('p', '')}".rstrip() for l in b.get("layers", [])) + "\n")
            elif ty == "seq":
                hosts = b.get("hosts", [])
                h = lambda i: hosts[i] if isinstance(i, int) and 0 <= i < len(hosts) else "?"
                out.append("\n" + "\n".join(
                    f"{n + 1}. {h(m.get('from'))} → {h(m.get('to'))}: `{m.get('label', '')}`"
                    + (f" ({m['meta']})" if m.get("meta") else "") + (" — **perso**" if m.get("lost") else "")
                    + (f" — {m['note']}" if m.get("note") else "")
                    for n, m in enumerate(b.get("msgs", []))) + "\n")
            elif ty == "net":
                nodes, links = b.get("nodes", []), b.get("links", [])
                parts = []
                for i, nd in enumerate(nodes):
                    parts.append(f"[{nd.get('name', '')}{' · ' + nd['sub'] if nd.get('sub') else ''}]")
                    if i < len(nodes) - 1:
                        parts.append(f" —{links[i] if i < len(links) else ''}— ")
                out.append(("\n_" + b["caption"] + "_" if b.get("caption") else "") + "\n`" + "".join(parts) + "`\n")
            elif ty == "quote":
                out.append(f"\n> _{b.get('text', '')}_\n")
            elif ty == "check":
                out.append(f"\n> ⚠︎ **Da verificare:** {b.get('text', '')}\n")
            else:
                out.append(f"\n{b.get('text', '')}\n")
    return "".join(out)


def to_html(title: str, stem: str, secs: list[dict]) -> str:
    css = "".join((HERE / f).read_text() for f in ("ds/tokens.css", "ds/bundle.css", "notes.css"))
    # @import must come first in a stylesheet: hoist the font import.
    imp = re.compile(r"@import\s+url\([^)]*\)[^;]*;")
    imports = "\n".join(imp.findall(css))
    css = imports + "\n" + imp.sub("", css)
    data = json.dumps(secs, ensure_ascii=False).replace("</", "<\\/")
    return f"""<!doctype html>
<html lang="it" data-theme="light"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(title)} · Appunti</title><style>{css}</style></head>
<body><main class="n-doc"><header class="n-doc-head"><p class="pr-eyebrow eyebrow">Appunti</p><h1>{html.escape(title)}</h1>
<p class="n-meta">{html.escape(stem)}</p></header><div id="notes"></div></main>
<script>{(HERE / "render.js").read_text()}</script>
<script>document.getElementById("notes").innerHTML = renderNotes({data});</script>
</body></html>"""


# ---------- DeepSeek ----------

SYSTEM = """Sei l'assistente che scrive gli appunti di una lezione universitaria del corso «{course}», mentre la lezione è in corso.
Ricevi un estratto della trascrizione automatica (riconoscimento vocale, in italiano): contiene errori di riconoscimento, ripetizioni, frasi spezzate, battute e chiacchiere.

COMPITO: trasformare l'estratto NUOVO in appunti chiari e ordinati.

FEDELTÀ
- Riporta solo ciò che il docente ha detto. Non aggiungere contenuti, esempi, numeri o fonti che non compaiono nella trascrizione.
- Correggi gli errori di riconoscimento usando il contesto del corso (es. «HCB», «l'FCP» → DHCP se il discorso riguarda l'assegnazione degli indirizzi). Se la correzione è sicura scrivi solo il termine corretto; se è probabile ma non certa, scrivi il termine seguito da «(?)».
- Se un passaggio importante è incomprensibile, aggiungi un blocco "check" che dice cosa ricontrollare (con il minuto).
- Ometti saluti, logistica e chiacchiere, ma conserva avvisi utili (esame, scadenze, materiale, esercizi) in un blocco "check" o "p" che inizia con **Avviso**.

STILE (design system «Protocolli & Reti»)
- Una idea per sezione. Il titolo è un'affermazione o il nome del meccanismo («Il TTL evita i loop», «DHCP: assegnazione dinamica»), mai «Introduzione» o «Panoramica».
- Termini tecnici in inglese come nello standard (nomi di campi, flag, stati, comandi: `SYN_RCVD`, `Window`); spiegazione in italiano.
- Elenchi corti: al massimo 5 voci, frasi complete, senza punto e virgola.
- Testo asciutto, niente enfasi né decorazione. Usa `backtick` per indirizzi, porte, comandi, campi, e **grassetto** con parsimonia.

FORMA: ELENCHI E DIAGRAMMI PRIMA DEI PARAGRAFI
- Gli appunti si leggono a colpo d'occhio: ogni sezione contiene almeno un blocco strutturato (list, def, table, term, header, stack, seq, net).
- Al massimo un blocco "p" per sezione, di una o due frasi, solo per introdurre. Tutto il resto va in elenchi.
- Scegli il blocco in base a ciò che il docente descrive:
  · procedura, passaggi, cause/conseguenze, proprietà → "list" (più elenchi se servono più di 5 voci)
  · scambio di messaggi tra host (DHCP, ARP, handshake, richiesta/risposta) → "seq"
  · rete del laboratorio, collegamenti tra host, router, switch, interfacce, indirizzi → "net"
  · comandi, file di configurazione, output → "term"
  · confronto tra due o più alternative → "table"
  · campi di un header → "header"; livelli → "stack"; termine definito → "def"
- I diagrammi contengono solo elementi detti dal docente (host, interfacce, indirizzi, messaggi). Se un elemento è dedotto dal contesto ma non certo, aggiungi «(?)» alla sua etichetta.

FORMATO: rispondi SOLO con un oggetto JSON:
{{"replace_last": <sezione o null>, "sections": [<sezione>, ...]}}
sezione = {{"title": str, "subtitle": str opzionale, "from": "hh:mm:ss", "to": "hh:mm:ss", "blocks": [<blocco>, ...]}}
"from"/"to" sono i minuti della trascrizione coperti dalla sezione.
Blocchi ammessi:
- {{"type":"p","text":str}}
- {{"type":"list","items":[str,...]}}  (max 5)
- {{"type":"def","tag":"Definizione"|"Teorema"|"Attenzione"|"RFC","term":str,"text":str,"src":str opzionale}}  — per definizioni date dal docente; "src" solo se il docente cita la fonte (es. "RFC 2131 · §3")
- {{"type":"table","head":[str,...],"rows":[[cella,...],...],"focus":indice riga opzionale}}  — per confronti; cella = str oppure {{"yes":str}} / {{"no":str}} per sì/no; prima colonna = proprietà; max 6 righe
- {{"type":"term","host":str,"tool":str,"lines":[str,...],"hl":[indici]}}  — solo comandi/configurazioni effettivamente dettati o mostrati; il comando inizia con "$ " (shell) o con il prompt del dispositivo (es. "Router# ")
- {{"type":"header","caption":str,"rows":[[{{"name":str,"bits":int,"on":bool,"opt":bool}},...],...]}}  — formato di un header, righe da 32 bit; "on" per i campi di cui si parla (max 3)
- {{"type":"stack","layers":[{{"k":"L5","name":"Applicazione","p":"HTTP"}},...],"focus":indice opzionale}}  — pila dei livelli dall'alto in basso
- {{"type":"seq","hosts":[str,...],"msgs":[{{"from":i,"to":j,"label":str,"meta":str opzionale,"lost":bool opzionale,"note":str opzionale}},...]}}  — diagramma di sequenza: 2 o 3 host, messaggi in ordine di tempo (max 8); "from"/"to" sono indici in "hosts"; "meta" per dettagli brevi (es. "0.0.0.0:68 → 255.255.255.255:67"); "lost" per un messaggio perso; "note" per ritrasmissioni o avvertenze
- {{"type":"net","caption":str opzionale,"nodes":[{{"name":str,"sub":str opzionale}},...],"links":[str,...]}}  — topologia come catena da sinistra a destra (max 5 nodi): "sub" = ruolo o indirizzo (es. "router", "10.0.0.1"); links[i] = etichetta del collegamento tra nodes[i] e nodes[i+1] (interfacce, subnet); per topologie ramificate usa più blocchi "net"
- {{"type":"quote","text":str}}  — solo per una frase del docente da riportare testualmente
- {{"type":"check","text":str}}

CONTINUITÀ
- Ti vengono dati i titoli delle sezioni già scritte e l'ultima sezione per intero. Non ripetere contenuti già presenti.
- Se l'estratto nuovo prosegue l'argomento dell'ultima sezione, metti in "replace_last" l'ultima sezione riscritta e ampliata (stesso "from", nuovo "to"), poi eventuali sezioni nuove in "sections". Altrimenti "replace_last": null.
- Se l'estratto non contiene nulla di utile, rispondi {{"replace_last": null, "sections": []}}."""


def call_deepseek(course: str, prev: list[dict], context: list[tuple[str, str]], new: list[tuple[str, str]]) -> dict:
    user = []
    if prev:
        user.append("TITOLI GIÀ SCRITTI:\n" + "\n".join(f"- {s.get('title')}" for s in prev))
        user.append("ULTIMA SEZIONE (JSON):\n" + json.dumps(prev[-1], ensure_ascii=False))
    if context:
        user.append("TRASCRIZIONE GIÀ COPERTA (solo contesto):\n" + "\n".join(f"[{t}] {x}" for t, x in context))
    user.append("TRASCRIZIONE NUOVA:\n" + "\n".join(f"[{t}] {x}" for t, x in new))
    body = {
        "model": MODEL,
        "messages": [
            {"role": "system", "content": SYSTEM.format(course=course or "universitario")},
            {"role": "user", "content": "\n\n".join(user)},
        ],
        "thinking": {"type": "enabled"},
        "reasoning_effort": EFFORT,
        "response_format": {"type": "json_object"},
        "max_tokens": 16000,
        "stream": False,
    }
    req = urllib.request.Request(
        API_URL,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {api_key()}"},
    )
    try:
        with urllib.request.urlopen(req, timeout=240) as r:
            resp = json.load(r)
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"DeepSeek {e.code}: {e.read().decode(errors='replace')[:300]}")
    except urllib.error.URLError as e:
        raise RuntimeError(f"DeepSeek non raggiungibile: {e.reason}")
    content = resp["choices"][0]["message"].get("content") or ""
    m = re.search(r"\{.*\}", content, re.S)
    if not m:
        raise RuntimeError("Risposta di DeepSeek senza JSON.")
    out = json.loads(m.group(0))
    out["sections"] = [s for s in out.get("sections") or [] if isinstance(s, dict)]
    if not isinstance(out.get("replace_last"), dict):
        out["replace_last"] = None
    out["usage"] = resp.get("usage", {})
    return out


# ---------- actions ----------

def state(name: str | None) -> dict:
    t = resolve(name)
    s = load_settings()
    base = {"files": [p.name for p in transcripts()], "course": s.get("course", "Protocolli e reti"), "dir": str(TRANSCRIPTS)}
    if not t:
        return base | {"file": None}
    title, lines = read_transcript(t)
    notes = load_notes(t)
    done = notes["batches"][-1]["to_line"] if notes["batches"] else 0
    return base | {
        "file": t.name,
        "title": title,
        "lines": len(lines),
        "done": done,
        "lastT": lines[-1][0] if lines else None,
        "sections": sections_of(notes),
        "busy": t.name in busy,
        "html": str(notes_path(t, ".html")),
    }


def rewrite(name: str | None, course: str) -> dict:
    t = resolve(name)
    if not t:
        raise RuntimeError("Nessuna trascrizione trovata.")
    lock = locks.setdefault(t.name, threading.Lock())
    if not lock.acquire(blocking=False):
        raise RuntimeError("Sto già riscrivendo questi appunti.")
    busy.add(t.name)
    try:
        s = load_settings()
        s["course"] = course
        save_settings(s)
        title, lines = read_transcript(t)
        notes = load_notes(t)
        done = notes["batches"][-1]["to_line"] if notes["batches"] else 0
        new = lines[done:]
        if not new:
            raise RuntimeError("Nessuna frase nuova dall'ultima riscrittura.")
        prev = sections_of(notes)
        out = call_deepseek(course, prev, lines[max(0, done - 4):done], new)
        secs = out["sections"]
        replace = out["replace_last"] is not None and bool(prev)
        if replace:
            secs = [out["replace_last"]] + secs
        notes["batches"].append({"from_line": done, "to_line": len(lines), "replace_last": replace, "sections": secs, "usage": out["usage"]})
        save_notes(t, notes, title)
        return state(t.name) | {"added": len(secs), "replaced": replace}
    finally:
        busy.discard(t.name)
        lock.release()


def undo(name: str | None) -> dict:
    t = resolve(name)
    if not t:
        raise RuntimeError("Nessuna trascrizione trovata.")
    notes = load_notes(t)
    if notes["batches"]:
        notes["batches"].pop()
        save_notes(t, notes, read_transcript(t)[0])
    return state(t.name)


# ---------- HTTP ----------

STATIC = {"/": ("page.html", "text/html"), "/render.js": ("render.js", "text/javascript"), "/notes.css": ("notes.css", "text/css"),
          "/ds/tokens.css": ("ds/tokens.css", "text/css"), "/ds/bundle.css": ("ds/bundle.css", "text/css")}


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def ok_host(self) -> bool:  # blocks DNS rebinding
        return self.headers.get("Host", "").split(":")[0] in ("127.0.0.1", "localhost")

    def send(self, code: int, body: bytes, ctype: str = "application/json") -> None:
        self.send_response(code)
        self.send_header("Content-Type", ctype + "; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if not self.ok_host():
            return self.send(403, b"{}")
        u = urlparse(self.path)
        if u.path in STATIC:
            f, ct = STATIC[u.path]
            return self.send(200, (HERE / f).read_bytes(), ct)
        if u.path == "/api/state":
            q = parse_qs(u.query)
            return self.send(200, json.dumps(state(q.get("file", [None])[0]), ensure_ascii=False).encode())
        self.send(404, b"{}")

    def do_POST(self):
        # Custom header: a web page on another origin can't send it without a CORS preflight, which we never accept.
        if not self.ok_host() or self.headers.get("X-Appunti") != "1":
            return self.send(403, b"{}")
        n = int(self.headers.get("Content-Length") or 0)
        try:
            msg = json.loads(self.rfile.read(n) or b"{}")
        except ValueError:
            msg = {}
        try:
            p = urlparse(self.path).path
            if p == "/api/rewrite":
                r = rewrite(msg.get("file"), (msg.get("course") or "").strip())
            elif p == "/api/undo":
                r = undo(msg.get("file"))
            elif p == "/api/open":
                t = resolve(msg.get("file"))
                target = notes_path(t, ".html") if t and msg.get("what") == "html" and notes_path(t, ".html").exists() else TRANSCRIPTS
                subprocess.Popen(["open", str(target)])
                r = {}
            else:
                return self.send(404, b"{}")
            self.send(200, json.dumps(r, ensure_ascii=False).encode())
        except Exception as e:
            self.send(500, json.dumps({"error": str(e)}, ensure_ascii=False).encode())


def main() -> None:
    port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 8765
    url = f"http://127.0.0.1:{port}/"
    try:
        srv = ThreadingHTTPServer(("127.0.0.1", port), H)
    except OSError:
        # Already running: just show it.
        if "--no-open" not in sys.argv:
            webbrowser.open(url)
        return
    if "--no-open" not in sys.argv:
        webbrowser.open(url)
    print(f"Appunti on {url}", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
