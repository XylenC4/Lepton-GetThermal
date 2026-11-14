import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme, loadThemeMode } from "./hooks/useTheme";

// Apply the saved theme before the first paint (no dark->light flash).
applyTheme(loadThemeMode());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
