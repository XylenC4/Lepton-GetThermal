import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme } from "./hooks/useTheme";
import { loadSession } from "./lib/session";

// Restore the last session, and apply its theme before the first paint (no flash).
loadSession().then((session) => {
  applyTheme(session.theme ?? "system");
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App session={session} />
    </React.StrictMode>,
  );
});
