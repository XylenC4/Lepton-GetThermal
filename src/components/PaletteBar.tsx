import { useCallback, useRef, useState } from "react";

export interface DisplayRange {
  min: number; // raw Y16
  max: number; // raw Y16
}

interface PaletteBarProps {
  minVal: number; // scene min (raw Y16)
  maxVal: number; // scene max (raw Y16)
  palette: string;
  /** Fixed color-mapping range, or null for automatic (scene min/max). */
  range: DisplayRange | null;
  onRangeChange: (range: DisplayRange | null) => void;
}

// Sampled from the actual Rust LUT data at key indices to match the real colorization.
// [color, position % from the bottom]
const paletteStops: Record<string, [string, number][]> = {
  ironblack: [
    ["rgb(255,255,255)", 0],
    ["rgb(129,129,129)", 25],
    ["rgb(0,0,0)", 50],
    ["rgb(31,0,118)", 56],
    ["rgb(207,34,95)", 70],
    ["rgb(243,141,13)", 82],
    ["rgb(255,224,30)", 93],
    ["rgb(255,255,24)", 100],
  ],
  rainbow: [
    ["rgb(1,3,74)", 0],
    ["rgb(0,61,154)", 12.5],
    ["rgb(0,113,223)", 25],
    ["rgb(67,171,60)", 37.5],
    ["rgb(212,215,3)", 50],
    ["rgb(253,174,19)", 62.5],
    ["rgb(251,33,59)", 75],
    ["rgb(251,101,112)", 87.5],
    ["rgb(255,233,208)", 100],
  ],
  grayscale: [
    ["rgb(0,0,0)", 0],
    ["rgb(255,255,255)", 100],
  ],
};

/** Raw units per °C: TLinear 0.01 K (raw > 10000) or 0.1 K. */
function unitsPerDegree(raw: number): number {
  return raw > 10000 ? 100 : 10;
}

/** Convert raw Y16 value to Celsius.
 *  Values > 10000 are in centikelvins (0.01K), otherwise decikelvins (0.1K). */
function rawToCelsius(raw: number): string {
  if (raw > 10000) {
    return ((raw - 27315) / 100).toFixed(1);
  }
  return ((raw - 2731.5) / 10).toFixed(1);
}

type Handle = "min" | "max";
interface Axis {
  lo: number;
  hi: number;
}

/** Axis spans the scene and the limits, with some headroom to drag into. */
function computeAxis(sceneMin: number, sceneMax: number, range: DisplayRange | null): Axis {
  const lo = Math.min(sceneMin, range?.min ?? sceneMin);
  const hi = Math.max(sceneMax, range?.max ?? sceneMax);
  const pad = Math.max((hi - lo) * 0.1, 2 * unitsPerDegree(hi));
  return { lo: lo - pad, hi: hi + pad };
}

export function PaletteBar({ minVal, maxVal, palette, range, onRangeChange }: PaletteBarProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ handle: Handle; axis: Axis } | null>(null);

  const upd = unitsPerDegree(maxVal);
  const step = upd / 10; // 0.1 °C
  const minGap = upd / 2; // keep limits at least 0.5 °C apart

  const active: DisplayRange = range ?? { min: minVal, max: maxVal };
  // Freeze the axis while dragging so the handle doesn't drift under the pointer.
  const axis = drag?.axis ?? computeAxis(minVal, maxVal, range);
  const span = Math.max(axis.hi - axis.lo, 1);
  const pct = (v: number) => Math.min(100, Math.max(0, ((v - axis.lo) / span) * 100));

  const stops = paletteStops[palette] ?? paletteStops.ironblack;
  const gradient = `linear-gradient(to top, ${stops.map(([c, p]) => `${c} ${p}%`).join(", ")})`;
  const colorLo = stops[0][0];
  const colorHi = stops[stops.length - 1][0];

  const setLimit = useCallback(
    (handle: Handle, raw: number, base: DisplayRange) => {
      const v = Math.round(raw / step) * step;
      const next =
        handle === "max"
          ? { min: base.min, max: Math.max(v, base.min + minGap) }
          : { min: Math.min(v, base.max - minGap), max: base.max };
      onRangeChange({ min: Math.max(0, next.min), max: Math.min(65535, next.max) });
    },
    [onRangeChange, step, minGap]
  );

  const rawAtPointer = (clientY: number, ax: Axis) => {
    const rect = trackRef.current!.getBoundingClientRect();
    const t = 1 - (clientY - rect.top) / rect.height;
    return ax.lo + Math.min(1, Math.max(0, t)) * (ax.hi - ax.lo);
  };

  const onPointerDown = (e: React.PointerEvent, handle?: Handle) => {
    if (e.button !== 0 || !trackRef.current) return;
    e.preventDefault();
    const ax = axis;
    const raw = rawAtPointer(e.clientY, ax);
    // Clicking the bar itself grabs the nearest limit and moves it there.
    const h = handle ?? (Math.abs(raw - active.max) < Math.abs(raw - active.min) ? "max" : "min");
    trackRef.current.setPointerCapture(e.pointerId);
    setDrag({ handle: h, axis: ax });
    if (!handle) setLimit(h, raw, active);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag) return;
    setLimit(drag.handle, rawAtPointer(e.clientY, drag.axis), active);
  };

  const endDrag = (e: React.PointerEvent) => {
    if (!drag) return;
    trackRef.current?.releasePointerCapture(e.pointerId);
    setDrag(null);
  };

  const onKeyDown = (e: React.KeyboardEvent, handle: Handle) => {
    const mult = e.shiftKey ? 10 : 1;
    let delta = 0;
    if (e.key === "ArrowUp" || e.key === "ArrowRight") delta = step * mult;
    else if (e.key === "ArrowDown" || e.key === "ArrowLeft") delta = -step * mult;
    else if (e.key === "Escape" || e.key === "Delete") {
      onRangeChange(null);
      e.preventDefault();
      return;
    } else return;
    e.preventDefault();
    setLimit(handle, active[handle] + delta, active);
  };

  const manual = range !== null;
  const pLo = pct(active.min);
  const pHi = pct(active.max);

  const handleEl = (h: Handle, p: number) => (
    <div
      className={`scale-handle ${h} ${drag?.handle === h ? "dragging" : ""}`}
      style={{ bottom: `${p}%` }}
      role="slider"
      tabIndex={0}
      aria-label={h === "max" ? "Upper limit" : "Lower limit"}
      aria-valuenow={Number(rawToCelsius(active[h]))}
      aria-valuetext={`${rawToCelsius(active[h])} °C`}
      onPointerDown={(e) => {
        e.stopPropagation();
        onPointerDown(e, h);
      }}
      onKeyDown={(e) => onKeyDown(e, h)}
    />
  );

  return (
    <div className="palette-bar">
      <div className="scale">
        <div
          ref={trackRef}
          className={`scale-track ${manual ? "manual" : ""}`}
          onPointerDown={(e) => onPointerDown(e)}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onDoubleClick={() => onRangeChange(null)}
          title="Drag the limits to fix the range. Double-click for auto."
        >
          <div className="scale-fill" style={{ bottom: 0, height: `${pLo}%`, background: colorLo }} />
          <div
            className="scale-gradient"
            style={{ bottom: `${pLo}%`, height: `${Math.max(pHi - pLo, 0)}%`, background: gradient }}
          />
          <div className="scale-fill" style={{ bottom: `${pHi}%`, top: 0, background: colorHi }} />
          {manual && (
            <>
              <div
                className="scale-scene-tick"
                style={{ bottom: `${pct(minVal)}%` }}
                title={`Scene min ${rawToCelsius(minVal)} °C`}
              />
              <div
                className="scale-scene-tick"
                style={{ bottom: `${pct(maxVal)}%` }}
                title={`Scene max ${rawToCelsius(maxVal)} °C`}
              />
            </>
          )}
          {handleEl("max", pHi)}
          {handleEl("min", pLo)}
        </div>
        <div className="scale-labels">
          <span className="scale-label max" style={{ bottom: `${pHi}%` }}>
            {rawToCelsius(active.max)}&deg;
          </span>
          <span className="scale-label min" style={{ bottom: `${pLo}%` }}>
            {rawToCelsius(active.min)}&deg;
          </span>
        </div>
      </div>
      <button
        type="button"
        className={`scale-mode ${manual ? "" : "auto"}`}
        onClick={() => onRangeChange(manual ? null : { min: minVal, max: maxVal })}
        title={manual ? "Return to automatic range" : "Lock the current range"}
      >
        {manual ? "Auto" : "Auto ✓"}
      </button>
    </div>
  );
}
