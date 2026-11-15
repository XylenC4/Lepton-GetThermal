import { Segmented } from "./ImageModeControls";

export type GainMode = "high" | "low" | "auto";

export interface MeasurementConfig {
  gain: GainMode;
  emissivity: number; // 0.10..1.00
  reflectedC: number; // °C
}

export const DEFAULT_MEASUREMENT: MeasurementConfig = { gain: "high", emissivity: 1.0, reflectedC: 22 };

/** Typical emissivities (approximate; surfaces vary). */
const PRESETS: { label: string; e: number }[] = [
  { label: "Ideal / black body", e: 1.0 },
  { label: "Human skin", e: 0.98 },
  { label: "Matte black paint", e: 0.97 },
  { label: "Water, ice", e: 0.96 },
  { label: "Brick, concrete", e: 0.93 },
  { label: "Wood, paper, plastic", e: 0.9 },
  { label: "Oxidized steel", e: 0.8 },
  { label: "Anodized aluminium", e: 0.77 },
  { label: "Polished metal", e: 0.1 },
];

interface MeasurementSettingsProps {
  config: MeasurementConfig;
  onChange: (c: MeasurementConfig) => void;
}

/** Device tab: sensor gain and emissivity correction (applied in the Lepton). */
export function MeasurementSettings({ config, onChange }: MeasurementSettingsProps) {
  const preset = PRESETS.find((p) => Math.abs(p.e - config.emissivity) < 0.005);
  return (
    <div className="control-section">
      <h3>Measurement</h3>
      <label>
        Gain
        <Segmented
          label="Gain"
          value={config.gain}
          onChange={(gain) => onChange({ ...config, gain })}
          options={[
            { value: "high", label: "High", title: "≈ −10…140 °C, most sensitive (default)" },
            { value: "low", label: "Low", title: "≈ −10…400 °C, less sensitive" },
            { value: "auto", label: "Auto", title: "Lepton switches between high and low" },
          ]}
        />
      </label>
      <label>
        Material
        <select
          value={preset ? String(preset.e) : "custom"}
          onChange={(e) => {
            if (e.target.value !== "custom") onChange({ ...config, emissivity: Number(e.target.value) });
          }}
        >
          {PRESETS.map((p) => (
            <option key={p.label} value={String(p.e)}>
              {p.label} ({p.e.toFixed(2)})
            </option>
          ))}
          {!preset && <option value="custom">Custom ({config.emissivity.toFixed(2)})</option>}
        </select>
      </label>
      <label>
        Emissivity
        <div className="slider-row">
          <input
            type="range"
            min={0.1}
            max={1}
            step={0.01}
            value={config.emissivity}
            onChange={(e) => onChange({ ...config, emissivity: Number(e.target.value) })}
          />
          <span>{config.emissivity.toFixed(2)}</span>
        </div>
      </label>
      <label>
        Reflected temperature
        <div className="slider-row">
          <input
            type="range"
            min={-20}
            max={60}
            step={1}
            value={config.reflectedC}
            onChange={(e) => onChange({ ...config, reflectedC: Number(e.target.value) })}
          />
          <span>{config.reflectedC}&deg;C</span>
        </div>
      </label>
      <div className="diff-hint">
        Gain changes run an FFC. Emissivity and reflected temperature correct all readings; the
        reflected temperature matters most for low emissivities.
      </div>
    </div>
  );
}
