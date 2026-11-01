// First-start tour: persisted state and the step definitions. The tour points at real UI
// elements marked with `data-tour="…"`; nothing here touches files.

/**
 * Bump when the interface changes enough that existing users should be offered the tour
 * again. Stored separately from preferences, so a new version never resets settings.
 */
export const TOUR_VERSION = 1;

const STORAGE_KEY = "nexdir.onboarding.v1";

export type TourStatus = "offered" | "skipped" | "completed";

export interface OnboardingState {
  /** Tour version the status refers to. */
  version: number;
  status: TourStatus;
  /** ISO time the welcome was first shown for this version. */
  offeredAt: string;
  /** ISO time of the last skip or completion. */
  decidedAt?: string;
}

export function loadOnboarding(): OnboardingState | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<OnboardingState>;
    if (typeof parsed.version !== "number" || !["offered", "skipped", "completed"].includes(parsed.status as string)) return null;
    return parsed as OnboardingState;
  } catch {
    return null;
  }
}

export function saveOnboarding(state: OnboardingState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage unavailable: the welcome may simply be offered again next time.
  }
}

/** Whether the welcome should appear on start. Only a skip or completion of the current version stops it. */
export function shouldOfferTour(state: OnboardingState | null, version = TOUR_VERSION): boolean {
  return !state || state.version < version || state.status === "offered";
}

export function recordTour(status: TourStatus, now = new Date(), version = TOUR_VERSION): OnboardingState {
  const previous = loadOnboarding();
  const offeredAt = previous && previous.version === version ? previous.offeredAt : now.toISOString();
  const state: OnboardingState = { version, status, offeredAt, ...(status !== "offered" ? { decidedAt: now.toISOString() } : {}) };
  saveOnboarding(state);
  return state;
}

export interface TourStep {
  id: string;
  title: string;
  /** Short paragraphs. */
  body: string[];
  /** Bullet points shown under the text. */
  points?: string[];
  /** `data-tour` values of the elements to highlight. Missing or hidden ones are left out. */
  targets: string[];
}

export const TOUR_STEPS: TourStep[] = [
  {
    id: "navigation",
    title: "Find your way around",
    body: ["The sidebar takes you to Home, your quick-access folders, pinned folders and every drive. Above the files, the address bar shows where you are."],
    points: [
      "Click a part of the path to jump there, or click the empty area to type a path (Ctrl+L).",
      "Back, forward and up work with Alt+← / Alt+→ / Alt+↑ and the mouse side buttons.",
      "Pin folders you use often with Ctrl+D.",
    ],
    targets: ["sidebar", "address"],
  },
  {
    id: "tabs",
    title: "Tabs and split view",
    body: ["Work in several folders at once. Each tab remembers its own history, and Nexdir restores your tabs on the next start."],
    points: [
      "New tab: Ctrl+T or the + button. Switch with Ctrl+Tab, close with Ctrl+W.",
      "Middle-click a folder to open it in a background tab.",
      "Split view (Ctrl+\\) shows two folders side by side; F6 switches between them. Click the button again to go back to one pane.",
    ],
    targets: ["tabs", "split"],
  },
  {
    id: "files",
    title: "Copy, move and rename safely",
    body: [
      "Use the toolbar, the right-click menu, the keyboard (Ctrl+C, Ctrl+X, Ctrl+V, F2, Del) or drag and drop. Copies run in the background — the activity button shows progress and lets you cancel.",
    ],
    points: [
      "If a name already exists, Nexdir asks: keep both, skip, or replace.",
      "Replacing goes through a verified temporary copy, so a cancelled or failed copy never destroys the existing file.",
      "Deleting always moves items to the Recycle Bin — Nexdir never deletes permanently.",
    ],
    targets: ["file-ops", "operations"],
  },
  {
    id: "tools",
    title: "Smart tools",
    body: ["Everything runs on your PC — no cloud, no uploads."],
    points: [
      "Search understands plain requests like “videos larger than 2 GB in Downloads”.",
      "Storage analysis shows what fills a drive or folder.",
      "Duplicate files finds identical copies and always keeps at least one.",
      "Organize moves files by rules, with a preview before anything changes.",
    ],
    targets: ["tools"],
  },
  {
    id: "personalize",
    title: "Make it yours",
    body: ["Switch between system, light and dark theme, details or icon view, show hidden items and turn on the preview pane. Nexdir remembers your choices."],
    points: ["Press F1 at any time for all keyboard shortcuts.", "Help & Support has offline help, the tour, and ways to report a bug."],
    targets: ["appearance", "help"],
  },
];
