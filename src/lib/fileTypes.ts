import type { FileEntry } from "../types/fs";

export type FileCategory =
  | "folder"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "code"
  | "document"
  | "spreadsheet"
  | "presentation"
  | "pdf"
  | "text"
  | "executable"
  | "link"
  | "file";

const CATEGORY_BY_EXTENSION: Record<string, FileCategory> = {};
const register = (category: FileCategory, extensions: string) => {
  for (const ext of extensions.split(" ")) CATEGORY_BY_EXTENSION[ext] = category;
};
register("image", "jpg jpeg png gif bmp webp tif tiff svg ico heic heif avif raw cr2 nef psd");
register("video", "mp4 mkv avi mov wmv webm flv m4v mpg mpeg 3gp ts");
register("audio", "mp3 wav flac aac ogg m4a wma opus aiff mid midi");
register("archive", "zip rar 7z tar gz bz2 xz zst cab iso tgz");
register("code", "js jsx ts tsx rs py java c h cpp hpp cs go rb php swift kt html css scss json yaml yml toml xml sh ps1 bat cmd sql lua");
register("document", "doc docx odt rtf");
register("spreadsheet", "xls xlsx ods csv");
register("presentation", "ppt pptx odp");
register("pdf", "pdf");
register("text", "txt md log ini cfg conf");
register("executable", "exe msi com scr appx msix");
register("link", "lnk url");

const LABELS: Record<FileCategory, string> = {
  folder: "File folder",
  image: "Image",
  video: "Video",
  audio: "Audio",
  archive: "Archive",
  code: "Source code",
  document: "Document",
  spreadsheet: "Spreadsheet",
  presentation: "Presentation",
  pdf: "PDF document",
  text: "Text document",
  executable: "Application",
  link: "Shortcut",
  file: "File",
};

export function categoryOf(entry: Pick<FileEntry, "kind" | "extension">): FileCategory {
  if (entry.kind === "dir" || entry.kind === "dirLink" || entry.kind === "junction") return "folder";
  return (entry.extension && CATEGORY_BY_EXTENSION[entry.extension]) || "file";
}

/** Human-readable type column text, e.g. "PDF document" or "MKV file". */
export function typeLabel(entry: Pick<FileEntry, "kind" | "extension">): string {
  if (entry.kind === "junction") return "Junction";
  if (entry.kind === "dirLink") return "Folder link";
  if (entry.kind === "fileLink") return "Symbolic link";
  const category = categoryOf(entry);
  if (category === "folder") return LABELS.folder;
  if (category === "file" || category === "code" || category === "archive") {
    return entry.extension ? `${entry.extension.toUpperCase()} file` : "File";
  }
  return LABELS[category];
}

export function isNavigable(entry: Pick<FileEntry, "kind">): boolean {
  return entry.kind === "dir" || entry.kind === "dirLink" || entry.kind === "junction";
}

const EXECUTABLE = new Set(
  "exe com bat cmd msi msp msix msixbundle appx appxbundle ps1 psm1 vbs vbe js jse wsf wsh hta scr cpl pif reg lnk url jar application gadget msc inf scf appref-ms".split(
    " ",
  ),
);

/** Mirrors the backend list: these need a confirmation before they are opened. */
export function isExecutable(entry: Pick<FileEntry, "extension">): boolean {
  return !!entry.extension && EXECUTABLE.has(entry.extension);
}
