import { useState, type ReactNode } from "react";

export type PanelTab = "view" | "analyze" | "device";

const TABS: { id: PanelTab; label: string }[] = [
  { id: "view", label: "View" },
  { id: "analyze", label: "Analyze" },
  { id: "device", label: "Device" },
];

const STORAGE_KEY = "lepton-panel-tab";

function loadTab(): PanelTab {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v === "view" || v === "analyze" || v === "device") return v;
  } catch {
    /* storage unavailable */
  }
  return "view";
}

interface ControlPanelProps {
  view: ReactNode;
  analyze: ReactNode;
  device: ReactNode;
  /** Small dot on a tab that has something active (e.g. hold or difference running). */
  badges?: Partial<Record<PanelTab, boolean>>;
}

/**
 * Left panel: header + View / Analyze / Device tabs. All pages stay mounted and
 * are only hidden, so controls with local state (e.g. isotherm) keep it.
 */
export function ControlPanel({ view, analyze, device, badges }: ControlPanelProps) {
  const [tab, setTab] = useState<PanelTab>(loadTab);

  const select = (t: PanelTab) => {
    setTab(t);
    try {
      localStorage.setItem(STORAGE_KEY, t);
    } catch {
      /* not persisted */
    }
  };

  const pages: Record<PanelTab, ReactNode> = { view, analyze, device };

  return (
    <aside className="control-panel">
      <div className="panel-header">
        <h2>Thermal</h2>
        <span className="version-badge">V2</span>
      </div>
      <div className="panel-tabs" role="tablist" aria-label="Panel">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            id={`tab-${t.id}`}
            aria-selected={tab === t.id}
            aria-controls={`page-${t.id}`}
            className={`panel-tab ${tab === t.id ? "active" : ""}`}
            onClick={() => select(t.id)}
          >
            {t.label}
            {badges?.[t.id] && <span className="panel-tab-dot" aria-label="active" />}
          </button>
        ))}
      </div>
      {TABS.map((t) => (
        <div
          key={t.id}
          id={`page-${t.id}`}
          role="tabpanel"
          aria-labelledby={`tab-${t.id}`}
          className="panel-page"
          hidden={tab !== t.id}
        >
          {pages[t.id]}
        </div>
      ))}
    </aside>
  );
}
