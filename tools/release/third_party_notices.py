"""Generates the third-party license information of the Windows release.

One source of data, three outputs that therefore always agree:
  THIRD-PARTY-NOTICES.txt           shipped with every release (installers and portable ZIP)
  src/generated/licenses.json       index shown in About Nexdir -> Open Source Licenses
  src/generated/license-texts.json  the license texts for that view (loaded only when opened)

What is listed (only what ends up in what users download):
  * Rust crates linked into nexdir.exe: runtime dependencies for x86_64-pc-windows-msvc. Build
    scripts, build-dependencies, dev-dependencies and proc macros are excluded — they only run
    while compiling.
  * npm packages bundled into the user interface: everything in package-lock.json that is not
    marked dev (devDependencies are build tools, not shipped).
  * Native code compiled or linked in that is not a crate of its own: SQLite (via
    libsqlite3-sys "bundled") and the Microsoft WebView2 loader (via webview2-com-sys).
Development tools (compilers, bundler, test runner, Tauri CLI) are listed separately as not
distributed, without license texts.

The script fails when a license is missing or unknown, when a copyleft license appears that is
not reviewed below, or when a native component's version changed (its license must be checked
again by a person).

Usage (from the repository root, after `npm ci`, with cargo on PATH):
    python tools/release/third_party_notices.py            # writes the three files
    python tools/release/third_party_notices.py --check    # fails if any of them is out of date
"""
import json
import os
import re
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(ROOT, "THIRD-PARTY-NOTICES.txt")
INDEX = os.path.join(ROOT, "src", "generated", "licenses.json")
TEXTS = os.path.join(ROOT, "src", "generated", "license-texts.json")
LICENSE_DIR = os.path.join(os.path.dirname(__file__), "licenses")
TARGET = "x86_64-pc-windows-msvc"
LICENSE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")

# SPDX identifiers reviewed as permissive (no obligations beyond keeping notices).
PERMISSIVE = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
    "ISC", "Zlib", "0BSD", "BSL-1.0", "CC0-1.0", "Unlicense", "Unicode-3.0", "Unicode-DFS-2016",
    "CDLA-Permissive-2.0", "blessing",
}
# Reviewed copyleft licenses and what they require from Nexdir.
COPYLEFT = {
    "MPL-2.0": "File-level copyleft. The covered files are used unmodified; their source code is "
               "available at the listed location (MPL-2.0 section 3.2). Changes to those files "
               "would have to be published under MPL-2.0.",
}

# Native components: (crate that brings them, crate version the entry was checked for).
WEBVIEW2_CRATE = ("webview2-com-sys", "0.39.1")
SQLITE_CRATE = "libsqlite3-sys"

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


def fail(message):
    sys.exit(f"third_party_notices: {message}")


def read(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return f.read().replace("\r\n", "\n").strip() + "\n"


def license_files(directory):
    if not os.path.isdir(directory):
        return []
    return sorted(f for f in os.listdir(directory) if f.upper().startswith(LICENSE_PREFIXES) and os.path.isfile(os.path.join(directory, f)))


def normalize(expr):
    """Older crates write "MIT/Apache-2.0"; SPDX says "MIT OR Apache-2.0"."""
    return re.sub(r"\s*/\s*", " OR ", (expr or "").strip())


def classify(expr):
    """("permissive" | "copyleft", note) for an SPDX expression, or fails for unknown terms.

    An OR expression is permissive when one alternative is permissive (Nexdir uses that one)."""
    def term_kind(term):
        term = term.strip().strip("()").strip()
        if term in PERMISSIVE:
            return "permissive"
        if term in COPYLEFT:
            return "copyleft"
        return None

    alternatives = [a for a in re.split(r"\s+OR\s+", expr) if a.strip()]
    kinds = []
    for alt in alternatives:
        parts = [term_kind(t) for t in re.split(r"\s+AND\s+", alt.strip().strip("()"))]
        if None in parts:
            kinds.append(None)
        elif all(p == "permissive" for p in parts):
            return "permissive", None
        else:
            kinds.append("copyleft")
    if "copyleft" in kinds:
        used = next(t.strip("() ") for alt in alternatives for t in re.split(r"\s+AND\s+", alt) if term_kind(t) == "copyleft")
        return "copyleft", COPYLEFT[used]
    fail(f"license {expr!r} is not reviewed — add it to PERMISSIVE or COPYLEFT after checking its terms")


def homepage(url):
    url = (url or "").strip().replace("git+", "").replace("git://", "https://")
    url = re.sub(r"^git@github\.com:", "https://github.com/", url)
    url = re.sub(r"\.git$", "", url)
    return url if url.startswith("https://") else ""


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
        if not p.get("license") and not license_files(d):
            fail(f"crate {p['name']} {p['version']} declares no license")
        out.append({
            "name": p["name"], "version": p["version"], "category": "rust",
            "license": normalize(p.get("license")) or "LicenseRef-see-license-file",
            "source": p.get("repository") or p.get("homepage") or f"https://crates.io/crates/{p['name']}",
            "authors": [a.split(" <")[0] for a in p.get("authors") or []], "dir": d, "files": license_files(d),
        })
    build_tools = []
    root_pkg = pkgs[root]
    for dep in root_pkg["dependencies"]:
        if dep["kind"] == "build":
            match = next(p for p in meta["packages"] if p["name"] == dep["name"])
            build_tools.append({"name": dep["name"], "version": match["version"], "license": normalize(match.get("license")),
                                "source": match.get("repository") or "", "purpose": "Build script (Rust)"})
    return sorted(out, key=lambda x: (x["name"], x["version"])), build_tools, {p["name"]: p for p in meta["packages"]}


def npm_packages():
    with open(os.path.join(ROOT, "package-lock.json"), encoding="utf-8") as f:
        lock = json.load(f)
    with open(os.path.join(ROOT, "package.json"), encoding="utf-8") as f:
        manifest = json.load(f)
    out, dev = [], []
    for path, info in lock["packages"].items():
        if not path:
            continue
        name = path.split("node_modules/")[-1]
        d = os.path.join(ROOT, *path.split("/"))
        is_dev = info.get("dev") or info.get("devOptional")
        if is_dev:
            if name in manifest.get("devDependencies", {}) and path == f"node_modules/{name}":
                dev.append({"name": name, "version": info.get("version"), "license": normalize(info.get("license")),
                            "source": f"https://www.npmjs.com/package/{name}", "purpose": "Build, test or tooling (npm)"})
            continue
        if not os.path.isdir(d):
            fail(f"{path} is missing — run `npm ci` first")
        with open(os.path.join(d, "package.json"), encoding="utf-8") as f:
            pj = json.load(f)
        repo = pj.get("repository")
        repo = repo.get("url") if isinstance(repo, dict) else repo
        author = pj.get("author")
        author = author.get("name") if isinstance(author, dict) else author
        lic = normalize(info.get("license") or pj.get("license"))
        if not lic:
            fail(f"npm package {name} declares no license")
        out.append({
            "name": name, "version": info.get("version"), "category": "javascript", "license": lic,
            "source": homepage(repo) or pj.get("homepage") or f"https://www.npmjs.com/package/{name}",
            "authors": [author.split(" <")[0]] if author else [], "dir": d, "files": license_files(d),
        })
    return sorted(out, key=lambda x: x["name"]), sorted(dev, key=lambda x: x["name"])


def native_components(crates):
    out = []
    sqlite_crate = crates.get(SQLITE_CRATE)
    if sqlite_crate:
        header = os.path.join(os.path.dirname(sqlite_crate["manifest_path"]), "sqlite3", "sqlite3.h")
        with open(header, encoding="utf-8", errors="replace") as f:
            head = f.read()
        version = re.search(r'#define SQLITE_VERSION\s+"([^"]+)"', head).group(1)
        blessing = re.search(r"\*\* The author disclaims copyright.*?share freely, never taking more than you give\.", head, re.S)
        if not blessing:
            fail("SQLite's public-domain notice was not found in sqlite3.h — check its license")
        text = "\n".join(line.lstrip("*").strip() for line in blessing.group(0).splitlines()).strip() + "\n"
        out.append({"name": "SQLite", "version": version, "category": "native", "license": "blessing",
                    "source": "https://sqlite.org/copyright.html", "authors": ["D. Richard Hipp and the SQLite developers"],
                    "note": f"Compiled into nexdir.exe from the amalgamation bundled with the {SQLITE_CRATE} crate. Public domain.",
                    "texts": [text]})
    name, checked = WEBVIEW2_CRATE
    wv = crates.get(name)
    if wv:
        if wv["version"] != checked:
            fail(f"{name} changed from {checked} to {wv['version']}: check the WebView2 SDK version and license, "
                 "update tools/release/licenses/ and WEBVIEW2_CRATE")
        sdk = "1.0.3800.47"
        out.append({"name": "Microsoft Edge WebView2 Loader", "version": sdk, "category": "native", "license": "BSD-3-Clause",
                    "source": "https://www.nuget.org/packages/Microsoft.Web.WebView2", "authors": ["Microsoft Corporation"],
                    "note": f"WebView2LoaderStatic.lib from the WebView2 SDK, linked into nexdir.exe through the {name} crate. "
                            "The WebView2 Runtime itself is part of Windows (or installed from Microsoft) and not distributed with Nexdir.",
                    "texts": [read(os.path.join(LICENSE_DIR, f"Microsoft.Web.WebView2-{sdk}.txt"))]})
    return out


def texts_for(pkg, by_license):
    """License texts of a package; a package without its own file gets the standard text of its license."""
    if pkg.get("texts"):
        return pkg["texts"]
    if pkg["files"]:
        return [read(os.path.join(pkg["dir"], f)) for f in pkg["files"]]
    lic = pkg["license"]
    if lic == "MIT":
        holders = ", ".join(pkg["authors"]) or f"the {pkg['name']} authors"
        return [f"Copyright (c) {holders}\n\n{MIT_TEXT}"]
    donor = by_license.get(lic)
    if donor:
        return [read(os.path.join(donor["dir"], donor["files"][0]))]
    fail(f"no license text for {pkg['name']} {pkg['version']} ({lic})")


def collect():
    rust, build_tools, crates = rust_packages()
    npm, npm_dev = npm_packages()
    native = native_components(crates)
    by_license = {}
    for p in rust + npm:
        if p["files"] and len(p["files"]) == 1:
            by_license.setdefault(p["license"], p)
    texts, text_ids, components = [], {}, []
    for p in native + rust + npm:
        kind, note = classify(p["license"]) if p["license"] != "LicenseRef-see-license-file" else ("permissive", None)
        ids = []
        for t in texts_for(p, by_license):
            if t not in text_ids:
                text_ids[t] = len(texts)
                texts.append(t)
            ids.append(text_ids[t])
        components.append({
            "name": p["name"], "version": p["version"], "category": p["category"], "license": p["license"],
            "source": p["source"], "authors": p["authors"], "copyleft": note, "note": p.get("note"), "texts": ids,
            "kind": kind,
        })
    dev_tools = sorted(npm_dev + build_tools, key=lambda x: x["name"].lower())
    return components, texts, dev_tools


def render_notices(components, texts, dev_tools):
    groups = {"native": [], "rust": [], "javascript": []}
    for c in components:
        groups[c["category"]].append(c)
    lines = [
        "THIRD-PARTY NOTICES - Nexdir",
        "=" * 29,
        "",
        "Nexdir is licensed under the MIT License (see LICENSE). The Windows release contains the",
        "following third-party components. Each is used unmodified under the license named here; the",
        "full license texts follow below. The same list is shown in Nexdir under",
        "About Nexdir -> Open Source Licenses.",
        "",
        "Source code of every component is available from the location given (crates.io / npm",
        "packages of exactly the listed version).",
        "",
        f"Native code compiled or linked into nexdir.exe ({len(groups['native'])}):",
        "",
    ]
    for c in groups["native"]:
        lines.append(f"  {c['name']} {c['version']} - {c['license']} - {c['source']}")
        lines += ["    " + l for l in wrap(c["note"], 90)]
    lines += ["", f"Rust crates linked into nexdir.exe ({len(groups['rust'])}):", ""]
    lines += [f"  {c['name']} {c['version']} - {c['license']} - {c['source']}" for c in groups["rust"]]
    lines += ["", f"JavaScript packages bundled into the user interface ({len(groups['javascript'])}):", ""]
    lines += [f"  {c['name']} {c['version']} - {c['license']} - {c['source']}" for c in groups["javascript"]]

    copyleft = [c for c in components if c["copyleft"]]
    lines += ["", "", "COPYLEFT LICENSES", "=" * 17, ""]
    if copyleft:
        by_note = {}
        for c in copyleft:
            by_note.setdefault((c["license"], c["copyleft"]), []).append(f"{c['name']} {c['version']}")
        for (lic, note), names in sorted(by_note.items()):
            lines.append(f"{lic}: {', '.join(sorted(names))}")
            lines += wrap(note, 94)
            lines.append("")
    else:
        lines += ["None.", ""]

    lines += ["", "DEVELOPMENT TOOLS (NOT DISTRIBUTED)", "=" * 35, "",
              "Used to build and test Nexdir; they are not part of the release files:", ""]
    lines += [f"  {t['name']} {t['version']} - {t['license']}" for t in dev_tools]
    lines += ["  Rust compiler and Cargo - MIT OR Apache-2.0", ""]

    lines += ["", "LICENSE TEXTS", "=" * 13, ""]
    users = {}
    for c in components:
        for i in c["texts"]:
            users.setdefault(i, []).append(f"{c['name']} {c['version']}")
    for i in sorted(users, key=lambda i: sorted(users[i], key=str.lower)[0].lower()):
        lines.append("-" * 78)
        lines.append("Used by: " + ", ".join(sorted(set(users[i]), key=str.lower)))
        lines.append("-" * 78)
        lines.append("")
        lines.append(texts[i].rstrip("\n"))
        lines.append("")
    return "\n".join(lines) + "\n"


def wrap(text, width):
    out, line = [], ""
    for word in (text or "").split():
        if line and len(line) + 1 + len(word) > width:
            out.append(line)
            line = word
        else:
            line = f"{line} {word}".strip()
    return out + ([line] if line else [])


def render_index(components, dev_tools):
    index = {
        "schema": 1,
        "notice": "Generated by tools/release/third_party_notices.py from the lockfiles. Do not edit.",
        "components": [{k: v for k, v in c.items() if k != "kind"} for c in components],
        "devTools": [{k: t[k] for k in ("name", "version", "license", "purpose")} for t in dev_tools],
    }
    return json.dumps(index, indent=1, ensure_ascii=False) + "\n"


def outputs():
    components, texts, dev_tools = collect()
    return {
        OUT: render_notices(components, texts, dev_tools),
        INDEX: render_index(components, dev_tools),
        TEXTS: json.dumps(texts, ensure_ascii=False) + "\n",
    }


def main():
    files = outputs()
    if "--check" in sys.argv:
        stale = [os.path.relpath(p, ROOT) for p, content in files.items()
                 if not os.path.exists(p) or read(p).strip() != content.strip()]
        if stale:
            fail(f"{', '.join(stale)} out of date — run python tools/release/third_party_notices.py")
        print("Third-party notices and license index are up to date")
        return
    for path, content in files.items():
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(content)
        print(f"wrote {os.path.relpath(path, ROOT)} ({len(content):,} characters)")


if __name__ == "__main__":
    main()
