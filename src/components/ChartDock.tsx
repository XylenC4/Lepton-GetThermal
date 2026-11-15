import { useMemo } from "react";
import { LineChart, Series } from "./LineChart";
import { Segmented } from "./ImageModeControls";
import { Tool, ToolStats, TrendSample } from "../lib/measure";

export type TrendMetric = "max" | "avg" | "min";
export type TrendWindow = 60 | 300 | 900;

interface ChartDockProps {
  tools: Tool[];
  stats: Record<number, ToolStats | null>;
  samples: TrendSample[];
  /** Bumped on every new sample so the memo below recomputes. */
  sampleTick: number;
  selectedId: number | null;
  metric: TrendMetric;
  onMetric: (m: TrendMetric) => void;
  windowS: TrendWindow;
  onWindow: (w: TrendWindow) => void;
}

/** Max points drawn per series (the CSV keeps every sample). */
const MAX_POINTS = 600;

/** Panel under the image: trend of all tools, and the profile of the selected line. */
export function ChartDock({ tools, stats, samples, sampleTick, selectedId, metric, onMetric, windowS, onWindow }: ChartDockProps) {
  const now = samples.length ? samples[samples.length - 1].t : 0;

  const trend: Series[] = useMemo(() => {
    const from = now - windowS * 1000;
    let start = samples.length;
    while (start > 0 && samples[start - 1].t >= from) start--;
    const win = samples.slice(start);
    const stride = Math.max(1, Math.ceil(win.length / MAX_POINTS));
    return tools.map((t) => {
      const pts: [number, number][] = [];
      for (let i = 0; i < win.length; i += stride) {
        const v = win[i].stats[t.id];
        if (v) pts.push([(win[i].t - now) / 1000, v[metric]]);
      }
      return { key: String(t.id), label: t.label, slot: t.slot, points: pts };
    });
    // sampleTick: samples is mutated in place (ring buffer in a ref).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tools, metric, windowS, sampleTick, now]);

  const line = tools.find((t) => t.id === selectedId && t.kind === "line") ?? tools.find((t) => t.kind === "line");
  const profile: Series[] = line
    ? [
        {
          key: String(line.id),
          label: line.label,
          slot: line.slot,
          points: (stats[line.id]?.profile ?? []).map((v, i) => [i, v] as [number, number]),
        },
      ]
    : [];
  const profileLen = profile[0]?.points.length ?? 0;

  return (
    <section className="chart-dock" aria-label="Measurement graphs">
      <div className="chart-dock-controls">
        <Segmented
          label="Trend metric"
          value={metric}
          onChange={onMetric}
          options={[
            { value: "max", label: "Max" },
            { value: "avg", label: "Avg" },
            { value: "min", label: "Min" },
          ]}
        />
        <Segmented
          label="Time window"
          value={String(windowS) as "60" | "300" | "900"}
          onChange={(v) => onWindow(Number(v) as TrendWindow)}
          options={[
            { value: "60", label: "1 min" },
            { value: "300", label: "5 min" },
            { value: "900", label: "15 min" },
          ]}
        />
      </div>
      <div className="chart-dock-charts">
        <LineChart
          title={`Trend · ${metric}`}
          series={trend}
          xDomain={[-windowS, 0]}
          formatX={(x) => (x === 0 ? "now" : windowS > 60 ? `${Math.round(x / 60)}m` : `${Math.round(x)}s`)}
          formatXLong={(x) => `${Math.abs(x).toFixed(1)} s ago`}
          yUnit="°C"
          empty="Add a box or line to record a trend"
        />
        <LineChart
          title={line ? `Profile · ${line.label}` : "Profile"}
          series={profile}
          xDomain={[0, Math.max(profileLen - 1, 1)]}
          formatX={(x) => `${Math.round(x)}`}
          formatXLong={(x) => `pixel ${Math.round(x)} along ${line?.label ?? ""}`}
          yUnit="°C"
          empty="Draw a line to see its temperature profile"
        />
      </div>
    </section>
  );
}
