// Forwards frontend logs and errors to the Rust side, which writes them to
// lepton-getthermal.log (Windows: next to the exe, or %TEMP%).
import { invoke } from "@tauri-apps/api/core";

function fmt(args: unknown[]): string {
  return args
    .map((a) => {
      if (a instanceof Error) return `${a.name}: ${a.message}\n${a.stack ?? ""}`;
      if (typeof a === "string") return a;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

export function jsLog(level: string, ...args: unknown[]) {
  try {
    invoke("frontend_log", { level, message: fmt(args) }).catch(() => {});
  } catch {
    /* IPC not available */
  }
}

export function installDebugLog() {
  for (const level of ["log", "info", "warn", "error"] as const) {
    const orig = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      orig(...args);
      jsLog(level, ...args);
    };
  }
  window.addEventListener("error", (e) => {
    jsLog("error", `window.onerror: ${e.message} at ${e.filename}:${e.lineno}:${e.colno}`, e.error ?? "");
  });
  window.addEventListener("unhandledrejection", (e) => {
    jsLog("error", "unhandled promise rejection:", e.reason);
  });
  jsLog("info", `frontend started, UA: ${navigator.userAgent}, ${window.innerWidth}x${window.innerHeight}`);
}
