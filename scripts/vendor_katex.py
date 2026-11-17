"""Builds src/katex/ from an unpacked KaTeX npm package: the minified JS, and the CSS with
only the woff2 fonts, inlined as data URIs (the app and the exported deck work offline).

    python3 -I scripts/vendor_katex.py /path/to/package
"""
import base64, pathlib, re, shutil, sys

pkg = pathlib.Path(sys.argv[1]) / "dist"
out = pathlib.Path(__file__).resolve().parent.parent / "src" / "katex"
out.mkdir(exist_ok=True)
css = (pkg / "katex.min.css").read_text()

def font(m):
    srcs = m.group(1)
    w2 = re.search(r"url\((fonts/[^)]+\.woff2)\)", srcs)
    data = base64.b64encode((pkg / w2.group(1)).read_bytes()).decode()
    return f"src:url(data:font/woff2;base64,{data}) format(\"woff2\")"

css = re.sub(r"src:(url\([^;}]*)", font, css)
(out / "katex.css").write_text(css)
shutil.copy(pkg / "katex.min.js", out / "katex.min.js")
shutil.copy(pkg.parent / "LICENSE", out / "LICENSE")
print("ok", len(css))
