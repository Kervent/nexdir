// Windows path helpers for display purposes. The backend remains the authority on paths.

export interface Crumb {
  label: string;
  path: string;
}

/** Splits "C:\Users\me" into [C:\, Users, me] crumbs; UNC roots stay together. */
export function breadcrumbs(path: string): Crumb[] {
  const normalized = path.replace(/\//g, "\\");
  const crumbs: Crumb[] = [];
  let rest: string;
  let current: string;

  const unc = /^\\\\([^\\]+)\\([^\\]+)\\?/.exec(normalized);
  const drive = /^([A-Za-z]:)\\?/.exec(normalized);
  if (unc) {
    current = `\\\\${unc[1]}\\${unc[2]}`;
    crumbs.push({ label: `${unc[1]} › ${unc[2]}`, path: current });
    rest = normalized.slice(unc[0].length);
  } else if (drive) {
    current = `${drive[1]}\\`;
    crumbs.push({ label: `${drive[1]!.toUpperCase()}`, path: current });
    rest = normalized.slice(drive[0].length);
  } else {
    return [{ label: path, path }];
  }

  for (const part of rest.split("\\").filter(Boolean)) {
    current = current.endsWith("\\") ? `${current}${part}` : `${current}\\${part}`;
    crumbs.push({ label: part, path: current });
  }
  return crumbs;
}

export function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const idx = Math.max(trimmed.lastIndexOf("\\"), trimmed.lastIndexOf("/"));
  return idx >= 0 ? trimmed.slice(idx + 1) || path : trimmed;
}

export function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
  return norm(a) === norm(b);
}

/** Proposes "New folder", "New folder (2)", … that does not clash with existing names. */
export function uniqueName(base: string, existing: Iterable<string>): string {
  const taken = new Set(Array.from(existing, (n) => n.toLowerCase()));
  if (!taken.has(base.toLowerCase())) return base;
  for (let i = 2; ; i++) {
    const candidate = `${base} (${i})`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
}

const INVALID_CHARS = /[<>:"/\\|?*\u0000-\u001f]/;
const RESERVED = /^(con|prn|aux|nul|conin\$|conout\$|com[0-9¹²³]|lpt[0-9¹²³])$/i;

/** Client-side mirror of the backend name validation, for instant feedback. */
export function validateName(name: string): string | null {
  if (!name.trim()) return "The name cannot be empty.";
  if (name === "." || name === "..") return "This name is reserved.";
  if (INVALID_CHARS.test(name)) return 'Names cannot contain < > : " / \\ | ? * or control characters.';
  if (/[ .]$/.test(name)) return "Names cannot end with a space or a period.";
  if (name.length > 255) return "The name is too long (maximum 255 characters).";
  const stem = name.split(".")[0]!.trimEnd();
  if (RESERVED.test(stem)) return "This name is reserved by Windows.";
  return null;
}
