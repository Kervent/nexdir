import {
  AppWindow,
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileType,
  FileVideo,
  Folder,
  FolderSymlink,
  Link2,
  Presentation,
  type LucideIcon,
} from "lucide-react";
import { categoryOf, type FileCategory } from "../lib/fileTypes";
import type { FileEntry } from "../types/fs";

const ICONS: Record<FileCategory, { icon: LucideIcon; color: string }> = {
  folder: { icon: Folder, color: "var(--nx-icon-folder)" },
  image: { icon: FileImage, color: "var(--nx-icon-image)" },
  video: { icon: FileVideo, color: "var(--nx-icon-video)" },
  audio: { icon: FileAudio, color: "var(--nx-icon-audio)" },
  archive: { icon: FileArchive, color: "var(--nx-icon-archive)" },
  code: { icon: FileCode, color: "var(--nx-icon-code)" },
  document: { icon: FileText, color: "var(--nx-icon-doc)" },
  spreadsheet: { icon: FileSpreadsheet, color: "var(--nx-icon-image)" },
  presentation: { icon: Presentation, color: "var(--nx-icon-archive)" },
  pdf: { icon: FileType, color: "var(--nx-danger)" },
  text: { icon: FileText, color: "var(--nx-icon-file)" },
  executable: { icon: AppWindow, color: "var(--nx-icon-code)" },
  link: { icon: Link2, color: "var(--nx-icon-file)" },
  file: { icon: File, color: "var(--nx-icon-file)" },
};

export function FileIcon({ entry, size = 18 }: { entry: Pick<FileEntry, "kind" | "extension">; size?: number }) {
  const category = categoryOf(entry);
  const isLinkFolder = entry.kind === "junction" || entry.kind === "dirLink";
  const { icon: Icon, color } = isLinkFolder ? { icon: FolderSymlink, color: ICONS.folder.color } : ICONS[category];
  const filled = category === "folder";
  return (
    <Icon
      aria-hidden="true"
      size={size}
      color={color}
      fill={filled ? color : "none"}
      fillOpacity={filled ? 0.22 : undefined}
      strokeWidth={1.75}
      className="nx-file-icon"
    />
  );
}
