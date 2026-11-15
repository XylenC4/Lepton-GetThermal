import { invoke } from "@tauri-apps/api/core";
import type { Palette } from "./types";
import type { Rotation } from "./rotation";
import type { Tool } from "./measure";
import type { ThemeMode } from "../hooks/useTheme";
import type { PanelTab } from "../components/ControlPanel";
import type { ContrastMode, HoldMode } from "../components/ImageModeControls";
import type { DiffSettings } from "../components/DiffControls";
import type { MeasurementConfig } from "../components/MeasurementSettings";
import type { DisplayRange } from "../components/PaletteBar";
import type { TrendMetric, TrendWindow } from "../components/ChartDock";

/**
 * Everything restored at the next start, stored in session.json by the backend
 * (next to the exe in portable use, else in the user's app config folder).
 * All fields optional: older or hand-edited files fall back to defaults.
 */
export interface Session {
  version?: 1;
  theme?: ThemeMode;
  tab?: PanelTab;
  palette?: Palette;
  polarity?: 0 | 1;
  contrast?: ContrastMode;
  hold?: HoldMode;
  markers?: boolean;
  upscale?: boolean;
  rotation?: Rotation;
  displayRange?: DisplayRange | null;
  /** Difference settings; the reference itself is not saved, so `enabled` starts off. */
  diff?: DiffSettings;
  isotherm?: { enabled: boolean; tempC: number };
  measurement?: MeasurementConfig;
  /** Boxes / lines in displayed-image coordinates (for `rotation`). */
  tools?: Tool[];
  graph?: { open: boolean; metric: TrendMetric; window: TrendWindow };
  /** Spot meter position, sensor pixel. */
  spot?: { col: number; row: number };
}

/** Settings stored by earlier versions in localStorage, used when there is no session file yet. */
function legacy(): Session {
  const s: Session = {};
  try {
    const theme = localStorage.getItem("lepton-theme");
    if (theme === "system" || theme === "light" || theme === "dark") s.theme = theme;
    const tab = localStorage.getItem("lepton-panel-tab");
    if (tab === "view" || tab === "analyze" || tab === "device") s.tab = tab;
    const m = JSON.parse(localStorage.getItem("lepton-measurement") ?? "null");
    if (m && typeof m.emissivity === "number") s.measurement = m;
  } catch {
    /* ignore */
  }
  return s;
}

export async function loadSession(): Promise<Session> {
  try {
    const s = await invoke<Session | null>("load_session");
    if (s && typeof s === "object") return s;
  } catch (e) {
    console.warn("load_session failed", e);
  }
  return legacy();
}

let timer: ReturnType<typeof setTimeout> | undefined;
let pending: Session | null = null;

async function flush() {
  if (!pending) return;
  const s = pending;
  pending = null;
  try {
    await invoke("save_session", { session: s });
  } catch (e) {
    console.warn("save_session failed", e);
  }
}

/** Saves shortly after the last change (debounced). */
export function saveSession(s: Session) {
  pending = { ...s, version: 1 };
  clearTimeout(timer);
  timer = setTimeout(flush, 400);
}

// Closing the window: write whatever is still pending.
window.addEventListener("beforeunload", () => {
  clearTimeout(timer);
  void flush();
});
