import { Tool, ToolStats, MAX_TOOLS, seriesVar } from "../lib/measure";

interface MeasureControlsProps {
  tools: Tool[];
  stats: Record<number, ToolStats | null>;
  selectedId: number | null;
  onSelect: (id: number) => void;
  onRemove: (id: number) => void;
  onClearAll: () => void;
  drawMode: "box" | "line" | null;
  onDraw: (kind: "box" | "line" | null) => void;
  onExportCsv: () => void;
  samples: number;
  graphOpen: boolean;
  onToggleGraph: () => void;
  streaming: boolean;
}

const fmt = (v: number | undefined) => (v === undefined ? "—" : v.toFixed(1));

/** Analyze tab: add boxes/lines, their live values (table view), trend export. */
export function MeasureControls({
  tools,
  stats,
  selectedId,
  onSelect,
  onRemove,
  onClearAll,
  drawMode,
  onDraw,
  onExportCsv,
  samples,
  graphOpen,
  onToggleGraph,
  streaming,
}: MeasureControlsProps) {
  const full = tools.length >= MAX_TOOLS;
  return (
    <div className="control-section">
      <h3>Measure</h3>
      <div className="button-row">
        {(["box", "line"] as const).map((k) => (
          <button
            key={k}
            type="button"
            className={`ffc-button ${drawMode === k ? "armed" : ""}`}
            disabled={!streaming || (full && drawMode !== k)}
            onClick={() => onDraw(drawMode === k ? null : k)}
            title={full ? `Up to ${MAX_TOOLS} tools` : `Then drag on the image (Esc cancels)`}
          >
            {drawMode === k ? "Drag on image…" : k === "box" ? "+ Box" : "+ Line"}
          </button>
        ))}
      </div>
      {tools.length > 0 && (
        <>
          <table className="measure-table">
            <thead>
              <tr>
                <th />
                <th>min</th>
                <th>max</th>
                <th>avg</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {tools.map((t) => {
                const s = stats[t.id];
                return (
                  <tr
                    key={t.id}
                    className={t.id === selectedId ? "selected" : ""}
                    onClick={() => onSelect(t.id)}
                    title={t.kind === "line" ? "Select to show its profile" : "Select"}
                  >
                    <td>
                      <span className="measure-swatch" style={{ background: seriesVar(t.slot) }} />
                      {t.label}
                    </td>
                    <td>{fmt(s?.min)}</td>
                    <td>{fmt(s?.max)}</td>
                    <td>{fmt(s?.avg)}</td>
                    <td>
                      <button
                        type="button"
                        className="measure-remove"
                        aria-label={`Remove ${t.label}`}
                        onClick={(e) => {
                          e.stopPropagation();
                          onRemove(t.id);
                        }}
                      >
                        ×
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          <div className="measure-unit">°C · {samples} samples recorded</div>
          <div className="button-row">
            <button type="button" className="ffc-button" onClick={onToggleGraph}>
              {graphOpen ? "Hide graph" : "Show graph"}
            </button>
            <button type="button" className="ffc-button" onClick={onExportCsv} disabled={samples === 0}>
              CSV
            </button>
            <button type="button" className="ffc-button" onClick={onClearAll} title="Remove all tools and their history">
              Clear
            </button>
          </div>
        </>
      )}
    </div>
  );
}
