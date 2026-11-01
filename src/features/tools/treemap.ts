// Squarified treemap layout (Bruls, Huizing & van Wijk): keeps tiles close to square so
// sizes stay comparable at a glance.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

function worst(row: number[], side: number, scale: number): number {
  const sum = row.reduce((a, b) => a + b, 0) * scale;
  const max = Math.max(...row) * scale;
  const min = Math.min(...row) * scale;
  const s2 = side * side;
  return Math.max((s2 * max) / (sum * sum), (sum * sum) / (s2 * min));
}

/** Lays out positive `values` (sorted descending for best results) inside `bounds`. */
export function squarify(values: number[], bounds: Rect): Rect[] {
  const total = values.reduce((a, b) => a + b, 0);
  const out: Rect[] = new Array(values.length);
  if (total <= 0 || bounds.w <= 0 || bounds.h <= 0) return values.map(() => ({ x: bounds.x, y: bounds.y, w: 0, h: 0 }));
  const scale = (bounds.w * bounds.h) / total;
  let { x, y, w, h } = bounds;
  let i = 0;
  while (i < values.length) {
    const side = Math.min(w, h);
    const row = [values[i]!];
    let j = i + 1;
    while (j < values.length && worst([...row, values[j]!], side, scale) <= worst(row, side, scale)) {
      row.push(values[j]!);
      j++;
    }
    const rowArea = row.reduce((a, b) => a + b, 0) * scale;
    if (w >= h) {
      // Column on the left.
      const colW = rowArea / h;
      let cy = y;
      row.forEach((v, k) => {
        const rh = (v * scale) / colW;
        out[i + k] = { x, y: cy, w: colW, h: rh };
        cy += rh;
      });
      x += colW;
      w -= colW;
    } else {
      const rowH = rowArea / w;
      let cx = x;
      row.forEach((v, k) => {
        const rw = (v * scale) / rowH;
        out[i + k] = { x: cx, y, w: rw, h: rowH };
        cx += rw;
      });
      y += rowH;
      h -= rowH;
    }
    i = j;
  }
  return out;
}
