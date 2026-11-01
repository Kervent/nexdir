// Bug reports and feature requests go to GitHub issue forms. Nexdir never sends anything
// itself: it opens a prefilled issue page in the browser, where the user decides whether to
// submit. Before that, text is cleaned of paths, user names, e-mail addresses and secrets.

import { LINKS } from "../../config/links";

export type IssueKind = "bug" | "feature";

export interface IssueField {
  /** Field id in the GitHub issue form (.github/ISSUE_TEMPLATE/*.yml). */
  id: string;
  label: string;
  required: boolean;
  multiline: boolean;
  placeholder?: string;
  /** Allowed values for a choice field. */
  options?: string[];
}

export const ISSUE_FORMS: Record<IssueKind, { template: string; titlePrefix: string; fields: IssueField[] }> = {
  bug: {
    template: "bug_report.yml",
    titlePrefix: "[Bug] ",
    fields: [
      { id: "description", label: "What happened?", required: true, multiline: true, placeholder: "A short description of the problem." },
      { id: "steps", label: "Steps to reproduce", required: true, multiline: true, placeholder: "1. Open …\n2. Select …\n3. …" },
      { id: "expected", label: "Expected behaviour", required: true, multiline: true },
      { id: "actual", label: "Actual behaviour", required: true, multiline: true },
      { id: "data-loss", label: "Was any data lost or changed unexpectedly?", required: true, multiline: false, options: ["No", "Yes", "Not sure"] },
      { id: "extra", label: "Additional information (optional)", required: false, multiline: true, placeholder: "Drive type (local, USB, network), file system, anything else that helps." },
    ],
  },
  feature: {
    template: "feature_request.yml",
    titlePrefix: "[Feature] ",
    fields: [
      { id: "problem", label: "Problem description", required: true, multiline: true, placeholder: "What are you trying to do, and what makes it hard today?" },
      { id: "solution", label: "Proposed solution", required: true, multiline: true },
      { id: "benefit", label: "Expected benefit", required: true, multiline: true, placeholder: "Who would use it, and how would it help?" },
      { id: "context", label: "Additional context (optional)", required: false, multiline: true },
    ],
  },
};

export const MIN_TITLE = 6;
export const MAX_TITLE = 200;
export const MAX_FIELD = 10_000;
/** Prefilled links above this length are not opened; the report goes through the clipboard instead. */
export const MAX_PREFILL_URL = 6_000;

export interface SystemInfo {
  appVersion: string;
  osName: string;
  osVersion: string | null;
  osBuild: string | null;
  architecture: string;
}

export function windowsLabel(info: SystemInfo): string {
  return [info.osName, info.osVersion, info.osBuild ? `(build ${info.osBuild})` : null].filter(Boolean).join(" ");
}

/** Returns a message per invalid field id ("title" for the title), empty when valid. */
export function validateIssue(kind: IssueKind, title: string, values: Record<string, string>): Record<string, string> {
  const errors: Record<string, string> = {};
  const t = title.trim();
  if (t.length < MIN_TITLE) errors.title = `Enter a title of at least ${MIN_TITLE} characters.`;
  else if (t.length > MAX_TITLE) errors.title = `Keep the title under ${MAX_TITLE} characters.`;
  for (const f of ISSUE_FORMS[kind].fields) {
    const v = (values[f.id] ?? "").trim();
    if (f.required && !v) errors[f.id] = "This field is required.";
    else if (v.length > MAX_FIELD) errors[f.id] = `Shorten this to ${MAX_FIELD.toLocaleString()} characters or less.`;
    else if (f.options && v && !f.options.includes(v)) errors[f.id] = "Choose one of the options.";
  }
  return errors;
}

export interface Redaction {
  kind: "path" | "user" | "email" | "secret";
  count: number;
}

const SECRET_PATTERNS: RegExp[] = [
  /\bgh[pousr]_[A-Za-z0-9]{30,}\b/g, // GitHub tokens
  /\bgithub_pat_[A-Za-z0-9_]{40,}\b/g,
  /\b[MN][A-Za-z\d_-]{23,27}\.[A-Za-z\d_-]{6}\.[A-Za-z\d_-]{27,40}\b/g, // Discord bot tokens
  /\bxox[abprs]-[A-Za-z0-9-]{10,}\b/g, // Slack tokens
  /\bAKIA[0-9A-Z]{16}\b/g, // AWS access keys
  /\bsk-[A-Za-z0-9_-]{20,}\b/g, // API secret keys
  /-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----/g,
  /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/g, // JWTs
];
// "password: x", "token=x" — but not "the secret folder" in a normal sentence.
const SECRET_ASSIGNMENT = /\b(password|passwd|pwd|token|secret|api[_-]?key|access[_-]?key|authorization)(\s*[:=]\s*)("[^"]*"|'[^']*'|\S+)/gi;
const BEARER = /\bBearer\s+[A-Za-z0-9._~+/-]{16,}=*/g;
const EMAIL = /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g;
// Drive paths (C:\…, C:/…), UNC paths (\\server\share\…) and profile paths in other forms.
// Folder names may contain spaces (but never end with one — Windows forbids that); the last
// part ends at the first space, so the sentence after a path is kept. A file name with spaces
// is only partly hidden; the user sees and can edit the result before anything is shared.
const SEGMENT = String.raw`[\\/](?:[^\\/:*?"<>|\r\n]*[^\\/:*?"<>|\s.](?=[\\/])|[^\\/:*?"<>|\s]*)`;
const WINDOWS_PATH = new RegExp(String.raw`(?:\b[A-Za-z]:|\\\\[^\s\\/:*?"<>|]+)(?:${SEGMENT})*`, "g");
const PROFILE_PATH = /(?:\/|\\)(?:Users|home)(?:\/|\\)[^\\/\s]+/gi;

/**
 * Replaces personal or sensitive details with placeholders. `userNames` are additional words
 * to hide (the Windows user name, taken from the profile folder).
 */
export function sanitize(text: string, userNames: string[] = []): { text: string; redactions: Redaction[] } {
  const counts: Record<Redaction["kind"], number> = { path: 0, user: 0, email: 0, secret: 0 };
  let out = text;
  for (const re of SECRET_PATTERNS) {
    out = out.replace(re, () => (counts.secret++, "<secret removed>"));
  }
  out = out.replace(SECRET_ASSIGNMENT, (_m, key: string, sep: string) => (counts.secret++, `${key}${sep}<secret removed>`));
  out = out.replace(BEARER, () => (counts.secret++, "Bearer <secret removed>"));
  out = out.replace(EMAIL, () => (counts.email++, "<email removed>"));
  out = out.replace(WINDOWS_PATH, (m) => {
    // A lone drive letter ("C:" or "D:\") says nothing personal.
    if (/^[A-Za-z]:[\\/]?$/.test(m)) return m;
    counts.path++;
    return "<path removed>";
  });
  out = out.replace(PROFILE_PATH, () => (counts.path++, "<path removed>"));
  for (const name of userNames.filter((n) => n.length >= 3)) {
    const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    out = out.replace(new RegExp(`\\b${escaped}\\b`, "gi"), () => (counts.user++, "<user>"));
  }
  return { text: out, redactions: (Object.keys(counts) as Redaction["kind"][]).filter((k) => counts[k] > 0).map((kind) => ({ kind, count: counts[kind] })) };
}

/** The Windows user name, from the profile folder path (C:\Users\<name>). */
export function userNamesFrom(homePath: string | undefined): string[] {
  const m = homePath?.match(/[\\/]Users[\\/]([^\\/]+)/i);
  return m ? [m[1]!] : [];
}

export interface IssueDraft {
  kind: IssueKind;
  title: string;
  values: Record<string, string>;
  /** Technical details the user chose to include. */
  system: { version?: string; windows?: string; architecture?: string };
}

export interface PreparedIssue {
  title: string;
  /** Field id → cleaned value, as sent in the prefilled link. */
  fields: Record<string, string>;
  /** The same content as Markdown, for the clipboard. */
  markdown: string;
  redactions: Redaction[];
}

export function prepareIssue(draft: IssueDraft, userNames: string[]): PreparedIssue {
  const form = ISSUE_FORMS[draft.kind];
  const all: Redaction[] = [];
  const clean = (s: string) => {
    const r = sanitize(s.trim(), userNames);
    for (const x of r.redactions) {
      const existing = all.find((a) => a.kind === x.kind);
      if (existing) existing.count += x.count;
      else all.push({ ...x });
    }
    return r.text;
  };
  const title = clean(draft.title);
  const fields: Record<string, string> = {};
  for (const f of form.fields) {
    const v = clean(draft.values[f.id] ?? "");
    if (v) fields[f.id] = v;
  }
  if (draft.kind === "bug") {
    if (draft.system.version) fields.version = draft.system.version;
    const windows = [draft.system.windows, draft.system.architecture].filter(Boolean).join(", ");
    if (windows) fields.windows = windows;
  } else if (draft.system.version) {
    fields.context = [fields.context, `Nexdir version: ${draft.system.version}`].filter(Boolean).join("\n\n");
  }

  const sections: string[] = [];
  if (fields.version) sections.push(`### Nexdir version\n\n${fields.version}`);
  if (fields.windows) sections.push(`### Windows version\n\n${fields.windows}`);
  for (const f of form.fields) {
    if (fields[f.id]) sections.push(`### ${f.label.replace(/ \(optional\)$/, "")}\n\n${fields[f.id]}`);
  }
  return { title, fields, markdown: `${sections.join("\n\n")}\n`, redactions: all };
}

/** Prefilled GitHub issue-form link (`?template=…&title=…&<field-id>=…`). */
export function issueUrl(kind: IssueKind, prepared: PreparedIssue | null): string {
  const form = ISSUE_FORMS[kind];
  const params = new URLSearchParams({ template: form.template });
  if (prepared) {
    params.set("title", `${form.titlePrefix}${prepared.title}`);
    for (const [id, value] of Object.entries(prepared.fields)) params.set(id, value);
  }
  // URLSearchParams encodes spaces as "+", which GitHub reads as spaces; %20 is unambiguous.
  return `${LINKS.issues}/new?${params.toString().replace(/\+/g, "%20")}`;
}

/**
 * The link to open: prefilled when it fits, otherwise only the title (the user pastes the
 * report from the clipboard).
 */
export function chooseIssueLink(kind: IssueKind, prepared: PreparedIssue): { url: string; prefilled: boolean } {
  const full = issueUrl(kind, prepared);
  if (full.length <= MAX_PREFILL_URL) return { url: full, prefilled: true };
  return { url: issueUrl(kind, { ...prepared, fields: {} }), prefilled: false };
}
