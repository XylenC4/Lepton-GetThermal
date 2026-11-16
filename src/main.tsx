import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme } from "./hooks/useTheme";
import { loadSession, Session } from "./lib/session";
import { installDebugLog, jsLog } from "./lib/debuglog";

installDebugLog();

/** Shows render errors instead of a blank window (and logs them). */
class ErrorBoundary extends React.Component<{ children: React.ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: React.ErrorInfo) {
    jsLog("error", "React render error:", error, info.componentStack ?? "");
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <pre style={{ color: "#f88", background: "#111", padding: 16, margin: 0, height: "100vh", overflow: "auto", whiteSpace: "pre-wrap", font: "12px monospace" }}>
        {"Lepton-GetThermal crashed while drawing the UI.\nDetails are in lepton-getthermal.log.\n\n"}
        {String(this.state.error.stack ?? this.state.error)}
      </pre>
    );
  }
}

function render(session: Session) {
  applyTheme(session.theme ?? "system");
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ErrorBoundary>
        <App session={session} />
      </ErrorBoundary>
    </React.StrictMode>,
  );
  jsLog("info", "UI rendered");
}

// Restore the last session (theme applied before the first paint). If the
// backend does not answer, start anyway after 3 s instead of staying blank.
const timeout = new Promise<null>((r) => setTimeout(() => r(null), 3000));
Promise.race([loadSession(), timeout])
  .then((s) => {
    if (!s) jsLog("warn", "load_session timed out, starting with defaults");
    render(s ?? ({} as Session));
  })
  .catch((e) => {
    jsLog("error", "startup failed:", e);
    render({} as Session);
  });
