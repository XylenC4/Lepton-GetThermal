import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const media = () => window.matchMedia("(prefers-color-scheme: dark)");

export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode !== "system") return mode;
  return media().matches ? "dark" : "light";
}

/** Sets data-theme on <html>. Called once before the first render to avoid a flash. */
export function applyTheme(mode: ThemeMode) {
  document.documentElement.dataset.theme = resolveTheme(mode);
}

export function useTheme(initial: ThemeMode = "system") {
  const [mode, setMode] = useState<ThemeMode>(initial);

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

  return { mode, setMode };
}
