// UI test runner: loads the app's real page (`transcribe-lecture --dump-ui`) in WebKit, the
// engine the app's window uses, with `prelude.js` standing in for the Rust side. Tests drive it
// with real AppKit mouse and keyboard events (T.click, T.type, T.key), so a click goes through
// the same path as the user's: mouse down and up must land on the same element.
//
//   runner <page.html> <prelude.js> <shots dir> <test.js>... [--only substring]
//
// Each test gets a freshly loaded page. Exit status 1 if any test fails.
import AppKit
import WebKit

final class Runner: NSObject, WKNavigationDelegate, WKScriptMessageHandlerWithReply {
    let page: URL, prelude: String, shots: URL
    var window: NSWindow!
    var web: WKWebView!
    var onLoad: (() -> Void)?

    init(page: URL, prelude: String, shots: URL) {
        self.page = page; self.prelude = prelude; self.shots = shots
    }

    func makeView() {
        let cfg = WKWebViewConfiguration()
        cfg.userContentController.addUserScript(WKUserScript(source: prelude, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        cfg.userContentController.addScriptMessageHandler(self, contentWorld: .page, name: "native")
        let rect = NSRect(x: 0, y: 0, width: 1280, height: 800)
        web = WKWebView(frame: rect, configuration: cfg)
        web.navigationDelegate = self
        if window == nil {
            // On screen (WebKit ignores mouse events in an off-screen window) but invisible, behind
            // everything and transparent to real clicks; never activated, so the tests don't take
            // focus from whatever you're doing.
            window = NSWindow(contentRect: rect, styleMask: [.borderless], backing: .buffered, defer: false)
            window.alphaValue = 0
            window.ignoresMouseEvents = true
            window.level = NSWindow.Level(rawValue: NSWindow.Level.normal.rawValue - 1)
        }
        window.contentView = web
        window.orderFrontRegardless()
        window.makeFirstResponder(web)
    }

    func load() async {
        makeView()
        await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in
            onLoad = { c.resume() }
            web.loadFileURL(page, allowingReadAccessTo: page.deletingLastPathComponent())
        }
    }

    func webView(_ w: WKWebView, didFinish n: WKNavigation!) { onLoad?(); onLoad = nil }

    func run(_ js: String) async -> String {
        do {
            let r = try await web.callAsyncJavaScript(js, arguments: [:], in: nil, contentWorld: .page)
            return (r as? String) ?? "null"
        } catch {
            return "{\"ok\":false,\"error\":\(String(reflecting: "\(error)"))}"
        }
    }

    // MARK: native input, asked for by the page (prelude.js `native`)
    func userContentController(_ u: WKUserContentController, didReceive m: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard let d = m.body as? [String: Any], let op = d["op"] as? String else { return replyHandler(nil, "bad message") }
        switch op {
        case "down", "up", "click":
            let x = d["x"] as? Double ?? 0, y = d["y"] as? Double ?? 0
            let p = NSPoint(x: x, y: Double(web.bounds.height) - y) // page coordinates are top-down
            if op != "up" { mouse(.leftMouseDown, p) }
            if op != "down" { mouse(.leftMouseUp, p) }
            replyHandler(true, nil)
        case "type":
            for ch in (d["text"] as? String ?? "") { key(String(ch), code: 0, mods: []) }
            replyHandler(true, nil)
        case "key":
            let name = d["key"] as? String ?? ""
            let mods = NSEvent.ModifierFlags((d["mods"] as? [String] ?? []).map {
                ["cmd": NSEvent.ModifierFlags.command, "shift": .shift, "alt": .option, "ctrl": .control][$0] ?? []
            })
            let keys: [String: (String, UInt16)] = [
                "Enter": ("\r", 36), "Backspace": ("\u{7f}", 51), "Escape": ("\u{1b}", 53), "Tab": ("\t", 48),
                "ArrowLeft": (String(UnicodeScalar(NSLeftArrowFunctionKey)!), 123), "ArrowRight": (String(UnicodeScalar(NSRightArrowFunctionKey)!), 124),
                "ArrowDown": (String(UnicodeScalar(NSDownArrowFunctionKey)!), 125), "ArrowUp": (String(UnicodeScalar(NSUpArrowFunctionKey)!), 126),
            ]
            guard let (chars, code) = keys[name] ?? (name.count == 1 ? (name, 0) : nil) else { return replyHandler(nil, "unknown key \(name)") }
            key(chars, code: code, mods: mods)
            replyHandler(true, nil)
        case "shot":
            let file = shots.appendingPathComponent("\(d["name"] as? String ?? "shot").png")
            web.takeSnapshot(with: nil) { img, err in
                guard let img, let tiff = img.tiffRepresentation, let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else {
                    return replyHandler(nil, "snapshot failed: \(String(describing: err))")
                }
                try? png.write(to: file)
                replyHandler(file.path, nil)
            }
        default:
            replyHandler(nil, "unknown op \(op)")
        }
    }

    func mouse(_ type: NSEvent.EventType, _ p: NSPoint) {
        let e = NSEvent.mouseEvent(with: type, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                   windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        window.sendEvent(e)
    }

    func key(_ chars: String, code: UInt16, mods: NSEvent.ModifierFlags) {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let e = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: mods, timestamp: ProcessInfo.processInfo.systemUptime,
                                     windowNumber: window.windowNumber, context: nil, characters: chars, charactersIgnoringModifiers: chars,
                                     isARepeat: false, keyCode: code)!
            window.sendEvent(e)
        }
    }
}

struct Result: Decodable { let ok: Bool; let error: String? }

@MainActor func main() async {
    var args = Array(CommandLine.arguments.dropFirst())
    var only: String?
    if let i = args.firstIndex(of: "--only"), i + 1 < args.count { only = args[i + 1]; args.removeSubrange(i...i + 1) }
    guard args.count >= 4 else {
        FileHandle.standardError.write("usage: runner page.html prelude.js shotsdir test.js... [--only name]\n".data(using: .utf8)!)
        exit(2)
    }
    let page = URL(fileURLWithPath: args[0])
    let prelude = try! String(contentsOfFile: args[1], encoding: .utf8)
    let shots = URL(fileURLWithPath: args[2])
    try? FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
    let r = Runner(page: page, prelude: prelude, shots: shots)
    var passed = 0, failed: [String] = []
    for file in args[3...] {
        let src = try! String(contentsOfFile: file, encoding: .utf8)
        let base = (file as NSString).lastPathComponent
        await r.load()
        let names = (try? JSONDecoder().decode([String].self, from: Data(await r.run(src + "\nreturn await T.__run(-1);").utf8))) ?? []
        if names.isEmpty { failed.append("\(base): no tests found (or the file doesn't parse)"); print("✗ \(base): no tests"); continue }
        for (i, name) in names.enumerated() where only.map({ name.contains($0) || base.contains($0) }) ?? true {
            await r.load()
            let out = await r.run(src + "\nreturn await T.__run(\(i));")
            let res = (try? JSONDecoder().decode(Result.self, from: Data(out.utf8))) ?? Result(ok: false, error: out)
            if res.ok { passed += 1; print("✓ \(base) › \(name)") } else {
                failed.append("\(base) › \(name)\n    \(res.error ?? "")")
                print("✗ \(base) › \(name)\n    \((res.error ?? "").replacingOccurrences(of: "\n", with: "\n    "))")
            }
        }
    }
    print("\nUI tests: \(passed) passed, \(failed.count) failed")
    exit(failed.isEmpty ? 0 : 1)
}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
Task { await main() }
app.run()
