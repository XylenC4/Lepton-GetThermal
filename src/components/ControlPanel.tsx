import type { ReactNode } from "react";
import { VideoControls } from "./VideoControls";
import { DeviceInfo } from "./DeviceInfo";
import { ThemeSwitcher } from "./ThemeSwitcher";
import { ThemeMode } from "../hooks/useTheme";
import { Palette, DeviceInfo as DeviceInfoType } from "../lib/types";

interface ControlPanelProps {
  deviceInfo: DeviceInfoType | null;
  currentPalette: Palette;
  onPaletteChange: (palette: Palette) => void;
  onFfc: () => void;
  onPolarityChange: (polarity: number) => void;
  onIsothermChange: (tempC: number | null) => void;
  onCapture: () => void;
  showMarkers: boolean;
  onToggleMarkers: () => void;
  upscaleEnabled: boolean;
  onToggleUpscale: () => void;
  streaming: boolean;
  themeMode: ThemeMode;
  onThemeChange: (mode: ThemeMode) => void;
  /** Extra sections rendered below the imaging controls (e.g. Difference). */
  children?: ReactNode;
}

export function ControlPanel({
  deviceInfo,
  currentPalette,
  onPaletteChange,
  onFfc,
  onPolarityChange,
  onIsothermChange,
  onCapture,
  showMarkers,
  onToggleMarkers,
  upscaleEnabled,
  onToggleUpscale,
  streaming,
  themeMode,
  onThemeChange,
  children,
}: ControlPanelProps) {
  return (
    <aside className="control-panel">
      <div className="panel-header">
        <h2>Thermal</h2>
        <span className="version-badge">V2</span>
      </div>
      <DeviceInfo info={deviceInfo} />
      <VideoControls
        currentPalette={currentPalette}
        onPaletteChange={onPaletteChange}
        onFfc={onFfc}
        onPolarityChange={onPolarityChange}
        onIsothermChange={onIsothermChange}
        onCapture={onCapture}
        showMarkers={showMarkers}
        onToggleMarkers={onToggleMarkers}
        upscaleEnabled={upscaleEnabled}
        onToggleUpscale={onToggleUpscale}
        streaming={streaming}
      />
      {children}
      <div className="panel-footer">
        <ThemeSwitcher mode={themeMode} onChange={onThemeChange} />
      </div>
    </aside>
  );
}
