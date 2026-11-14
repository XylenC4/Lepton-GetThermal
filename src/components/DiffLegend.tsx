import { useRef, useState } from "react";
import { DiffSettings, DEFAULT_DIFF } from "./DiffControls";

interface DiffLegendProps {
  settings: DiffSettings;
  onChange: (s: DiffSettings) => void;
}

type Handle = "warmC" | "coldC" | "thresholdC";

const STEP = 0.1; // °C
const MIN_GAP = 0.2; // full scale stays at least this far beyond the threshold
const MAX_C = 50;

// Colors match processing/diff.rs.
const WARM_LO = "rgb(200,30,30)";
const WARM_HI = "rgb(255,235,60)";
const COLD_LO = "rgb(30,70,220)";
const COLD_HI = "rgb(90,235,255)";
const NEUTRAL = "rgb(115,115,115)";

const round = (v: number) => Math.round(v / STEP) * STEP;

/** Interactive scale for difference mode: drag the warm / cold full-scale
 *  limits and the noise threshold (edge of the gray band). */
export function DiffLegend({ settings, onChange }: DiffLegendProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ handle: Handle; axis: number } | null>(null);

  const { warmC, coldC, thresholdC } = settings;
  // Symmetric axis -axis..+axis (°C), with headroom; frozen while dragging.
  const axis = drag?.axis ?? Math.max(warmC, coldC, 1) * 1.3;
  const pct = (c: number) => Math.min(100, Math.max(0, ((c + axis) / (2 * axis)) * 100));

  const pCold = pct(-coldC);
  const pThrLo = pct(-thresholdC);
  const pThrHi = pct(thresholdC);
  const pWarm = pct(warmC);
  const gradient = `linear-gradient(to top,
    ${COLD_HI} 0%, ${COLD_HI} ${pCold}%, ${COLD_LO} ${pThrLo}%,
    ${NEUTRAL} ${pThrLo}%, ${NEUTRAL} ${pThrHi}%,
    ${WARM_LO} ${pThrHi}%, ${WARM_HI} ${pWarm}%, ${WARM_HI} 100%)`;

  const apply = (handle: Handle, valueC: number) => {
    const v = round(valueC);
    const s = { ...settings };
    if (handle === "warmC") s.warmC = Math.min(MAX_C, Math.max(v, s.thresholdC + MIN_GAP));
    else if (handle === "coldC") s.coldC = Math.min(MAX_C, Math.max(v, s.thresholdC + MIN_GAP));
    else s.thresholdC = Math.max(0, Math.min(Math.abs(v), Math.min(s.warmC, s.coldC) - MIN_GAP));
    s.warmC = round(s.warmC);
    s.coldC = round(s.coldC);
    s.thresholdC = round(s.thresholdC);
    onChange(s);
  };

  /** Pointer position -> signed °C on the axis. */
  const valueAt = (clientY: number, ax: number) => {
    const rect = trackRef.current!.getBoundingClientRect();
    const t = 1 - (clientY - rect.top) / rect.height;
    return (Math.min(1, Math.max(0, t)) * 2 - 1) * ax;
  };

  // Map a signed axis value to the value a handle stores.
  const handleValue = (h: Handle, signed: number) =>
    h === "warmC" ? signed : h === "coldC" ? -signed : Math.abs(signed);

  const nearest = (signed: number): Handle => {
    const cands: [Handle, number][] = [
      ["warmC", Math.abs(signed - warmC)],
      ["coldC", Math.abs(signed + coldC)],
      ["thresholdC", Math.abs(Math.abs(signed) - thresholdC)],
    ];
    return cands.sort((a, b) => a[1] - b[1])[0][0];
  };

  const onPointerDown = (e: React.PointerEvent, handle?: Handle) => {
    if (e.button !== 0 || !trackRef.current) return;
    e.preventDefault();
    const signed = valueAt(e.clientY, axis);
    const h = handle ?? nearest(signed);
    trackRef.current.setPointerCapture(e.pointerId);
    setDrag({ handle: h, axis });
    if (!handle) apply(h, handleValue(h, signed));
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag) return;
    apply(drag.handle, handleValue(drag.handle, valueAt(e.clientY, drag.axis)));
  };

  const endDrag = (e: React.PointerEvent) => {
    if (!drag) return;
    trackRef.current?.releasePointerCapture(e.pointerId);
    setDrag(null);
  };

  const onKeyDown = (e: React.KeyboardEvent, h: Handle) => {
    const mult = e.shiftKey ? 10 : 1;
    // Up always moves the handle up the scale; the cold handle sits below 0.
    const dir = e.key === "ArrowUp" || e.key === "ArrowRight" ? 1 : e.key === "ArrowDown" || e.key === "ArrowLeft" ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    const d = dir * STEP * mult;
    apply(h, h === "coldC" ? settings.coldC - d : settings[h] + d);
  };

  const handleEl = (h: Handle, p: number, label: string, extra = "") => (
    <div
      className={`scale-handle ${extra} ${drag?.handle === h ? "dragging" : ""}`}
      style={{ bottom: `${p}%` }}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuenow={settings[h]}
      aria-valuetext={`${settings[h].toFixed(1)} °C`}
      onPointerDown={(e) => {
        e.stopPropagation();
        onPointerDown(e, h);
      }}
      onKeyDown={(e) => onKeyDown(e, h)}
    />
  );

  const isDefault =
    warmC === DEFAULT_DIFF.warmC && coldC === DEFAULT_DIFF.coldC && thresholdC === DEFAULT_DIFF.thresholdC;

  return (
    <div className="palette-bar">
      <div className="info-panel-label diff-legend-title">Change</div>
      <div className="scale">
        <div
          ref={trackRef}
          className="scale-track manual"
          style={{ background: gradient }}
          onPointerDown={(e) => onPointerDown(e)}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          title="Drag: upper = warming at full color, lower = cooling at full color, middle = noise threshold"
        >
          {handleEl("warmC", pWarm, "Warming at full color")}
          {handleEl("thresholdC", pThrHi, "Noise threshold", "band")}
          {handleEl("coldC", pCold, "Cooling at full color")}
        </div>
        <div className="scale-labels">
          <span className="scale-label max" style={{ bottom: `${pWarm}%` }}>
            +{warmC.toFixed(1)}&deg;
          </span>
          <span className="scale-label diff-band" style={{ bottom: "50%" }}>
            &plusmn;{thresholdC.toFixed(1)}&deg;
          </span>
          <span className="scale-label min" style={{ bottom: `${pCold}%` }}>
            &minus;{coldC.toFixed(1)}&deg;
          </span>
        </div>
      </div>
      <button
        type="button"
        className={`scale-mode ${isDefault ? "auto" : ""}`}
        onClick={() =>
          onChange({ ...settings, warmC: DEFAULT_DIFF.warmC, coldC: DEFAULT_DIFF.coldC, thresholdC: DEFAULT_DIFF.thresholdC })
        }
        title="Reset to ±3.0° / ±0.1°"
      >
        Reset
      </button>
    </div>
  );
}
