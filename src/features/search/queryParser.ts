// Deterministic parser for plain-English search requests such as
// "Find all videos larger than 2 GB in Downloads" or "pdf files from last month".
// Everything runs locally; nothing is sent anywhere. Unrecognised words become name text,
// and every interpretation is shown to the user as a chip so nothing happens silently.
import type { KnownFolderId } from "../../types/fs";
import type { SearchKind } from "../../types/tools";

export interface ParsedQuery {
  text: string | null;
  extensions: string[];
  categories: string[];
  minSize: number | null;
  maxSize: number | null;
  modifiedAfter: number | null;
  modifiedBefore: number | null;
  kind: SearchKind;
  /** Known folder id or an absolute path. */
  scope: KnownFolderId | string | null;
  /** Human-readable interpretation, one entry per recognised condition. */
  chips: string[];
}

const UNITS: Record<string, number> = {
  b: 1,
  byte: 1,
  bytes: 1,
  k: 1024,
  kb: 1024,
  kilobyte: 1024,
  kilobytes: 1024,
  m: 1024 ** 2,
  mb: 1024 ** 2,
  megabyte: 1024 ** 2,
  megabytes: 1024 ** 2,
  g: 1024 ** 3,
  gb: 1024 ** 3,
  gig: 1024 ** 3,
  gigs: 1024 ** 3,
  gigabyte: 1024 ** 3,
  gigabytes: 1024 ** 3,
  t: 1024 ** 4,
  tb: 1024 ** 4,
  terabyte: 1024 ** 4,
  terabytes: 1024 ** 4,
};

const CATEGORY_WORDS: Record<string, string> = {
  video: "video",
  videos: "video",
  movie: "video",
  movies: "video",
  films: "video",
  image: "image",
  images: "image",
  picture: "image",
  pictures: "image",
  photo: "image",
  photos: "image",
  pics: "image",
  screenshots: "image",
  music: "audio",
  song: "audio",
  songs: "audio",
  audio: "audio",
  document: "document",
  documents: "document",
  docs: "document",
  archive: "archive",
  archives: "archive",
  program: "program",
  programs: "program",
  applications: "program",
  apps: "program",
  installers: "program",
  code: "code",
};

const CATEGORY_LABELS: Record<string, string> = {
  video: "Videos",
  image: "Images",
  audio: "Music & audio",
  document: "Documents",
  archive: "Archives",
  program: "Programs",
  code: "Source code",
};

const KNOWN_EXTENSIONS = new Set(
  "pdf zip rar 7z iso exe msi txt md log csv json xml doc docx xls xlsx ppt pptx odt jpg jpeg png gif bmp webp heic svg psd raw mp4 mkv avi mov wmv webm mp3 wav flac aac ogg m4a ini cfg ps1 bat py js ts rs html css sql apk dmg torrent".split(
    " ",
  ),
);

const FOLDERS: Record<string, KnownFolderId> = {
  desktop: "desktop",
  downloads: "downloads",
  download: "downloads",
  documents: "documents",
  pictures: "pictures",
  music: "music",
  videos: "videos",
  home: "home",
};

const FOLDER_LABELS: Record<KnownFolderId, string> = {
  home: "Home folder",
  desktop: "Desktop",
  downloads: "Downloads",
  documents: "Documents",
  pictures: "Pictures",
  music: "Music",
  videos: "Videos",
};

const STOPWORDS = new Set(
  "find search show list get give me all every any my the a an of with which that are is was were and or for files file items item named called containing contains modified changed created from in on at than whose".split(
    " ",
  ),
);

const DAY = 86_400_000;

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function formatSizeShort(bytes: number): string {
  for (const [unit, f] of [["TB", 1024 ** 4], ["GB", 1024 ** 3], ["MB", 1024 ** 2], ["KB", 1024]] as const) {
    if (bytes >= f) return `${+(bytes / f).toFixed(2)} ${unit}`;
  }
  return `${bytes} B`;
}

function formatDay(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function parseQuery(input: string, now: Date = new Date()): ParsedQuery {
  const out: ParsedQuery = {
    text: null,
    extensions: [],
    categories: [],
    minSize: null,
    maxSize: null,
    modifiedAfter: null,
    modifiedBefore: null,
    kind: "any",
    scope: null,
    chips: [],
  };
  // Sentence punctuation ("…larger than 2 GB.") is not part of any condition.
  let s = ` ${input.trim().replace(/[.!,;]+$/, "")} `;
  const nameParts: string[] = [];
  const take = (re: RegExp, fn: (m: RegExpExecArray) => void) => {
    let m: RegExpExecArray | null;
    while ((m = re.exec(s))) {
      fn(m);
      s = s.slice(0, m.index) + " " + s.slice(m.index + m[0].length);
      re.lastIndex = 0;
    }
  };

  // Quoted text and explicit name phrases are always name text.
  take(/"([^"]+)"/g, (m) => nameParts.push(m[1]!));
  take(/\b(?:named|called|containing|with name)\s+([^\s]+)/gi, (m) => nameParts.push(m[1]!));

  // Scope: "in C:\path" or "in Downloads".
  take(/\b(?:in|inside|under)\s+([a-z]:\\[^\s]*)/gi, (m) => {
    out.scope = m[1]!;
    out.chips.push(`In: ${m[1]}`);
  });
  take(/\b(?:in|inside|from)\s+(?:the\s+|my\s+)?(desktop|downloads?|documents|pictures|music|videos|home)(?:\s+folder)?\b/gi, (m) => {
    const id = FOLDERS[m[1]!.toLowerCase()]!;
    out.scope = id;
    out.chips.push(`In: ${FOLDER_LABELS[id]}`);
  });

  // Sizes.
  const num = String.raw`(\d+(?:[.,]\d+)?)\s*([a-z]+)`;
  take(new RegExp(String.raw`(?:larger|bigger|greater|more|over|above|>=?|at least)(?:\s+than)?\s+${num}`, "gi"), (m) => {
    const unit = UNITS[m[2]!.toLowerCase()];
    if (!unit) return;
    out.minSize = Math.round(parseFloat(m[1]!.replace(",", ".")) * unit);
    out.chips.push(`Size > ${formatSizeShort(out.minSize)}`);
  });
  take(new RegExp(String.raw`(?:smaller|less|under|below|<=?|at most)(?:\s+than)?\s+${num}`, "gi"), (m) => {
    const unit = UNITS[m[2]!.toLowerCase()];
    if (!unit) return;
    out.maxSize = Math.round(parseFloat(m[1]!.replace(",", ".")) * unit);
    out.chips.push(`Size < ${formatSizeShort(out.maxSize)}`);
  });

  // Dates (modification time).
  const today = startOfDay(now).getTime();
  const after = (ms: number, label: string) => {
    out.modifiedAfter = ms;
    out.chips.push(label);
  };
  const before = (ms: number, label: string) => {
    out.modifiedBefore = ms;
    out.chips.push(label);
  };
  const unitDays: Record<string, number> = { day: 1, days: 1, week: 7, weeks: 7, month: 30, months: 30, year: 365, years: 365 };
  take(/\bolder than\s+(\d+)\s+(days?|weeks?|months?|years?)\b/gi, (m) => {
    const n = Number(m[1]) * unitDays[m[2]!.toLowerCase()]!;
    before(now.getTime() - n * DAY, `Modified more than ${m[1]} ${m[2]} ago`);
  });
  take(/\b(?:in the\s+)?(?:last|past)\s+(\d+)\s+(days?|weeks?|months?|years?)\b/gi, (m) => {
    const n = Number(m[1]) * unitDays[m[2]!.toLowerCase()]!;
    after(now.getTime() - n * DAY, `Modified in the last ${m[1]} ${m[2]}`);
  });
  take(/\b(?:modified\s+|from\s+|changed\s+|created\s+)?today\b/gi, () => after(today, "Modified today"));
  take(/\b(?:modified\s+|from\s+)?yesterday\b/gi, () => {
    after(today - DAY, "Modified yesterday");
    out.modifiedBefore = today;
  });
  take(/\b(?:from\s+|modified\s+)?this\s+week\b/gi, () => {
    const dow = (now.getDay() + 6) % 7; // Monday = 0
    after(today - dow * DAY, "Modified this week");
  });
  take(/\b(?:from\s+|modified\s+|in\s+)?(?:the\s+)?(?:last|past|previous)\s+week\b/gi, () => after(now.getTime() - 7 * DAY, "Modified in the past 7 days"));
  take(/\b(?:from\s+|modified\s+)?this\s+month\b/gi, () => after(new Date(now.getFullYear(), now.getMonth(), 1).getTime(), "Modified this month"));
  take(/\b(?:from\s+|modified\s+|in\s+)?(?:the\s+)?(?:last|past|previous)\s+month\b/gi, () => after(now.getTime() - 30 * DAY, "Modified in the past 30 days"));
  take(/\b(?:from\s+|modified\s+)?this\s+year\b/gi, () => after(new Date(now.getFullYear(), 0, 1).getTime(), "Modified this year"));
  take(/\b(?:from\s+|modified\s+|in\s+)?(?:the\s+)?(?:last|previous)\s+year\b/gi, () => {
    after(new Date(now.getFullYear() - 1, 0, 1).getTime(), `Modified in ${now.getFullYear() - 1}`);
    out.modifiedBefore = new Date(now.getFullYear(), 0, 1).getTime();
  });
  take(/\b(before|after|since)\s+(\d{4})-(\d{2})-(\d{2})\b/gi, (m) => {
    const d = new Date(Number(m[2]), Number(m[3]) - 1, Number(m[4])).getTime();
    if (m[1]!.toLowerCase() === "before") before(d, `Modified before ${formatDay(d)}`);
    else after(m[1]!.toLowerCase() === "after" ? d + DAY : d, `Modified ${m[1]!.toLowerCase()} ${formatDay(d)}`);
  });
  take(/\b(before|after|since|in|from)\s+((?:19|20)\d{2})\b/gi, (m) => {
    const year = Number(m[2]);
    const start = new Date(year, 0, 1).getTime();
    const end = new Date(year + 1, 0, 1).getTime();
    const word = m[1]!.toLowerCase();
    if (word === "before") before(start, `Modified before ${year}`);
    else if (word === "after") after(end, `Modified after ${year}`);
    else if (word === "since") after(start, `Modified since ${year}`);
    else {
      after(start, `Modified in ${year}`);
      out.modifiedBefore = end;
    }
  });

  // Folders only?
  take(/\b(folders?|directories|directory)\b/gi, () => {
    if (out.kind !== "folders") out.chips.push("Folders only");
    out.kind = "folders";
  });

  // Extensions: "*.pdf", ".pdf", "pdf files", "pdfs".
  const addExt = (ext: string) => {
    const e = ext.toLowerCase();
    if (!out.extensions.includes(e)) {
      out.extensions.push(e);
      out.chips.push(`Type: .${e}`);
    }
  };
  take(/(?:^|\s)\*?\.([a-z0-9]{1,8})(?=\s)/gi, (m) => addExt(m[1]!));
  // A bare word counts as a file type only when it is clearly meant as one: followed by
  // "file(s)", written in capitals ("PDF"), or in plural ("pdfs"). "server log" stays a name.
  const tokens = s.split(/\s+/).filter(Boolean);
  const kept: string[] = [];
  tokens.forEach((word, i) => {
    const lower = word.toLowerCase();
    const singular = lower.endsWith("s") && KNOWN_EXTENSIONS.has(lower.slice(0, -1)) ? lower.slice(0, -1) : null;
    const next = tokens[i + 1]?.toLowerCase();
    const isExt = KNOWN_EXTENSIONS.has(lower) && (next === "file" || next === "files" || (word === word.toUpperCase() && /[A-Z]/.test(word)));
    if (isExt) addExt(lower);
    else if (singular) addExt(singular);
    else kept.push(word);
  });
  s = ` ${kept.join(" ")} `;

  // Categories.
  for (const word of s.split(/\s+/)) {
    const cat = CATEGORY_WORDS[word.toLowerCase()];
    if (cat) {
      if (!out.categories.includes(cat)) {
        out.categories.push(cat);
        out.chips.push(`Type: ${CATEGORY_LABELS[cat]}`);
      }
      s = s.replace(new RegExp(`\\b${word}\\b`), " ");
    }
  }

  if (/\bfiles?\b/i.test(s) && out.kind === "any" && out.extensions.length === 0 && out.categories.length === 0) out.kind = "files";

  const leftovers = s
    .split(/\s+/)
    .filter((w) => w && /[\p{L}\p{N}*?]/u.test(w) && !STOPWORDS.has(w.toLowerCase()));
  const text = [...nameParts, ...leftovers].join(" ").trim();
  if (text) {
    out.text = text;
    out.chips.unshift(text.includes("*") || text.includes("?") ? `Name matches ${text}` : `Name contains "${text}"`);
  }
  return out;
}
