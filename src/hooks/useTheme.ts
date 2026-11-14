import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "lepton-theme";
const media = () => window.matchMedia("(prefers-color-scheme: dark)");

export function loadThemeMode(): ThemeMode {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* storage unavailable: fall back to the default */
  }
  return "system";
}

function saveThemeMode(mode: ThemeMode) {
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    /* not persisted, still applied for this session */
  }
}

export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode !== "system") return mode;
  return media().matches ? "dark" : "light";
}

/** Sets data-theme on <html>. Called once before the first render to avoid a flash. */
export function applyTheme(mode: ThemeMode) {
  document.documentElement.dataset.theme = resolveTheme(mode);
}

export function useTheme() {
  const [mode, setModeState] = useState<ThemeMode>(loadThemeMode);

  useEffect(() => {
    applyTheme(mode);
    // Native title bar / window chrome: null follows the OS theme.
    getCurrentWindow()
      .setTheme(mode === "system" ? null : mode)
      .catch(() => {});

    if (mode !== "system") return;
    const mq = media();
    const onChange = () => applyTheme("system");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [mode]);

  const setMode = useCallback((m: ThemeMode) => {
    saveThemeMode(m);
    setModeState(m);
  }, []);

  return { mode, setMode };
}
