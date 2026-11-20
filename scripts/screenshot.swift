// The README screenshot (scripts/screenshot.sh runs this): the app's real page in a real macOS
// window the size of the app's, with screenshot.js staging one lecture mid-recording, captured
// with its frame and shadow by `screencapture`.
//
//   screenshot <page.html> <screenshot.js> <shot json> <out.png> <width> <height>
//
// macOS draws a window as active (coloured buttons, full shadow) only while its app is, so it
// takes focus for the couple of seconds of the capture, then gives it back.
import AppKit
import WebKit

let a = CommandLine.arguments
guard a.count == 7, let w = Double(a[5]), let h = Double(a[6]) else {
    FileHandle.standardError.write("usage: screenshot <page.html> <screenshot.js> <shot json> <out.png> <width> <height>\n".data(using: .utf8)!)
    exit(2)
}
let page = URL(fileURLWithPath: a[1])
let script = try! String(contentsOfFile: a[2], encoding: .utf8)
let shot = try! String(contentsOfFile: a[3], encoding: .utf8)
let out = a[4]

let app = NSApplication.shared
app.setActivationPolicy(.regular)

let rect = NSRect(x: 80, y: 80, width: w, height: h)
let window = NSWindow(contentRect: rect, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
window.title = "Transcribe Lecture"
window.appearance = NSAppearance(named: .aqua)
// No full-screen button (the green one zooms instead, and looks the same): tiling window
// managers such as AeroSpace leave such windows floating, at this size.
window.collectionBehavior = [.fullScreenNone]

let cfg = WKWebViewConfiguration()
cfg.userContentController.addUserScript(WKUserScript(source: "window.SHOT = \(shot);\n\(script)", injectionTime: .atDocumentStart, forMainFrameOnly: true))
let web = WKWebView(frame: rect, configuration: cfg)
window.contentView = web
let before = NSWorkspace.shared.frontmostApplication
window.makeKeyAndOrderFront(nil)
web.loadFileURL(page, allowingReadAccessTo: page.deletingLastPathComponent())

func fail(_ msg: String) -> Never {
    before?.activate()
    FileHandle.standardError.write("\(msg)\n".data(using: .utf8)!)
    exit(1)
}

Task { @MainActor in
    app.activate(ignoringOtherApps: true) // only works once the app has launched, i.e. in here
    window.makeKeyAndOrderFront(nil)
    try? await Task.sleep(nanoseconds: 300_000_000)
    window.setFrame(window.frameRect(forContentRect: rect), display: true) // undo any tiling on activation
    for _ in 0..<100 {
        try? await Task.sleep(nanoseconds: 100_000_000)
        if (try? await web.evaluateJavaScript("!!window.SHOT?.ready")) as? Bool == true { break }
    }
    guard (try? await web.evaluateJavaScript("!!window.SHOT?.ready")) as? Bool == true else { fail("the page never got ready") }
    try? await Task.sleep(nanoseconds: 1_500_000_000) // KaTeX, fonts, the timer's next tick
    if window.contentLayoutRect.size != rect.size { fail("the window is \(window.contentLayoutRect.size), not \(rect.size)") }
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    p.arguments = ["-x", "-l\(window.windowNumber)", out]
    try! p.run()
    p.waitUntilExit()
    before?.activate()
    if p.terminationStatus != 0 || !FileManager.default.fileExists(atPath: out) { fail("screencapture failed (does this terminal have Screen Recording permission?)") }
    exit(0)
}
app.run()
