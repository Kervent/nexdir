import type { ReactNode } from "react";

/**
 * Release notes are Markdown written on GitHub. They come from the network, so they are never
 * rendered as HTML: headings, list items and paragraphs become plain React text, **bold** is the
 * only inline style, and links show their text only (the release page has the clickable version).
 */
export function ReleaseNotes({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let list: string[] = [];
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length) blocks.push(<p key={blocks.length}>{inline(paragraph.join(" "))}</p>);
    if (list.length)
      blocks.push(
        <ul key={blocks.length}>
          {list.map((item, i) => (
            <li key={i}>{inline(item)}</li>
          ))}
        </ul>,
      );
    paragraph = [];
    list = [];
  };
  for (const raw of text.replace(/\r\n/g, "\n").split("\n")) {
    const line = raw.trim();
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    const item = /^[-*+]\s+(.*)$/.exec(line) ?? /^\d+[.)]\s+(.*)$/.exec(line);
    if (!line || /^(-{3,}|\*{3,}|_{3,})$/.test(line) || line.startsWith("```")) flush();
    else if (heading) {
      flush();
      blocks.push(<h4 key={blocks.length}>{inline(heading[1]!)}</h4>);
    } else if (item) {
      if (paragraph.length) flush();
      list.push(item[1]!);
    } else if (line.startsWith("|")) continue; // tables (checksums) are on the release page
    else {
      if (list.length) flush();
      // Quotes lose their marker; GitHub alerts ("> [!WARNING]") keep only their text.
      const plain = line.replace(/^>\s?/, "").replace(/^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/i, "");
      if (plain) paragraph.push(plain);
    }
  }
  flush();
  return <div className="nx-release-notes">{blocks.length ? blocks : <p className="nx-muted">No release notes were provided.</p>}</div>;
}

/** Drops everything from a "<" up to the next ">" (or the end): no "<" can survive. */
export function stripTags(text: string): string {
  let out = "";
  let inTag = false;
  for (const ch of text) {
    if (ch === "<") inTag = true;
    else if (inTag) inTag = ch !== ">";
    else out += ch;
  }
  return out;
}

/** `**bold**`, `` `code` ``, `[text](url)` → text; everything else stays literal. */
function inline(text: string): ReactNode[] {
  const cleaned = stripTags(text.replace(/!\[[^\]]*\]\([^)]*\)/g, "").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1"));
  return cleaned.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).map((part, i) =>
    part.startsWith("**") && part.endsWith("**") && part.length > 4 ? (
      <strong key={i}>{part.slice(2, -2)}</strong>
    ) : part.startsWith("`") && part.endsWith("`") && part.length > 2 ? (
      <code key={i}>{part.slice(1, -1)}</code>
    ) : (
      part
    ),
  );
}
