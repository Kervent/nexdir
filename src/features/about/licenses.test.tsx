import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import index from "../../generated/licenses.json";
import texts from "../../generated/license-texts.json";
import { filterComponents, licenseOptions, type LicenseIndex, type LicensedComponent } from "./licenses";
import { LicensesView } from "./LicensesView";
import { CreditsView } from "./CreditsView";

// Vitest runs from the repository root.
const ROOT = process.cwd();
const data = index as LicenseIndex;
const allTexts = texts as string[];
const notices = readFileSync(resolve(ROOT, "THIRD-PARTY-NOTICES.txt"), "utf-8").replace(/\r\n/g, "\n");
const pkg = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf-8")) as { dependencies: Record<string, string>; devDependencies: Record<string, string> };
const cargo = readFileSync(resolve(ROOT, "src-tauri/Cargo.toml"), "utf-8");

describe("license index", () => {
  it("lists every component of THIRD-PARTY-NOTICES.txt and nothing else", () => {
    const listed = notices
      .split("\nLICENSE TEXTS\n")[0]!
      .split("\nCOPYLEFT LICENSES\n")[0]!
      .split("\n")
      .filter((l) => /^ {2}\S/.test(l))
      .map((l) => l.trim());
    const fromIndex = data.components.map((c) => `${c.name} ${c.version} - ${c.license} - ${c.source}`);
    expect(new Set(listed)).toEqual(new Set(fromIndex));
    expect(listed).toHaveLength(data.components.length);
  });

  it("has a license text for every component, and every text is in the notices file", () => {
    for (const c of data.components) {
      expect(c.texts.length, c.name).toBeGreaterThan(0);
      for (const i of c.texts) expect(allTexts[i], c.name).toBeTruthy();
    }
    for (const t of allTexts) expect(notices).toContain(t.trim());
  });

  it("contains the direct runtime dependencies and none of the development tools", () => {
    const names = new Set(data.components.map((c) => c.name));
    for (const dep of Object.keys(pkg.dependencies)) expect(names, dep).toContain(dep);
    for (const dep of ["tauri", "serde", "blake3", "rusqlite", "tauri-plugin-updater", "tauri-plugin-window-state", "windows"]) expect(names, dep).toContain(dep);
    for (const dev of Object.keys(pkg.devDependencies)) expect(names.has(dev), `${dev} is a dev dependency`).toBe(false);
    // Build-time only: build-dependencies and proc macros (code that runs inside the compiler).
    for (const buildOnly of ["tauri-build", "tauri-macros", "serde_derive", "thiserror-impl"]) expect(names.has(buildOnly), buildOnly).toBe(false);
    expect(cargo).toContain("[dev-dependencies]");
  });

  it("lists development tools separately", () => {
    const tools = data.devTools.map((t) => t.name);
    for (const dev of Object.keys(pkg.devDependencies)) expect(tools).toContain(dev);
    expect(tools).toContain("tauri-build");
  });

  it("includes the native libraries with their real licenses", () => {
    const sqlite = data.components.find((c) => c.name === "SQLite")!;
    expect(sqlite.category).toBe("native");
    expect(allTexts[sqlite.texts[0]!]).toContain("The author disclaims copyright");
    const loader = data.components.find((c) => c.name === "Microsoft Edge WebView2 Loader")!;
    expect(loader.license).toBe("BSD-3-Clause");
    expect(allTexts[loader.texts[0]!]).toContain("Copyright (C) Microsoft Corporation");
  });

  it("documents every copyleft component", () => {
    const copyleft = data.components.filter((c) => c.copyleft);
    expect(copyleft.length).toBeGreaterThan(0);
    for (const c of copyleft) {
      expect(c.license).toMatch(/MPL-2\.0/);
      expect(notices).toContain(`${c.name} ${c.version}`);
    }
    expect(notices).toContain("COPYLEFT LICENSES");
  });
});

describe("search and filter", () => {
  const comps = data.components;

  it("finds by name, license and author, case-insensitively", () => {
    expect(filterComponents(comps, { query: "REACT", license: null, category: null }).map((c) => c.name)).toContain("react");
    expect(filterComponents(comps, { query: "mpl-2.0", license: null, category: null }).every((c) => c.license.includes("MPL-2.0"))).toBe(true);
    expect(filterComponents(comps, { query: "microsoft", license: null, category: "native" }).map((c) => c.name)).toEqual(["Microsoft Edge WebView2 Loader"]);
    expect(filterComponents(comps, { query: "no-such-component-xyz", license: null, category: null })).toEqual([]);
  });

  it("filters by license and type", () => {
    const mit = filterComponents(comps, { query: "", license: "MIT", category: null });
    expect(mit.length).toBeGreaterThan(10);
    expect(mit.every((c) => c.license === "MIT")).toBe(true);
    expect(filterComponents(comps, { query: "", license: null, category: "javascript" }).every((c) => c.category === "javascript")).toBe(true);
    const options = licenseOptions(comps);
    expect(options.reduce((n, o) => n + o.count, 0)).toBe(comps.length);
  });

  it("stays fast with the full list", () => {
    const start = performance.now();
    for (let i = 0; i < 500; i++) filterComponents(comps, { query: `a ${i % 10}`, license: null, category: null });
    expect(performance.now() - start).toBeLessThan(1000);
  });
});

describe("Open Source Licenses view", () => {
  const view = (copy = vi.fn(() => Promise.resolve()), openProjectUrl = vi.fn(() => Promise.resolve())) => {
    const loadTexts = vi.fn(() => Promise.resolve(allTexts));
    render(<LicensesView loadIndex={() => Promise.resolve(data)} loadTexts={loadTexts} openProjectUrl={openProjectUrl} copyText={copy} notify={() => {}} />);
    return { loadTexts, copy, openProjectUrl };
  };

  it("loads texts only when a license is opened, shows it and copies it", async () => {
    const { loadTexts, copy, openProjectUrl } = view();
    const list = await screen.findByRole("listbox");
    expect(within(list).getAllByRole("option")).toHaveLength(data.components.length);
    expect(loadTexts).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("Search components"), { target: { value: "lucide-react" } });
    fireEvent.click(within(list).getByRole("option", { name: /lucide-react/ }));
    const text = await screen.findByLabelText("License text of lucide-react");
    expect(text.textContent).toMatch(/ISC|Permission/);
    expect(loadTexts).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /Copy License Text/ }));
    await waitFor(() => expect(copy).toHaveBeenCalledWith(expect.stringContaining("lucide-react")));
    fireEvent.click(screen.getByRole("button", { name: /Open Project Page/ }));
    expect(openProjectUrl).toHaveBeenCalledWith(data.components.find((c) => c.name === "lucide-react")!.source);
  });

  it("filters by license from the drop-down", async () => {
    view();
    const list = await screen.findByRole("listbox");
    fireEvent.change(screen.getByLabelText("Filter by license"), { target: { value: "MPL-2.0" } });
    const shown = within(list).getAllByRole("option");
    expect(shown.length).toBe(data.components.filter((c: LicensedComponent) => c.license === "MPL-2.0").length);
  });

  it("renders the largest license text quickly", async () => {
    const largest = data.components.reduce((a, b) => (b.texts.reduce((n, i) => n + allTexts[i]!.length, 0) > a.texts.reduce((n, i) => n + allTexts[i]!.length, 0) ? b : a));
    view();
    const list = await screen.findByRole("listbox");
    fireEvent.change(screen.getByLabelText("Search components"), { target: { value: largest.name } });
    const start = performance.now();
    fireEvent.click(within(list).getAllByRole("option")[0]!);
    await screen.findByLabelText(`License text of ${largest.name}`);
    expect(performance.now() - start).toBeLessThan(1500);
  });

  it("works offline: the data is bundled, not fetched", () => {
    expect(data.components.length).toBeGreaterThan(200);
    expect(JSON.stringify(data)).not.toContain("api.github.com");
  });
});

describe("Technologies & Credits", () => {
  it("shows the sections, real versions and no invented contributors", async () => {
    const opened: string[] = [];
    render(<CreditsView discordInvite={null} openUrl={(u) => opened.push(u)} />);
    for (const name of ["Core Technologies", "Libraries", "Development Tools", "Community"]) expect(screen.getByRole("region", { name })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("Tauri 2").parentElement).toHaveTextContent(data.components.find((c) => c.name === "tauri")!.version));
    expect(screen.queryByRole("button", { name: /Discord/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /Contributors/ }));
    expect(opened).toEqual(["https://github.com/Kervent/nexdir/graphs/contributors"]);
  });
});
