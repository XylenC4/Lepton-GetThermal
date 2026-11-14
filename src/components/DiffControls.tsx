export interface DiffSettings {
  enabled: boolean;
  /** Extra margin (°C) beyond each pixel's recorded min/max that still counts as noise. */
  thresholdC: number;
  /** Warming (°C) that gets full color. */
  warmC: number;
  /** Cooling (°C) that gets full color. */
  coldC: number;
}

export const DEFAULT_DIFF: DiffSettings = { enabled: false, thresholdC: 0.1, warmC: 3, coldC: 3 };

/** Frames recorded for the reference (per-pixel min/max), ~2 s at 8.7 fps. */
export const REFERENCE_FRAMES = 16;

interface DiffControlsProps {
  settings: DiffSettings;
  onChange: (s: DiffSettings) => void;
  onRecord: () => void;
  onClear: () => void;
  referenceReady: boolean;
  recording: boolean;
  streaming: boolean;
}

export function DiffControls({
  settings,
  onChange,
  onRecord,
  onClear,
  referenceReady,
  recording,
  streaming,
}: DiffControlsProps) {
  const status = recording ? "Recording min/max…" : referenceReady ? "Reference stored" : "No reference";

  return (
    <div className="control-section">
      <h3>Difference</h3>
      <div className="diff-status">
        <span className={`diff-dot ${recording ? "recording" : referenceReady ? "ready" : ""}`} />
        {status}
      </div>
      <div className="button-row">
        <button
          type="button"
          className="ffc-button"
          onClick={onRecord}
          disabled={!streaming || recording}
          title={`Record each pixel's min and max over ${REFERENCE_FRAMES} frames (~2 s). Keep the scene still.`}
        >
          {referenceReady ? "Re-record" : "Record reference"}
        </button>
        {referenceReady && (
          <button type="button" className="ffc-button" onClick={onClear} title="Forget the reference">
            Clear
          </button>
        )}
      </div>
      <label className="toggle">
        <input
          type="checkbox"
          checked={settings.enabled}
          disabled={!referenceReady && !recording}
          onChange={() => onChange({ ...settings, enabled: !settings.enabled })}
        />
        Show only changes
      </label>
      {settings.enabled && referenceReady && (
        <div className="diff-hint">Set the ranges on the scale on the right.</div>
      )}
    </div>
  );
}
