import { useMemo, useRef, useState } from "react";
import { useElementSize } from "../hooks/useElementSize";
import { seriesVar } from "../lib/measure";

export interface Series {
  key: string;
  label: string;
  slot: number; // color slot
  points: [number, number][]; // [x, y], x ascending
}

interface LineChartProps {
  title: string;
  series: Series[];
  xDomain: [number, number];
  formatX: (x: number) => string;
  /** Tooltip heading for a hovered x. */
  formatXLong?: (x: number) => string;
  yUnit: string;
  empty?: string;
}

const M = { top: 8, right: 44, bottom: 20, left: 40 };

/** "Nice" tick values covering [lo, hi]. */
function niceTicks(lo: number, hi: number, count: number): number[] {
  const span = Math.max(hi - lo, 1e-6);
  const raw = span / Math.max(count, 1);
  const pow = Math.pow(10, Math.floor(Math.log10(raw)));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * pow).find((s) => s >= raw) ?? raw;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) out.push(+v.toFixed(6));
  return out;
}

/**
 * Single-axis line chart (SVG): 2 px lines, recessive grid, legend for >= 2
 * series plus direct end labels for <= 4, crosshair + tooltip on hover.
 */
export function LineChart({ title, series, xDomain, formatX, formatXLong, yUnit, empty }: LineChartProps) {
  const boxRef = useRef<HTMLDivElement>(null);
  const { width, height } = useElementSize(boxRef);
  const [hoverX, setHoverX] = useState<number | null>(null);

  const visible = series.filter((s) => s.points.length > 0);
  const [yLo, yHi] = useMemo(() => {
    let lo = Infinity;
    let hi = -Infinity;
    for (const s of visible) for (const [, y] of s.points) {
      if (y < lo) lo = y;
      if (y > hi) hi = y;
    }
    if (!isFinite(lo)) return [0, 1];
    const pad = Math.max((hi - lo) * 0.1, 0.25);
    return [lo - pad, hi + pad];
  }, [visible]);

  const iw = Math.max(width - M.left - M.right, 10);
  const ih = Math.max(height - M.top - M.bottom, 10);
  const [x0, x1] = xDomain;
  const sx = (x: number) => M.left + ((x - x0) / Math.max(x1 - x0, 1e-9)) * iw;
  const sy = (y: number) => M.top + (1 - (y - yLo) / Math.max(yHi - yLo, 1e-9)) * ih;
  const yTicks = niceTicks(yLo, yHi, Math.max(2, Math.floor(ih / 28)));
  const xTicks = niceTicks(x0, x1, Math.max(2, Math.floor(iw / 80)));

  // Nearest point per series at the hovered x.
  const hover = useMemo(() => {
    if (hoverX === null) return null;
    const rows = visible.map((s) => {
      let best = s.points[0];
      for (const p of s.points) if (Math.abs(p[0] - hoverX) < Math.abs(best[0] - hoverX)) best = p;
      return { s, p: best };
    });
    return rows;
  }, [hoverX, visible]);

  const onMove = (e: React.MouseEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - r.left;
    if (x < M.left || x > M.left + iw) return setHoverX(null);
    setHoverX(x0 + ((x - M.left) / iw) * (x1 - x0));
  };

  const legend = visible.length >= 2;
  const endLabels = visible.length <= 4;
  const hx = hover?.[0] ? sx(hover[0].p[0]) : 0;

  return (
    <figure className="chart">
      <figcaption className="chart-head">
        <span className="chart-title">{title}</span>
        {legend && (
          <span className="chart-legend">
            {visible.map((s) => (
              <span key={s.key} className="chart-legend-item">
                <span className="chart-swatch" style={{ background: seriesVar(s.slot) }} />
                {s.label}
              </span>
            ))}
          </span>
        )}
      </figcaption>
      <div className="chart-plot" ref={boxRef}>
        {visible.length === 0 ? (
          <div className="chart-empty">{empty ?? "No data yet"}</div>
        ) : (
          width > 0 && (
            <svg width={width} height={height} onMouseMove={onMove} onMouseLeave={() => setHoverX(null)} role="img" aria-label={title}>
              {yTicks.map((t) => (
                <g key={`y${t}`}>
                  <line x1={M.left} x2={M.left + iw} y1={sy(t)} y2={sy(t)} className="chart-grid" />
                  <text x={M.left - 6} y={sy(t)} className="chart-tick" textAnchor="end" dominantBaseline="middle">
                    {t.toFixed(Math.abs(yHi - yLo) < 3 ? 1 : 0)}
                  </text>
                </g>
              ))}
              <text x={4} y={M.top} className="chart-tick" dominantBaseline="hanging">
                {yUnit}
              </text>
              {xTicks.map((t) => (
                <text key={`x${t}`} x={sx(t)} y={M.top + ih + 14} className="chart-tick" textAnchor="middle">
                  {formatX(t)}
                </text>
              ))}
              <line x1={M.left} x2={M.left + iw} y1={M.top + ih} y2={M.top + ih} className="chart-axis" />
              {visible.map((s) => (
                <polyline
                  key={s.key}
                  points={s.points.map(([x, y]) => `${sx(x).toFixed(1)},${sy(y).toFixed(1)}`).join(" ")}
                  fill="none"
                  stroke={seriesVar(s.slot)}
                  strokeWidth={2}
                  strokeLinejoin="round"
                  strokeLinecap="round"
                />
              ))}
              {endLabels &&
                visible.map((s) => {
                  const [lx, ly] = s.points[s.points.length - 1];
                  return (
                    <text key={`l${s.key}`} x={sx(lx) + 6} y={sy(ly)} className="chart-end-label" dominantBaseline="middle">
                      {s.label}
                    </text>
                  );
                })}
              {hover && (
                <g pointerEvents="none">
                  <line x1={hx} x2={hx} y1={M.top} y2={M.top + ih} className="chart-crosshair" />
                  {hover.map(({ s, p }) => (
                    <circle key={s.key} cx={sx(p[0])} cy={sy(p[1])} r={4} fill={seriesVar(s.slot)} className="chart-dot" />
                  ))}
                </g>
              )}
            </svg>
          )
        )}
        {hover && hover.length > 0 && (
          <div
            className="chart-tooltip"
            style={{
              left: hx > width / 2 ? undefined : hx + 12,
              right: hx > width / 2 ? width - hx + 12 : undefined,
            }}
          >
            <div className="chart-tooltip-head">{(formatXLong ?? formatX)(hover[0].p[0])}</div>
            {hover.map(({ s, p }) => (
              <div key={s.key} className="chart-tooltip-row">
                <span className="chart-swatch" style={{ background: seriesVar(s.slot) }} />
                <span>{s.label}</span>
                <span className="chart-tooltip-value">
                  {p[1].toFixed(1)} {yUnit}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </figure>
  );
}
