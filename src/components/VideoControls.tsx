import { Rotation } from "../lib/rotation";
import { Palette } from "../lib/types";
import { Segmented, ContrastMode } from "./ImageModeControls";

interface ViewControlsProps {
  currentPalette: Palette;
  onPaletteChange: (palette: Palette) => void;
  polarity: number;
  onPolarityChange: (polarity: number) => void;
  rotation: Rotation;
  onRotationChange: (r: Rotation) => void;
  contrast: ContrastMode;
  onContrastChange: (m: ContrastMode) => void;
  showMarkers: boolean;
  onToggleMarkers: () => void;
  upscaleEnabled: boolean;
  onToggleUpscale: () => void;
  onCapture: () => void;
  streaming: boolean;
}

/** "View" tab: how the image looks. */
export function ViewControls({
  currentPalette,
  onPaletteChange,
  polarity,
  onPolarityChange,
  rotation,
  onRotationChange,
  contrast,
  onContrastChange,
  showMarkers,
  onToggleMarkers,
  upscaleEnabled,
  onToggleUpscale,
  onCapture,
  streaming,
}: ViewControlsProps) {
  return (
    <div className="control-section">
      <label>
        Palette
        <select value={currentPalette} onChange={(e) => onPaletteChange(e.target.value as Palette)}>
          <option value="ironblack">Iron Black</option>
          <option value="rainbow">Rainbow</option>
          <option value="grayscale">Grayscale</option>
        </select>
      </label>
      <label>
        Polarity
        <select value={polarity} onChange={(e) => onPolarityChange(Number(e.target.value))}>
          <option value={0}>White Hot</option>
          <option value={1}>Black Hot</option>
        </select>
      </label>
      <label>
        Contrast
        <Segmented
          label="Contrast"
          value={contrast}
          onChange={onContrastChange}
          options={[
            { value: "linear", label: "Linear", title: "Colors evenly spread from coldest to hottest" },
            {
              value: "equalized",
              label: "Equalized",
              title:
                "Histogram equalization: colors follow the temperatures that occur, so one hot object doesn't flatten the rest",
            },
          ]}
        />
      </label>
      <label>
        Rotation
        <Segmented
          label="Rotation"
          value={String(rotation) as "0" | "90" | "180" | "270"}
          onChange={(v) => onRotationChange(Number(v) as Rotation)}
          options={(["0", "90", "180", "270"] as const).map((v) => ({
            value: v,
            label: `${v}°`,
            title: v === "0" ? "As the sensor sees it" : `Rotate the image ${v}° clockwise`,
          }))}
        />
      </label>
      <label className="toggle">
        <input type="checkbox" checked={showMarkers} onChange={onToggleMarkers} />
        Min/Max markers
      </label>
      <label className="toggle">
        <input type="checkbox" checked={upscaleEnabled} onChange={onToggleUpscale} />
        SR Upscale (3x)
      </label>
      {streaming && (
        <button onClick={onCapture} className="ffc-button">
          Capture
        </button>
      )}
    </div>
  );
}

export interface IsothermSettings {
  enabled: boolean;
  tempC: number;
}

export const DEFAULT_ISOTHERM: IsothermSettings = { enabled: false, tempC: 35 };

interface IsothermControlsProps {
  settings: IsothermSettings;
  onChange: (s: IsothermSettings) => void;
}

/** Isotherm: stripe everything above a temperature. */
export function IsothermControls({ settings, onChange }: IsothermControlsProps) {
  return (
    <div className="control-section">
      <h3>Isotherm</h3>
      <label className="toggle">
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={() => onChange({ ...settings, enabled: !settings.enabled })}
        />
        Stripe above threshold
      </label>
      {settings.enabled && (
        <label>
          Threshold
          <div className="slider-row">
            <input
              type="range"
              min={-20}
              max={150}
              value={settings.tempC}
              onChange={(e) => onChange({ ...settings, tempC: Number(e.target.value) })}
            />
            <span>{settings.tempC}&deg;C</span>
          </div>
        </label>
      )}
    </div>
  );
}
