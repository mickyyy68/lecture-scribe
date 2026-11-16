# Protocolli & Reti

Il design system delle slide del corso di protocolli e reti: minimale, squadrato, fatto di filetti e blocchi pieni, derivato dal tema dell'app (stessa palette oklch, stessi tre caratteri, raggio zero, nessuna ombra). Ogni slide deve sembrare un estratto di una RFC ben impaginata: poche parole, un diagramma esatto, niente decorazione.

## Contenuti e tono

- **Una idea per slide.** Il titolo è un'affermazione o il nome del meccanismo ("Three-way handshake", "Il TTL evita i loop"), mai "Introduzione" o "Panoramica".
- **Termini tecnici in inglese, spiegazione in italiano.** I nomi dei campi, dei flag e degli stati restano come nello standard (`SYN_RCVD`, `Window`, `ack`); il discorso intorno è in italiano.
- **Elenchi corti.** Massimo cinque voci, numerate `01 02 03` in mono; frasi complete senza punto e virgola.
- **Le fonti si citano.** RFC e paragrafo in `caption` o nella `Definition` (es. `RFC 9293 · §3.5`).
- **Numeri reali.** Indirizzi, porte e output vengono da catture vere, accorciate se serve; indirizzi d'esempio dai blocchi di documentazione quando non serve realismo (`192.0.2.0/24`, `2001:db8::/32`).

## Fondamenti visivi

**Formato.** 16:9 a base 1280×720. Margini `space-20` su tre lati, piè da 56px con nome del corso a sinistra e `12 / 40` a destra, barra di avanzamento `primary` alta `stroke-strong` sul bordo inferiore.

**Colore.** Fondo `background` (mai bianco puro), testo `foreground`, separazioni `border`. Il colore identitario è `primary` (ciano), usato come riempimento: il campo dell'header di cui si parla, il livello attivo, la barra di avanzamento. Nel tema chiaro primary ha solo 2:1 sul fondo, quindi non va mai usato per testo o linee sottili: per le frecce c'è `diagram-link`. La rampa `chart-1` → `chart-5` è riservata ai livelli della pila e va dall'alto (applicazione, chiaro) al basso (fisico, scuro). I colori di stato hanno un significato fisso: `success` = consegnato/ACK, `warning` = ritrasmissione/attenzione, `destructive` = perso/errore/RST, `info` = riferimento o comando.

**Tipografia.** Tre famiglie, ognuna con un ruolo: Plus Jakarta Sans (`sans`) per titoli e testo; JetBrains Mono (`mono`) per tutto ciò che è macchina (campi, flag, indirizzi, eyebrow, numerazione, codice); Newsreader (`serif`) corsivo solo per citazioni e definizioni normative. Titolo `slide-title` 48px, corpo `body` 22px: nulla sotto 15px in proiezione, eccetto gli offset dei bit (11px) che sono decorativi.

**Superfici.** Raggio `radius` = 0 ovunque; `radius-dot` solo per pallini. Nessuna ombra: la profondità si fa con un filetto `border` da `stroke-hairline` o con un blocco pieno. Nessun gradiente.

**Spaziatura.** Scala a passi di 4px (`space-1` … `space-20`). Gap `space-1` tra blocchi adiacenti di un diagramma (campi, livelli) così che si leggano come un'unica struttura; `space-12` tra colonne.

## Componenti

- **Slide**: la cornice di ogni slide.
- **LayerStack**: la pila TCP/IP o OSI.
- **PacketHeader**: il formato di un header a 32 bit per riga.
- **SequenceDiagram**: scambi di messaggi nel tempo.
- **Terminal**: comandi e catture.
- **Definition**: termine + definizione con fonte.
- **CompareTable**: confronto tra protocolli.

Sono classi CSS in `components/bundle.css` (prefisso `pr-`) sopra i token; nessun JavaScript. Le classi tipografiche generate (`.slide-title`, `.body`, `.eyebrow`, `.code`…) si combinano con quelle dei componenti.

## Iconografia

Nessuna icona e nessun logo: il sistema parla con tipografia e diagrammi. Host, router e switch nei diagrammi sono etichette mono dentro riquadri `card` con bordo `border`, non pittogrammi. Se servirà un logo del corso andrà aggiunto come asset.

## Esempi

- Slide di sezione: eyebrow `PARTE 2`, `cover-title` "Il livello di rete", nient'altro.
- Slide su un campo: `PacketHeader` a sinistra con `TTL` in `primary`, a destra tre righe numerate su cosa succede quando arriva a zero.
- Slide su un errore: `SequenceDiagram` con il segmento perso in `destructive` e la ritrasmissione annotata in `warning`.
