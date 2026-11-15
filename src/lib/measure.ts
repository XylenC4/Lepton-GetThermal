import { RawFrame } from "../hooks/useFrameStream";

/** Measurement tool on the image, in sensor pixel coordinates (inclusive). */
export interface Tool {
  id: number;
  kind: "box" | "line";
  label: string; // "B1", "L2", …
  slot: number; // color slot 1..MAX_TOOLS, fixed for the tool's lifetime
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface ToolStats {
  min: number; // °C
  max: number;
  avg: number;
  /** Line tools: temperature along the line, in drawing order. */
  profile?: number[];
}

/** Categorical color slots (CSS vars --series-N, validated in light and dark). */
export const MAX_TOOLS = 6;

export const seriesVar = (slot: number) => `var(--series-${slot})`;

/** Raw Y16 to °C: TLinear 0.01 K (> 10000) or 0.1 K. */
export function rawToC(raw: number): number {
  return raw > 10000 ? (raw - 27315) / 100 : (raw - 2731.5) / 10;
}

/** Image rectangle of an object-fit: contain canvas inside its container. */
export function containedRect(cw: number, ch: number, iw: number, ih: number) {
  const scale = Math.min(cw / iw, ch / ih);
  const width = iw * scale;
  const height = ih * scale;
  return { left: (cw - width) / 2, top: (ch - height) / 2, width, height, scale };
}

/** Pixels on the line from (x0,y0) to (x1,y1), Bresenham. */
export function linePixels(x0: number, y0: number, x1: number, y1: number): [number, number][] {
  const pts: [number, number][] = [];
  const dx = Math.abs(x1 - x0);
  const dy = -Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  let x = x0;
  let y = y0;
  for (;;) {
    pts.push([x, y]);
    if (x === x1 && y === y1) break;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y += sy;
    }
  }
  return pts;
}

export function toolStats(raw: RawFrame, t: Tool): ToolStats | null {
  const { width: w, height: h, data } = raw;
  const clampX = (v: number) => Math.max(0, Math.min(w - 1, v));
  const clampY = (v: number) => Math.max(0, Math.min(h - 1, v));
  let min = Infinity;
  let max = -Infinity;
  let sum = 0;
  let n = 0;
  const take = (x: number, y: number) => {
    const v = data[y * w + x];
    if (v === undefined) return undefined;
    if (v < min) min = v;
    if (v > max) max = v;
    sum += v;
    n++;
    return v;
  };

  if (t.kind === "box") {
    const xa = clampX(Math.min(t.x0, t.x1));
    const xb = clampX(Math.max(t.x0, t.x1));
    const ya = clampY(Math.min(t.y0, t.y1));
    const yb = clampY(Math.max(t.y0, t.y1));
    for (let y = ya; y <= yb; y++) for (let x = xa; x <= xb; x++) take(x, y);
    if (n === 0) return null;
    return { min: rawToC(min), max: rawToC(max), avg: rawToC(sum / n) };
  }

  const profile: number[] = [];
  for (const [x, y] of linePixels(clampX(t.x0), clampY(t.y0), clampX(t.x1), clampY(t.y1))) {
    const v = take(x, y);
    if (v !== undefined) profile.push(rawToC(v));
  }
  if (n === 0) return null;
  return { min: rawToC(min), max: rawToC(max), avg: rawToC(sum / n), profile };
}

/** One trend sample: time (ms since epoch) + stats per tool id. */
export interface TrendSample {
  t: number;
  stats: Record<number, { min: number; max: number; avg: number }>;
}

/** Wide CSV: time, then min/max/avg per tool. */
export function trendCsv(samples: TrendSample[], tools: Tool[]): string {
  const head = ["time_iso", "t_s", ...tools.flatMap((t) => [`${t.label}_min_C`, `${t.label}_max_C`, `${t.label}_avg_C`])];
  const t0 = samples[0]?.t ?? 0;
  const rows = samples.map((s) => [
    new Date(s.t).toISOString(),
    ((s.t - t0) / 1000).toFixed(2),
    ...tools.flatMap((t) => {
      const v = s.stats[t.id];
      return v ? [v.min.toFixed(2), v.max.toFixed(2), v.avg.toFixed(2)] : ["", "", ""];
    }),
  ]);
  return [head, ...rows].map((r) => r.join(",")).join("\n") + "\n";
}
