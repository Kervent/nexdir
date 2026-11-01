import { useCallback, useEffect, useState } from "react";
import type { SortSpec } from "../../lib/sort";

export type ThemePreference = "system" | "light" | "dark";
export type ViewMode = "details" | "icons";

export interface PinnedFolder {
  path: string;
  label: string;
}

export interface Preferences {
  theme: ThemePreference;
  viewMode: ViewMode;
  showHidden: boolean;
  sort: SortSpec;
  pinned: PinnedFolder[];
  showPreview: boolean;
}

export const defaultPreferences: Preferences = {
  theme: "system",
  viewMode: "details",
  showHidden: false,
  showPreview: false,
  sort: { key: "name", direction: "asc" },
  pinned: [],
};

const STORAGE_KEY = "nexdir.preferences.v1";

// UI preferences live in WebView storage. They are conveniences only; losing them
// never affects files. A backend settings store follows with the settings feature.
export function loadPreferences(): Preferences {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultPreferences;
    const parsed = JSON.parse(raw) as Partial<Preferences>;
    return { ...defaultPreferences, ...parsed, sort: { ...defaultPreferences.sort, ...parsed.sort } };
  } catch {
    return defaultPreferences;
  }
}

function savePreferences(prefs: Preferences) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // Storage unavailable: preferences simply aren't persisted.
  }
}

export function usePreferences() {
  const [prefs, setPrefs] = useState<Preferences>(loadPreferences);

  useEffect(() => savePreferences(prefs), [prefs]);

  useEffect(() => {
    const root = document.documentElement;
    if (prefs.theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", prefs.theme);
  }, [prefs.theme]);

  const update = useCallback((patch: Partial<Preferences>) => setPrefs((p) => ({ ...p, ...patch })), []);
  return [prefs, update] as const;
}
