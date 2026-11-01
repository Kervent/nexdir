import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { chooseIssueLink, ISSUE_FORMS, issueUrl, MAX_PREFILL_URL, prepareIssue, sanitize, userNamesFrom, validateIssue, windowsLabel, type IssueDraft } from "./report";
import { discordInvite, LINKS } from "../../config/links";

const bugValues = { description: "The list flickers.", steps: "1. Open a folder\n2. Press F5", expected: "No flicker", actual: "Flicker", "data-loss": "No", extra: "" };

describe("issue validation", () => {
  it("requires a title and the required fields", () => {
    const errors = validateIssue("bug", "", {});
    expect(errors.title).toBeDefined();
    expect(Object.keys(errors).sort()).toEqual(["actual", "data-loss", "description", "expected", "steps", "title"]);
    expect(validateIssue("bug", "List flickers on refresh", bugValues)).toEqual({});
  });

  it("rejects too short titles, overlong fields and unknown choices", () => {
    expect(validateIssue("bug", "abc", bugValues).title).toMatch(/at least/);
    expect(validateIssue("bug", "x".repeat(201), bugValues).title).toMatch(/under/);
    expect(validateIssue("bug", "A valid title", { ...bugValues, actual: "y".repeat(10_001) }).actual).toMatch(/Shorten/);
    expect(validateIssue("bug", "A valid title", { ...bugValues, "data-loss": "Maybe" })["data-loss"]).toMatch(/options/);
  });

  it("feature requests need problem, solution and benefit", () => {
    expect(Object.keys(validateIssue("feature", "Dark mode for icons", {})).sort()).toEqual(["benefit", "problem", "solution"]);
  });
});

describe("privacy filter", () => {
  it("removes drive, UNC and profile paths but keeps the sentence", () => {
    const r = sanitize("Copying C:\\Users\\Jane Doe\\My Documents\\tax-2025.pdf to \\\\nas\\share\\backup failed and then /Users/jane/x broke");
    expect(r.text).not.toMatch(/Jane|tax|nas|share|jane/);
    expect(r.text).toContain("<path removed>");
    expect(r.text).toContain("failed and then");
    expect(r.redactions).toEqual([{ kind: "path", count: 3 }]);
  });

  it("keeps bare drive letters", () => {
    expect(sanitize("Only on drive D: and E:\\ it fails").text).toBe("Only on drive D: and E:\\ it fails");
  });

  it("removes user names, e-mail addresses and secrets", () => {
    // Fake token assembled at run time so secret scanners do not flag the repository.
    const fakeToken = ["ghp", "0123456789abcdefghijABCDEFGHIJ0123"].join("_");
    const r = sanitize(`I am jdoe (jdoe@example.com). token=abc123 password: hunter2 ${fakeToken}`, ["jdoe"]);
    expect(r.text).not.toMatch(/jdoe|example\.com|abc123|hunter2|ghp_/);
    expect(r.text).toContain("token=<secret removed>");
    expect(r.redactions.map((x) => x.kind).sort()).toEqual(["email", "secret", "user"]);
  });

  it("does not treat ordinary words as secrets", () => {
    const text = "The secret folder and my password manager; token count 3. Authorization failed.";
    expect(sanitize(text)).toEqual({ text, redactions: [] });
    expect(sanitize("Header: Bearer abcdefghijklmnop1234").text).toBe("Header: Bearer <secret removed>");
  });

  it("removes Discord tokens and private keys", () => {
    const token = `${"M".padEnd(24, "a")}.${"b".repeat(6)}.${"c".repeat(30)}`;
    // Assembled at run time so secret scanners do not flag this fake key in the repository.
    const marker = ["PRIVATE", "KEY"].join(" ");
    const key = `-----BEGIN ${marker}-----\nabc\n-----END ${marker}-----`;
    const r = sanitize(`bot ${token} and ${key}`);
    expect(r.text).not.toContain(token);
    expect(r.text).not.toContain("BEGIN PRIVATE KEY");
  });

  it("finds the user name in the profile path", () => {
    expect(userNamesFrom("C:\\Users\\Kim\\Documents")).toEqual(["Kim"]);
    expect(userNamesFrom("D:\\Data")).toEqual([]);
    expect(userNamesFrom(undefined)).toEqual([]);
  });
});

describe("GitHub links", () => {
  const draft: IssueDraft = {
    kind: "bug",
    title: "Crash when opening C:\\Users\\Kim\\secret.txt",
    values: { ...bugValues, extra: "Ünïcödé & spaces + plus #hash ?q=1" },
    system: { version: "0.4.3", windows: "Windows 11 25H2 (build 26300.1)", architecture: "x86_64" },
  };

  it("prefills the bug form with cleaned, encoded fields", () => {
    const prepared = prepareIssue(draft, ["Kim"]);
    const { url, prefilled } = chooseIssueLink("bug", prepared);
    expect(prefilled).toBe(true);
    expect(url.startsWith(`${LINKS.issues}/new?template=bug_report.yml&`)).toBe(true);
    expect(url).not.toMatch(/[\s+]/); // spaces encoded as %20, never "+"
    const params = new URL(url).searchParams;
    expect(params.get("title")).toBe("[Bug] Crash when opening <path removed>");
    expect(params.get("version")).toBe("0.4.3");
    expect(params.get("windows")).toBe("Windows 11 25H2 (build 26300.1), x86_64");
    expect(params.get("extra")).toBe("Ünïcödé & spaces + plus #hash ?q=1");
    expect(params.get("data-loss")).toBe("No");
    expect(url).not.toContain("Kim");
  });

  it("leaves out technical details the user unticked", () => {
    const prepared = prepareIssue({ ...draft, system: {} }, []);
    const params = new URL(issueUrl("bug", prepared)).searchParams;
    expect(params.has("version")).toBe(false);
    expect(params.has("windows")).toBe(false);
    expect(prepared.markdown).not.toContain("Windows version");
  });

  it("falls back to the clipboard for long reports", () => {
    const long = { ...draft, values: { ...bugValues, description: "word ".repeat(2000) } };
    const prepared = prepareIssue(long, []);
    const { url, prefilled } = chooseIssueLink("bug", prepared);
    expect(prefilled).toBe(false);
    expect(url.length).toBeLessThan(MAX_PREFILL_URL);
    expect(new URL(url).searchParams.get("description")).toBeNull();
    expect(new URL(url).searchParams.get("title")).toContain("[Bug]");
    expect(prepared.markdown).toContain("### What happened?");
  });

  it("feature requests use their own template", () => {
    const prepared = prepareIssue({ kind: "feature", title: "Tabs on the side", values: { problem: "p", solution: "s", benefit: "b", context: "" }, system: { version: "0.4.3" } }, []);
    const url = issueUrl("feature", prepared);
    expect(url).toContain("template=feature_request.yml");
    expect(new URL(url).searchParams.get("context")).toBe("Nexdir version: 0.4.3");
  });

  it("windows label combines name, version and build", () => {
    expect(windowsLabel({ appVersion: "x", osName: "Windows 11", osVersion: "25H2", osBuild: "26300.1", architecture: "x86_64" })).toBe("Windows 11 25H2 (build 26300.1)");
    expect(windowsLabel({ appVersion: "x", osName: "Windows 10", osVersion: null, osBuild: null, architecture: "x86_64" })).toBe("Windows 10");
  });
});

describe("issue templates", () => {
  // The prefilled link only works if every field id exists in the GitHub form.
  for (const [kind, form] of Object.entries(ISSUE_FORMS)) {
    it(`${kind} form ids exist in ${form.template}`, () => {
      const yml = readFileSync(resolve(process.cwd(), ".github/ISSUE_TEMPLATE", form.template), "utf-8");
      const ids = [...yml.matchAll(/^\s+id:\s*(\S+)/gm)].map((m) => m[1]);
      for (const f of form.fields) expect(ids).toContain(f.id);
      if (kind === "bug") expect(ids).toEqual(expect.arrayContaining(["version", "windows"]));
      for (const f of form.fields.filter((x) => x.options)) {
        for (const o of f.options!) expect(yml).toContain(`"${o}"`);
      }
    });
  }
});

describe("Discord invite configuration", () => {
  it("accepts only Discord invite links", () => {
    expect(discordInvite(undefined)).toBeNull();
    expect(discordInvite("")).toBeNull();
    expect(discordInvite("https://discord.gg/abc123")).toBe("https://discord.gg/abc123");
    expect(discordInvite(" https://discord.com/invite/Abc-1 ")).toBe("https://discord.com/invite/Abc-1");
    expect(discordInvite("https://evil.example/discord.gg/abc")).toBeNull();
    expect(discordInvite("javascript:alert(1)")).toBeNull();
  });
});
