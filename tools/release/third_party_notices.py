"""Generates THIRD-PARTY-NOTICES.txt for the Windows release.

Lists every third-party component that ends up in what users download — Rust crates linked into
nexdir.exe (runtime dependencies for x86_64-pc-windows-msvc; build scripts and proc macros are
excluded because they only run at build time) and npm packages bundled into the frontend — with
license, source and the license texts shipped by each package.

Usage (from the repository root, after `npm ci`, with cargo on PATH):
    python tools/release/third_party_notices.py            # writes THIRD-PARTY-NOTICES.txt
    python tools/release/third_party_notices.py --check    # fails if the file is out of date
"""
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "THIRD-PARTY-NOTICES.txt")
TARGET = "x86_64-pc-windows-msvc"
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")

# Standard MIT text, used for packages that declare MIT but ship no license file.
MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""


def read(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read().replace("\r\n", "\n").strip() + "\n"


def license_files(directory):
    if not os.path.isdir(directory):
        return []
    return sorted(f for f in os.listdir(directory) if f.upper().startswith(LICENSE_PREFIXES) and os.path.isfile(os.path.join(directory, f)))


def rust_packages():
    meta = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET,
             "--manifest-path", os.path.join(ROOT, "src-tauri", "Cargo.toml")],
            text=True, encoding="utf-8",
        )
    )
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            if not any(k["kind"] is None for k in dep["dep_kinds"]):
                continue  # dev- and build-dependencies are not in the binary
            if any("proc-macro" in t["kind"] for t in pkgs[dep["pkg"]]["targets"]):
                continue  # runs inside the compiler only
            stack.append(dep["pkg"])
    seen.discard(root)
    out = []
    for pid in seen:
        p = pkgs[pid]
        d = os.path.dirname(p["manifest_path"])
        out.append({
            "name": p["name"], "version": p["version"], "license": p.get("license") or "see license file",
            "source": p.get("repository") or f"https://crates.io/crates/{p['name']}",
            "authors": p.get("authors") or [], "dir": d, "files": license_files(d),
        })
    return sorted(out, key=lambda x: (x["name"], x["version"]))


def npm_packages():
    with open(os.path.join(ROOT, "package-lock.json"), encoding="utf-8") as f:
        lock = json.load(f)
    out = []
    for path, info in lock["packages"].items():
        if not path or info.get("dev") or info.get("devOptional"):
            continue
        d = os.path.join(ROOT, *path.split("/"))
        if not os.path.isdir(d):
            sys.exit(f"{path} is missing — run `npm ci` first")
        with open(os.path.join(d, "package.json"), encoding="utf-8") as f:
            pj = json.load(f)
        repo = pj.get("repository")
        repo = repo.get("url") if isinstance(repo, dict) else repo
        out.append({
            "name": path.split("node_modules/")[-1], "version": info.get("version"), "license": info.get("license") or pj.get("license"),
            "source": (repo or f"https://www.npmjs.com/package/{path.split('node_modules/')[-1]}").replace("git+", ""),
            "authors": [], "dir": d, "files": license_files(d),
        })
    return sorted(out, key=lambda x: x["name"])


def texts_for(pkg, by_license):
    """License texts of a package; a package without its own file gets the standard text of its license."""
    if pkg["files"]:
        return [read(os.path.join(pkg["dir"], f)) for f in pkg["files"]]
    lic = pkg["license"]
    if lic == "MIT":
        holders = ", ".join(a.split(" <")[0] for a in pkg["authors"]) or f"the {pkg['name']} authors"
        return [f"Copyright (c) {holders}\n\n{MIT_TEXT}"]
    donor = by_license.get(lic)
    if donor:
        return [read(os.path.join(donor["dir"], donor["files"][0]))]
    sys.exit(f"no license text for {pkg['name']} {pkg['version']} ({lic})")


def render():
    rust = rust_packages()
    npm = npm_packages()
    by_license = {}
    for p in rust + npm:
        if p["files"] and len(p["files"]) == 1:
            by_license.setdefault(p["license"], p)

    lines = [
        "THIRD-PARTY NOTICES - Nexdir",
        "=" * 29,
        "",
        "Nexdir is licensed under the MIT License (see LICENSE). The Windows release contains the",
        "following third-party components. Each is used unmodified under the license named here; the",
        "full license texts follow below.",
        "",
        "Source code of every component is available from the location given (crates.io / npm",
        "packages of exactly the listed version). This includes the components under the Mozilla",
        "Public License 2.0, whose source is available at those locations as required by MPL-2.0",
        "section 3.2.",
        "",
        "nexdir.exe also statically links WebView2LoaderStatic.lib from the Microsoft Edge WebView2",
        "SDK (NuGet package Microsoft.Web.WebView2, via the webview2-com-sys crate), (c) Microsoft",
        "Corporation, distributed under the license included in that package:",
        "https://www.nuget.org/packages/Microsoft.Web.WebView2",
        "The Microsoft Edge WebView2 Runtime itself is not part of Nexdir; it is provided by Windows",
        "or installed by the Nexdir installer from Microsoft.",
        "",
        f"Rust crates linked into nexdir.exe ({len(rust)}):",
        "",
    ]
    for p in rust:
        lines.append(f"  {p['name']} {p['version']} - {p['license']} - {p['source']}")
    lines += ["", f"JavaScript packages bundled into the user interface ({len(npm)}):", ""]
    for p in npm:
        lines.append(f"  {p['name']} {p['version']} - {p['license']} - {p['source']}")
    lines += ["", "", "LICENSE TEXTS", "=" * 13, ""]

    # Identical texts are printed once, with every package that uses them.
    groups = {}
    for p in rust + npm:
        for t in texts_for(p, by_license):
            groups.setdefault(t, []).append(f"{p['name']} {p['version']}")
    for text, users in sorted(groups.items(), key=lambda kv: kv[1][0].lower()):
        lines.append("-" * 78)
        lines.append("Used by: " + ", ".join(sorted(set(users), key=str.lower)))
        lines.append("-" * 78)
        lines.append("")
        lines.append(text.rstrip("\n"))
        lines.append("")
    return "\n".join(lines) + "\n"


def main():
    content = render()
    if "--check" in sys.argv:
        current = read(OUT) if os.path.exists(OUT) else ""
        if current.strip() != content.strip():
            sys.exit("THIRD-PARTY-NOTICES.txt is out of date — run python tools/release/third_party_notices.py")
        print("THIRD-PARTY-NOTICES.txt is up to date")
        return
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(content)
    print(f"wrote {OUT} ({len(content):,} characters)")


if __name__ == "__main__":
    main()
