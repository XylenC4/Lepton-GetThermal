export type ContrastMode = "linear" | "equalized";
export type HoldMode = "off" | "max" | "min";

/** Lepton frame rate over USB (~8.7 fps), for the hold duration display. */
const FPS = 8.7;

/** Row of mutually exclusive buttons (radio group). */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string; title?: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          className={`segmented-option ${value === o.value ? "active" : ""}`}
          onClick={() => onChange(o.value)}
          title={o.title}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

function formatDuration(frames: number): string {
  const s = Math.round(frames / FPS);
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")} min`;
}

interface PeakHoldControlsProps {
  hold: HoldMode;
  onHoldChange: (m: HoldMode) => void;
  onHoldReset: () => void;
  holdFrames: number;
}

/** Peak hold: keep each pixel's hottest or coldest value. */
export function PeakHoldControls({ hold, onHoldChange, onHoldReset, holdFrames }: PeakHoldControlsProps) {
  return (
    <div className="control-section">
      <h3>Peak hold</h3>
      <Segmented
        label="Peak hold"
        value={hold}
        onChange={onHoldChange}
        options={[
          { value: "off", label: "Off" },
          { value: "max", label: "Max", title: "Keep each pixel's hottest value" },
          { value: "min", label: "Min", title: "Keep each pixel's coldest value" },
        ]}
      />
      {hold !== "off" && (
        <div className="hold-status">
          <span>
            {hold === "max" ? "Hottest" : "Coldest"} since {formatDuration(holdFrames)}
          </span>
          <button type="button" className="ffc-button hold-reset" onClick={onHoldReset} title="Start over from now">
            Reset
          </button>
        </div>
      )}
    </div>
  );
}
