// Pure selection logic (click, Ctrl+click, Shift+click, keyboard), kept free of React
// so it can be unit-tested.

export interface Selection {
  /** Selected item paths. */
  selected: ReadonlySet<string>;
  /** Item with keyboard focus. */
  focus: string | null;
  /** Anchor for Shift range selection. */
  anchor: string | null;
}

export const emptySelection: Selection = { selected: new Set(), focus: null, anchor: null };

export interface ClickModifiers {
  ctrl: boolean;
  shift: boolean;
}

export function clickSelect(sel: Selection, order: readonly string[], path: string, mods: ClickModifiers): Selection {
  if (mods.shift && sel.anchor && order.includes(sel.anchor)) {
    const range = rangeBetween(order, sel.anchor, path);
    const selected = mods.ctrl ? new Set([...sel.selected, ...range]) : new Set(range);
    return { selected, focus: path, anchor: sel.anchor };
  }
  if (mods.ctrl) {
    const selected = new Set(sel.selected);
    if (selected.has(path)) selected.delete(path);
    else selected.add(path);
    return { selected, focus: path, anchor: path };
  }
  return { selected: new Set([path]), focus: path, anchor: path };
}

function rangeBetween(order: readonly string[], a: string, b: string): string[] {
  const i = order.indexOf(a);
  const j = order.indexOf(b);
  const [from, to] = i <= j ? [i, j] : [j, i];
  return order.slice(from, to + 1);
}

/** Moves focus by `delta` items (or to an absolute index with `absolute`). */
export function moveFocus(
  sel: Selection,
  order: readonly string[],
  delta: number,
  opts: { shift: boolean; ctrl: boolean; absolute?: number },
): Selection {
  if (order.length === 0) return sel;
  const current = sel.focus ? order.indexOf(sel.focus) : -1;
  let next = opts.absolute ?? (current < 0 ? (delta > 0 ? 0 : order.length - 1) : current + delta);
  next = Math.max(0, Math.min(order.length - 1, next));
  const path = order[next]!;
  if (opts.shift) {
    const anchor = sel.anchor && order.includes(sel.anchor) ? sel.anchor : path;
    return { selected: new Set(rangeBetween(order, anchor, path)), focus: path, anchor };
  }
  if (opts.ctrl) return { ...sel, focus: path };
  return { selected: new Set([path]), focus: path, anchor: path };
}

export function selectAll(order: readonly string[]): Selection {
  return { selected: new Set(order), focus: order[0] ?? null, anchor: order[0] ?? null };
}

/** Drops paths that are no longer listed (after a refresh). */
export function pruneSelection(sel: Selection, order: readonly string[]): Selection {
  const present = new Set(order);
  const selected = new Set([...sel.selected].filter((p) => present.has(p)));
  return {
    selected,
    focus: sel.focus && present.has(sel.focus) ? sel.focus : null,
    anchor: sel.anchor && present.has(sel.anchor) ? sel.anchor : null,
  };
}
