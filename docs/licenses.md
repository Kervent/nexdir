# Open-source licenses

Nexdir is MIT-licensed and contains third-party open-source components. Their licenses are
shipped with every release and shown in the app.

| Where | What |
|---|---|
| `THIRD-PARTY-NOTICES.txt` | In the installers (next to `Nexdir.exe`) and the portable ZIP: every component, its license and source, copyleft notes, and all license texts |
| *About Nexdir → Open Source Licenses* | The same data, searchable and filterable by license and type, with the full license text, project link and *Copy License Text*. Works offline (bundled with the app). |
| *About Nexdir → Technologies & Credits* | Core technologies, main libraries, development tools and community links |

## How the list is generated

`python tools/release/third_party_notices.py` (after `npm ci`, with cargo on `PATH`) writes
three files from one set of data, so they always agree:

- `THIRD-PARTY-NOTICES.txt`
- `src/generated/licenses.json` — index for the app (name, version, SPDX license, type, authors,
  project page, copyleft note, text ids)
- `src/generated/license-texts.json` — deduplicated license texts, loaded only when the license
  view opens a component

What counts as shipped:

| Category | Source | Included |
|---|---|---|
| Rust crates | `cargo metadata --filter-platform x86_64-pc-windows-msvc` on `Cargo.lock` | Runtime dependencies reachable from Nexdir. Excluded: dev-dependencies, build-dependencies and proc macros (they only run while compiling). |
| JavaScript packages | `package-lock.json` | Everything not marked `dev` (bundled into the interface). `devDependencies` are excluded. |
| Native libraries | Crate sources | SQLite (compiled in by `libsqlite3-sys`, public domain; version read from `sqlite3.h`) and the Microsoft WebView2 loader (`WebView2LoaderStatic.lib` from the WebView2 SDK 1.0.3800.47 via `webview2-com-sys`; license text from the SDK package, stored in `tools/release/licenses/`). |
| Assets | — | Icons come from `lucide-react` (listed). The Nexdir logo is the project's own (MIT). No fonts are bundled (Segoe UI is part of Windows). |
| Development tools | `package.json` devDependencies, Cargo build-dependencies | Listed separately as *not distributed*, without texts. |

License texts come from each package's own `LICENSE*`, `COPYING*`, `NOTICE*` and `COPYRIGHT*`
files, so NOTICE and copyright lines are kept. A package without a license file gets the
standard text of its license (MIT with its authors, others from a package with the same license).

## Checks

`python tools/release/third_party_notices.py --check` runs in CI (job *Security*) and fails when

- any of the three files is out of date (a dependency was added, removed or updated),
- a package declares no license or no text can be found,
- a license is not on the reviewed list (`PERMISSIVE`/`COPYLEFT` in the script),
- the `webview2-com-sys` version changed (the WebView2 SDK license must be re-checked by a person).

The frontend tests (`licenses.test.tsx`) additionally check that the in-app index and
`THIRD-PARTY-NOTICES.txt` list exactly the same components, that every text is present, that no
dev dependency is listed as shipped, and that search, filter and the detail view work.

## Copyleft

| License | Components (0.5.0-alpha.1) | Obligation | How Nexdir meets it |
|---|---|---|---|
| MPL-2.0 | `cssparser`, `dtoa-short`, `option-ext`, `selectors` (Rust crates used by Tauri) | File-level copyleft: the source of the covered files must be available, and changes to those files must be published under MPL-2.0 | Used unmodified; the exact versions' source is on crates.io (linked in the notices); license text included |

No GPL, LGPL or AGPL code is included. Several crates are dual-licensed (`MIT OR Apache-2.0`
and similar); Nexdir uses them under the permissive option.
