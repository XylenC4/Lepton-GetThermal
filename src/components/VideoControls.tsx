import { useState } from "react";
import { Palette } from "../lib/types";
import { Segmented, ContrastMode } from "./ImageModeControls";

interface ViewControlsProps {
  currentPalette: Palette;
  onPaletteChange: (palette: Palette) => void;
  onPolarityChange: (polarity: number) => void;
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
  onPolarityChange,
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
        <select onChange={(e) => onPolarityChange(Number(e.target.value))}>
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

interface IsothermControlsProps {
  onIsothermChange: (tempC: number | null) => void;
}

/** Isotherm: stripe everything above a temperature. */
export function IsothermControls({ onIsothermChange }: IsothermControlsProps) {
  const [isothermEnabled, setIsothermEnabled] = useState(false);
  const [isothermTemp, setIsothermTemp] = useState(35);

  const handleIsothermToggle = () => {
    const next = !isothermEnabled;
    setIsothermEnabled(next);
    onIsothermChange(next ? isothermTemp : null);
  };

  const handleIsothermTemp = (val: number) => {
    setIsothermTemp(val);
    if (isothermEnabled) {
      onIsothermChange(val);
    }
  };

  return (
    <div className="control-section">
      <h3>Isotherm</h3>
      <label className="toggle">
        <input type="checkbox" checked={isothermEnabled} onChange={handleIsothermToggle} />
        Stripe above threshold
      </label>
      {isothermEnabled && (
        <label>
          Threshold
          <div className="slider-row">
            <input
              type="range"
              min={-20}
              max={150}
              value={isothermTemp}
              onChange={(e) => handleIsothermTemp(Number(e.target.value))}
            />
            <span>{isothermTemp}&deg;C</span>
          </div>
        </label>
      )}
    </div>
  );
}
