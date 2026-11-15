import { useEffect, useRef, useState } from "react";
import { Tool, ToolStats, containedRect, seriesVar } from "../lib/measure";

interface MeasureOverlayProps {
  tools: Tool[];
  stats: Record<number, ToolStats | null>;
  selectedId: number | null;
  /** Tool being drawn, or null. While set, the overlay captures the pointer. */
  drawMode: "box" | "line" | null;
  onDrawn: (kind: "box" | "line", x0: number, y0: number, x1: number, y1: number) => void;
  onCancelDraw: () => void;
  containerW: number;
  containerH: number;
  sensorW: number;
  sensorH: number;
}

/** Boxes and lines on the thermal image, with their labels; draws new ones. */
export function MeasureOverlay({
  tools,
  stats,
  selectedId,
  drawMode,
  onDrawn,
  onCancelDraw,
  containerW,
  containerH,
  sensorW,
  sensorH,
}: MeasureOverlayProps) {
  const [draft, setDraft] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!drawMode) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setDraft(null);
        onCancelDraw();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [drawMode, onCancelDraw]);

  if (containerW === 0 || containerH === 0 || sensorW === 0) return null;
  const r = containedRect(containerW, containerH, sensorW, sensorH);

  const toSensor = (e: React.PointerEvent) => {
    const box = rootRef.current!.getBoundingClientRect();
    const x = Math.floor(((e.clientX - box.left - r.left) / r.width) * sensorW);
    const y = Math.floor(((e.clientY - box.top - r.top) / r.height) * sensorH);
    return { x: Math.max(0, Math.min(sensorW - 1, x)), y: Math.max(0, Math.min(sensorH - 1, y)) };
  };

  // Sensor pixel -> container px (pixel edges for boxes, centers for lines).
  const px = (x: number) => r.left + x * r.scale;
  const py = (y: number) => r.top + y * r.scale;

  const shapes = [...tools.map((t) => ({ ...t, draft: false })), ...(draft && drawMode
    ? [{ id: -1, kind: drawMode, label: "", slot: 0, ...draft, draft: true }]
    : [])];

  return (
    <div
      ref={rootRef}
      className={`measure-overlay ${drawMode ? "drawing" : ""}`}
      onPointerDown={(e) => {
        if (!drawMode || e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();
        (e.currentTarget as HTMLDivElement).setPointerCapture(e.pointerId);
        const p = toSensor(e);
        setDraft({ x0: p.x, y0: p.y, x1: p.x, y1: p.y });
      }}
      onPointerMove={(e) => {
        if (!drawMode || !draft) return;
        const p = toSensor(e);
        setDraft({ ...draft, x1: p.x, y1: p.y });
      }}
      onPointerUp={(e) => {
        if (!drawMode || !draft) return;
        (e.currentTarget as HTMLDivElement).releasePointerCapture(e.pointerId);
        const d = draft;
        setDraft(null);
        // Ignore plain clicks: a box needs 2x2 px, a line 2 px length.
        if (Math.abs(d.x1 - d.x0) + Math.abs(d.y1 - d.y0) >= 2) onDrawn(drawMode, d.x0, d.y0, d.x1, d.y1);
      }}
    >
      <svg className="measure-svg" width={containerW} height={containerH}>
        {shapes.map((t) => {
          const color = t.draft ? "#ffffff" : seriesVar(t.slot);
          const sel = t.id === selectedId;
          if (t.kind === "box") {
            const x = px(Math.min(t.x0, t.x1));
            const y = py(Math.min(t.y0, t.y1));
            const w = (Math.abs(t.x1 - t.x0) + 1) * r.scale;
            const h = (Math.abs(t.y1 - t.y0) + 1) * r.scale;
            return (
              <g key={t.id}>
                <rect x={x} y={y} width={w} height={h} className="measure-halo" />
                <rect
                  x={x}
                  y={y}
                  width={w}
                  height={h}
                  fill="none"
                  stroke={color}
                  strokeWidth={sel ? 3 : 2}
                  strokeDasharray={t.draft ? "4 3" : undefined}
                />
              </g>
            );
          }
          const c = r.scale / 2;
          const [x1, y1, x2, y2] = [px(t.x0) + c, py(t.y0) + c, px(t.x1) + c, py(t.y1) + c];
          return (
            <g key={t.id}>
              <line x1={x1} y1={y1} x2={x2} y2={y2} className="measure-halo" />
              <line
                x1={x1}
                y1={y1}
                x2={x2}
                y2={y2}
                stroke={color}
                strokeWidth={sel ? 3 : 2}
                strokeLinecap="round"
                strokeDasharray={t.draft ? "4 3" : undefined}
              />
              {!t.draft && (
                <>
                  <circle cx={x1} cy={y1} r={3} fill={color} className="measure-end" />
                  <circle cx={x2} cy={y2} r={3} fill={color} className="measure-end" />
                </>
              )}
            </g>
          );
        })}
      </svg>
      {tools.map((t) => {
        const s = stats[t.id];
        // Label at the top-left of a box / at the start of a line.
        const lx = t.kind === "box" ? px(Math.min(t.x0, t.x1)) : px(t.x0);
        const ly = t.kind === "box" ? py(Math.min(t.y0, t.y1)) : py(t.y0);
        return (
          <div key={t.id} className="measure-label" style={{ left: lx, top: ly }}>
            <span className="measure-swatch" style={{ background: seriesVar(t.slot) }} />
            {t.label}
            {s && <span className="measure-label-value">{s.max.toFixed(1)}&deg;</span>}
          </div>
        );
      })}
    </div>
  );
}
