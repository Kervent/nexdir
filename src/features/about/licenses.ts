// Types and filtering for the bundled license index (src/generated/*.json, written by
// tools/release/third_party_notices.py — the same data as THIRD-PARTY-NOTICES.txt).

export type ComponentCategory = "native" | "rust" | "javascript";

export interface LicensedComponent {
  name: string;
  version: string;
  category: ComponentCategory;
  /** SPDX expression ("blessing" = SQLite's public-domain dedication). */
  license: string;
  source: string;
  authors: string[];
  /** Obligations of a copyleft license, when the component uses one. */
  copyleft: string | null;
  note: string | null;
  /** Indexes into the license-texts array. */
  texts: number[];
}

export interface DevTool {
  name: string;
  version: string;
  license: string;
  purpose: string;
}

export interface LicenseIndex {
  schema: number;
  components: LicensedComponent[];
  devTools: DevTool[];
}

export const CATEGORY_LABEL: Record<ComponentCategory, string> = {
  native: "Native library",
  rust: "Rust crate",
  javascript: "JavaScript package",
};

/** Readable form of an SPDX expression. */
export function licenseLabel(spdx: string): string {
  return spdx === "blessing" ? "Public domain (SQLite blessing)" : spdx;
}

/** Index loaded on demand: the About dialog does not slow down the start of Nexdir. */
export async function loadLicenseIndex(): Promise<LicenseIndex> {
  return (await import("../../generated/licenses.json")).default as LicenseIndex;
}

let textsPromise: Promise<string[]> | null = null;
/** License texts (about 0.5 MB) are loaded only when a license is opened, once. */
export function loadLicenseTexts(): Promise<string[]> {
  textsPromise ??= import("../../generated/license-texts.json").then((m) => m.default as string[]);
  return textsPromise;
}

/** Licenses by how often they are used, for the filter. */
export function licenseOptions(components: LicensedComponent[]): { license: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const c of components) counts.set(c.license, (counts.get(c.license) ?? 0) + 1);
  return [...counts].map(([license, count]) => ({ license, count })).sort((a, b) => b.count - a.count || a.license.localeCompare(b.license));
}

export interface LicenseFilter {
  query: string;
  license: string | null;
  category: ComponentCategory | null;
}

export function filterComponents(components: LicensedComponent[], { query, license, category }: LicenseFilter): LicensedComponent[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  return components.filter((c) => {
    if (license && c.license !== license) return false;
    if (category && c.category !== category) return false;
    const haystack = `${c.name} ${c.version} ${c.license} ${c.authors.join(" ")}`.toLowerCase();
    return terms.every((t) => haystack.includes(t));
  });
}

export function componentKey(c: Pick<LicensedComponent, "name" | "version" | "category">): string {
  return `${c.category}:${c.name}@${c.version}`;
}
