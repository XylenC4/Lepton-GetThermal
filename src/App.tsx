import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { VideoCanvas } from "./components/VideoCanvas";
import { ControlPanel } from "./components/ControlPanel";
import { TemperatureDisplay } from "./components/TemperatureDisplay";
import { PaletteBar, DisplayRange } from "./components/PaletteBar";
import { SpotmeterOverlay } from "./components/SpotmeterOverlay";
import { MarkersOverlay } from "./components/MarkersOverlay";
import { DiffControls, DiffSettings, DEFAULT_DIFF, REFERENCE_FRAMES } from "./components/DiffControls";
import { DiffLegend } from "./components/DiffLegend";
import { PeakHoldControls, ContrastMode, HoldMode } from "./components/ImageModeControls";
import { ViewControls, IsothermControls, IsothermSettings, DEFAULT_ISOTHERM } from "./components/VideoControls";
import { PanelTab } from "./components/ControlPanel";
import { Session, saveSession } from "./lib/session";
import { Rotation, asRotation, rerotatePoint } from "./lib/rotation";
import { DeviceInfo } from "./components/DeviceInfo";
import { ThemeSwitcher } from "./components/ThemeSwitcher";
import { HoverReadout } from "./components/HoverReadout";
import { MeasureOverlay } from "./components/MeasureOverlay";
import { MeasureControls } from "./components/MeasureControls";
import { ChartDock, TrendMetric, TrendWindow } from "./components/ChartDock";
import { MeasurementSettings, MeasurementConfig, DEFAULT_MEASUREMENT } from "./components/MeasurementSettings";
import { useElementSize } from "./hooks/useElementSize";
import { Tool, ToolStats, TrendSample, MAX_TOOLS, toolStats, trendCsv } from "./lib/measure";
import { useCamera } from "./hooks/useCamera";
import { useAutoConnect } from "./hooks/useAutoConnect";
import { WaitingScreen } from "./components/WaitingScreen";
import { useTheme } from "./hooks/useTheme";
import { FrameStats } from "./hooks/useFrameStream";
import { Palette, DEFAULT_WIDTH, DEFAULT_HEIGHT } from "./lib/types";
import "./App.css";

function App({ session }: { session: Session }) {
  const camera = useCamera();
  const theme = useTheme(session.theme ?? "system");
  const [palette, setPalette] = useState<Palette>(session.palette ?? "ironblack");
  const [frameStats, setFrameStats] = useState<FrameStats>({
    minVal: 0, maxVal: 0, minPos: 0, maxPos: 0, width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT,
    referenceReady: false, recordingReference: false, holdFrames: 0, scaleTicks: [], raw: null,
  });
  const [showMarkers, setShowMarkers] = useState(session.markers ?? false);
  const [upscaleEnabled, setUpscaleEnabled] = useState(session.upscale ?? false);
  const [displayRange, setDisplayRange] = useState<DisplayRange | null>(session.displayRange ?? null);
  const videoContainerRef = useRef<HTMLDivElement>(null);

  // Contrast mode and peak hold; like the other settings, re-sent after a reconnect.
  const [contrast, setContrast] = useState<ContrastMode>(session.contrast ?? "linear");
  const [hold, setHold] = useState<HoldMode>(session.hold ?? "off");
  // Display rotation (backend rotates every frame); see handleRotationChange.
  const [rotation, setRotationState] = useState<Rotation>(asRotation(session.rotation));
  const isStreamingNow = camera.state === "streaming";
  const { setContrastMode: sendContrast, setHoldMode: sendHold, resetHold } = camera;
  useEffect(() => {
    if (isStreamingNow) sendContrast(contrast).catch(() => {});
  }, [isStreamingNow, contrast, sendContrast]);
  useEffect(() => {
    if (isStreamingNow) sendHold(hold).catch(() => {});
  }, [isStreamingNow, hold, sendHold]);

  // Push the scale limits to the backend; re-sent after a reconnect (backend starts in auto).
  // Applies to both contrast modes (equalization then runs within the range).
  const sendDisplayRange = camera.setDisplayRange;
  useEffect(() => {
    if (!isStreamingNow) return;
    sendDisplayRange(displayRange?.min ?? null, displayRange?.max ?? null).catch(() => {});
  }, [isStreamingNow, displayRange, sendDisplayRange]);

  // Temperature under the pointer.
  const [pointer, setPointer] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const handlePointerMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setPointer({ x: e.clientX - r.left, y: e.clientY - r.top, w: r.width, h: r.height });
  }, []);

  // Difference mode. Settings in °C, sent in raw units (TLinear 0.01 K or 0.1 K).
  // The reference itself is not saved, so difference mode starts off.
  const [diff, setDiff] = useState<DiffSettings>({ ...DEFAULT_DIFF, ...session.diff, enabled: false });
  const rawPerDegree = frameStats.maxVal > 0 && frameStats.maxVal <= 10000 ? 10 : 100;
  const sendDiff = camera.setDiffMode;
  useEffect(() => {
    if (!isStreamingNow) return;
    sendDiff(
      diff.enabled,
      diff.thresholdC * rawPerDegree,
      diff.warmC * rawPerDegree,
      diff.coldC * rawPerDegree
    ).catch(() => {});
  }, [isStreamingNow, diff, rawPerDegree, sendDiff]);

  const handleRecordReference = useCallback(() => {
    camera.recordReference(REFERENCE_FRAMES).catch(() => {});
    setDiff((d) => ({ ...d, enabled: true }));
  }, [camera.recordReference]);

  const handleClearReference = useCallback(() => {
    camera.clearReference().catch(() => {});
    setDiff((d) => ({ ...d, enabled: false }));
  }, [camera.clearReference]);

  const diffActive = diff.enabled && frameStats.referenceReady;

  // Difference reference envelope for the pointer readout: fetched whenever a
  // recording finishes (or after reconnect when one exists).
  const [reference, setReference] = useState<{ min: Uint16Array; max: Uint16Array } | null>(null);
  const getReference = camera.getReference;
  useEffect(() => {
    if (!isStreamingNow || !frameStats.referenceReady || frameStats.recordingReference) {
      if (!frameStats.referenceReady) setReference(null);
      return;
    }
    let live = true;
    getReference()
      .then((r) => live && setReference(r ? { min: r.min, max: r.max } : null))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [isStreamingNow, frameStats.referenceReady, frameStats.recordingReference, getReference, rotation]);

  // Measurement settings (gain, emissivity): saved in the session, applied to the
  // Lepton (which forgets them on power-off) whenever streaming starts or they change.
  const [meas, setMeas] = useState<MeasurementConfig>({ ...DEFAULT_MEASUREMENT, ...session.measurement });
  const { getGainMode, setGainMode, setEmissivity } = camera;
  useEffect(() => {
    if (!isStreamingNow) return;
    // Only switch when needed: every gain change triggers an FFC (shutter click).
    getGainMode()
      .then((cur) => (cur !== meas.gain ? setGainMode(meas.gain) : undefined))
      .catch(() => {});
  }, [isStreamingNow, meas.gain, getGainMode, setGainMode]);
  useEffect(() => {
    if (!isStreamingNow) return;
    const t = setTimeout(() => setEmissivity(meas.emissivity, meas.reflectedC).catch(() => {}), 150);
    return () => clearTimeout(t);
  }, [isStreamingNow, meas.emissivity, meas.reflectedC, setEmissivity]);

  // Measurement tools (boxes / lines) and their trend history.
  const [tools, setTools] = useState<Tool[]>(() =>
    (session.tools ?? []).filter((t) => t && (t.kind === "box" || t.kind === "line")).slice(0, MAX_TOOLS)
  );
  const [selectedTool, setSelectedTool] = useState<number | null>(null);
  const [drawMode, setDrawMode] = useState<"box" | "line" | null>(null);
  const [graphOpen, setGraphOpen] = useState(session.graph?.open ?? true);
  const [trendMetric, setTrendMetric] = useState<TrendMetric>(session.graph?.metric ?? "max");
  const [trendWindow, setTrendWindow] = useState<TrendWindow>(session.graph?.window ?? 60);
  const samplesRef = useRef<TrendSample[]>([]);
  const [sampleTick, setSampleTick] = useState(0);
  // Next id / label numbers continue after the restored tools.
  const toolIds = useRef({
    next: Math.max(0, ...tools.map((t) => t.id)) + 1,
    box: Math.max(0, ...tools.filter((t) => t.kind === "box").map((t) => Number(t.label.slice(1)) || 0)) + 1,
    line: Math.max(0, ...tools.filter((t) => t.kind === "line").map((t) => Number(t.label.slice(1)) || 0)) + 1,
  });

  // Rotation (applied by the backend to every frame). Boxes and lines are kept in
  // displayed coordinates, so they turn with the image to stay on the same spot.
  const handleRotationChange = useCallback(
    (next: Rotation) => {
      if (next === rotation) return;
      const turn = (x: number, y: number) => rerotatePoint(x, y, DEFAULT_WIDTH, DEFAULT_HEIGHT, rotation, next);
      setTools((ts) =>
        ts.map((t) => {
          const a = turn(t.x0, t.y0);
          const b = turn(t.x1, t.y1);
          return { ...t, x0: a.x, y0: a.y, x1: b.x, y1: b.y };
        })
      );
      setRotationState(next);
    },
    [rotation]
  );
  const sendRotation = camera.setRotation;
  useEffect(() => {
    if (isStreamingNow) sendRotation(rotation).catch(() => {});
  }, [isStreamingNow, rotation, sendRotation]);

  // Other View / Analyze settings, (re-)sent whenever streaming starts.
  const [polarity, setPolarity] = useState<number>(session.polarity ?? 0);
  const [isotherm, setIsotherm] = useState<IsothermSettings>({ ...DEFAULT_ISOTHERM, ...session.isotherm });
  const [spot, setSpot] = useState(session.spot ?? { col: DEFAULT_WIDTH / 2, row: DEFAULT_HEIGHT / 2 });
  const [tab, setTab] = useState<PanelTab>(session.tab ?? "view");
  const { setPalette: sendPalette, setPolarity: sendPolarity, setIsotherm: sendIsotherm, setUpscale: sendUpscale } =
    camera;
  useEffect(() => {
    if (isStreamingNow) sendPalette(palette).catch(() => {});
  }, [isStreamingNow, palette, sendPalette]);
  useEffect(() => {
    if (isStreamingNow) sendPolarity(polarity).catch(() => {});
  }, [isStreamingNow, polarity, sendPolarity]);
  useEffect(() => {
    if (!isStreamingNow) return;
    // Raw Y16 in centikelvin: (°C + 273.15) * 100; 0 = off.
    sendIsotherm(isotherm.enabled ? Math.round((isotherm.tempC + 273.15) * 100) : 0).catch(() => {});
  }, [isStreamingNow, isotherm, sendIsotherm]);
  useEffect(() => {
    if (isStreamingNow) sendUpscale(upscaleEnabled).catch(() => {});
  }, [isStreamingNow, upscaleEnabled, sendUpscale]);
  const sendRoi = camera.setSpotmeterRoi;
  useEffect(() => {
    if (!isStreamingNow) return;
    const size = 2;
    sendRoi(
      Math.max(0, spot.row - size),
      Math.max(0, spot.col - size),
      Math.min(DEFAULT_HEIGHT - 1, spot.row + size),
      Math.min(DEFAULT_WIDTH - 1, spot.col + size)
    ).catch(() => {});
  }, [isStreamingNow, spot, sendRoi]);

  // Save the session (session.json) shortly after any change.
  useEffect(() => {
    saveSession({
      theme: theme.mode,
      tab,
      palette,
      polarity: polarity ? 1 : 0,
      contrast,
      hold,
      markers: showMarkers,
      upscale: upscaleEnabled,
      rotation,
      displayRange,
      diff,
      isotherm,
      measurement: meas,
      tools,
      graph: { open: graphOpen, metric: trendMetric, window: trendWindow },
      spot,
    });
  }, [
    theme.mode, tab, palette, polarity, contrast, hold, showMarkers, upscaleEnabled, rotation, displayRange,
    diff, isotherm, meas, tools, graphOpen, trendMetric, trendWindow, spot,
  ]);

  const toolStatsMap = useMemo(() => {
    const out: Record<number, ToolStats | null> = {};
    if (frameStats.raw) for (const t of tools) out[t.id] = toolStats(frameStats.raw, t);
    return out;
  }, [frameStats.raw, tools]);

  useEffect(() => {
    if (!isStreamingNow || tools.length === 0 || !frameStats.raw) return;
    const stats: TrendSample["stats"] = {};
    for (const t of tools) {
      const s = toolStatsMap[t.id];
      if (s) stats[t.id] = { min: s.min, max: s.max, avg: s.avg };
    }
    const buf = samplesRef.current;
    buf.push({ t: Date.now(), stats });
    if (buf.length > 40000) buf.splice(0, buf.length - 36000); // keep ~1 h at 8.7 fps
    setSampleTick((n) => n + 1);
  }, [toolStatsMap]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleToolDrawn = useCallback(
    (kind: "box" | "line", x0: number, y0: number, x1: number, y1: number) => {
      setDrawMode(null);
      if (tools.length >= MAX_TOOLS) return;
      // Colors follow the tool: take the first free slot, never re-color others.
      const used = new Set(tools.map((t) => t.slot));
      const slot = [1, 2, 3, 4, 5, 6].find((s) => !used.has(s)) ?? 1;
      const ids = toolIds.current;
      const id = ids.next++;
      const label = kind === "box" ? `B${ids.box++}` : `L${ids.line++}`;
      setTools([...tools, { id, kind, label, slot, x0, y0, x1, y1 }]);
      setSelectedTool(id);
    },
    [tools]
  );

  const handleExportCsv = useCallback(() => {
    const csv = trendCsv(samplesRef.current, tools);
    const url = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
    const link = document.createElement("a");
    link.download = `thermal-trend-${new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19)}.csv`;
    link.href = url;
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }, [tools]);

  const containerSize = useElementSize(videoContainerRef);

  const handleCapture = useCallback(() => {
    const canvas = videoContainerRef.current?.querySelector("canvas");
    if (!canvas) return;
    const timestamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
    const link = document.createElement("a");
    link.download = `thermal-${timestamp}.png`;
    link.href = canvas.toDataURL("image/png");
    link.click();
  }, []);

  const handleStats = useCallback((stats: FrameStats) => {
    setFrameStats(stats);
  }, []);

  // Palette, polarity, upscale, isotherm and spot meter are sent by the effects
  // above (also after a reconnect); the handlers only change state.
  // Auto-connect: watch for the camera and connect as soon as it is plugged in.
  const { connect: cameraConnect, startStream: cameraStart, disconnect: cameraDisconnect, getStatus } = camera;
  const handleConnect = useCallback(async (): Promise<string | null> => {
    const err = await cameraConnect();
    if (err !== null) return err;
    const streamErr = await cameraStart();
    if (streamErr !== null) await cameraDisconnect();
    return streamErr;
  }, [cameraConnect, cameraStart, cameraDisconnect]);
  const autoConnect = useAutoConnect({
    streaming: camera.state === "streaming",
    getStatus,
    connect: handleConnect,
  });

  const handleSpotChange = useCallback((col: number, row: number) => setSpot({ col, row }), []);

  // No frames for 3 s (unplugged / stalled): release the camera; auto-connect
  // takes over and reconnects when it is back.
  const handleDisconnect = useCallback(() => {
    cameraDisconnect().catch(() => {});
  }, [cameraDisconnect]);

  const isStreaming = camera.state === "streaming";
  const showRadiometry = camera.deviceInfo?.supports_radiometry ?? false;

  // Simulated camera (synthetic scene, no hardware needed).
  const [simulating, setSimulating] = useState(false);
  useEffect(() => {
    invoke<boolean>("get_simulation").then(setSimulating).catch(() => {});
  }, []);
  const setSimulation = useCallback(async (on: boolean) => {
    await invoke("set_simulation", { enabled: on }).catch(() => {});
    setSimulating(on);
    await camera.disconnect();
    autoConnect.retryNow();
  }, [camera, autoConnect]);

  // Full screen: hide the side panels and, where the platform allows it,
  // make the window itself full screen (desktop). Android: panels only.
  const [fullscreen, setFullscreen] = useState(false);
  const toggleFullscreen = useCallback(() => {
    setFullscreen((on) => {
      const next = !on;
      getCurrentWindow().setFullscreen(next).catch(() => {});
      return next;
    });
  }, []);
  useEffect(() => {
    if (!fullscreen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") toggleFullscreen(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [fullscreen, toggleFullscreen]);

  return (
    <div className={fullscreen ? "app app--fullscreen" : "app"}>
      <ControlPanel
        tab={tab}
        onTabChange={setTab}
        badges={{ analyze: hold !== "off" || diffActive || tools.length > 0 }}
        view={
          <ViewControls
            currentPalette={palette}
            onPaletteChange={setPalette}
            polarity={polarity}
            onPolarityChange={setPolarity}
            rotation={rotation}
            onRotationChange={handleRotationChange}
            contrast={contrast}
            onContrastChange={setContrast}
            showMarkers={showMarkers}
            onToggleMarkers={() => setShowMarkers((v) => !v)}
            upscaleEnabled={upscaleEnabled}
            onToggleUpscale={() => setUpscaleEnabled((v) => !v)}
            onCapture={handleCapture}
            streaming={isStreaming}
          />
        }
        analyze={
          <>
            <PeakHoldControls
              hold={hold}
              onHoldChange={setHold}
              onHoldReset={() => resetHold().catch(() => {})}
              holdFrames={frameStats.holdFrames}
            />
            <DiffControls
              settings={diff}
              onChange={setDiff}
              onRecord={handleRecordReference}
              onClear={handleClearReference}
              referenceReady={isStreaming && frameStats.referenceReady}
              recording={isStreaming && frameStats.recordingReference}
              streaming={isStreaming}
            />
            <MeasureControls
              tools={tools}
              stats={toolStatsMap}
              selectedId={selectedTool}
              onSelect={setSelectedTool}
              onRemove={(id) => setTools((ts) => ts.filter((t) => t.id !== id))}
              onClearAll={() => {
                setTools([]);
                samplesRef.current = [];
                setSampleTick((n) => n + 1);
              }}
              drawMode={drawMode}
              onDraw={setDrawMode}
              onExportCsv={handleExportCsv}
              samples={samplesRef.current.length}
              graphOpen={graphOpen}
              onToggleGraph={() => setGraphOpen((v) => !v)}
              streaming={isStreaming}
            />
            <IsothermControls settings={isotherm} onChange={setIsotherm} />
          </>
        }
        device={
          <>
            <DeviceInfo info={camera.deviceInfo} />
            <MeasurementSettings config={meas} onChange={setMeas} />
            <div className="control-section">
              <button onClick={camera.performFfc} className="ffc-button" title="Flat-field correction (shutter click)">
                Run FFC
              </button>
            </div>
            <ThemeSwitcher mode={theme.mode} onChange={theme.setMode} />
          </>
        }
      />
      <main className="video-area">
        {simulating && (
          <button className="sim-badge" onClick={() => setSimulation(false)} title="Stop the simulated camera">
            SIMULATION <span aria-hidden="true">✕</span>
          </button>
        )}
        <button
          className="fullscreen-btn"
          onClick={toggleFullscreen}
          title={fullscreen ? "Exit full screen (Esc)" : "Full screen"}
          aria-label={fullscreen ? "Exit full screen" : "Full screen"}
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            {fullscreen ? (
              <path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />
            ) : (
              <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
            )}
          </svg>
        </button>
        <div className="video-stage">
        {!isStreaming && <WaitingScreen phase={autoConnect.phase} onRetry={autoConnect.retryNow} onSimulate={() => setSimulation(true)} />}
        {isStreaming && camera.error && (
          <div className="error" onClick={() => camera.clearError()} style={{ cursor: "pointer" }}>
            {camera.error}
          </div>
        )}
        <div
          className="video-container"
          ref={videoContainerRef}
          onMouseMove={handlePointerMove}
          onMouseLeave={() => setPointer(null)}
        >
          <VideoCanvas
            streaming={isStreaming}
            onStats={handleStats}
            onDisconnect={handleDisconnect}
            className="thermal-video"
          />
          {isStreaming && showMarkers && (
            <MarkersOverlay
              width={frameStats.raw?.width ?? DEFAULT_WIDTH}
              height={frameStats.raw?.height ?? DEFAULT_HEIGHT}
              minPos={frameStats.minPos}
              maxPos={frameStats.maxPos}
              minVal={frameStats.minVal}
              maxVal={frameStats.maxVal}
            />
          )}
          {isStreaming && showRadiometry && (
            <SpotmeterOverlay
              col={spot.col}
              row={spot.row}
              sensorW={DEFAULT_WIDTH}
              sensorH={DEFAULT_HEIGHT}
              rotation={rotation}
              containerW={containerSize.width}
              containerH={containerSize.height}
              onSpotChange={handleSpotChange}
            />
          )}
          {isStreaming && (tools.length > 0 || drawMode) && (
            <MeasureOverlay
              tools={tools}
              stats={toolStatsMap}
              selectedId={selectedTool}
              drawMode={drawMode}
              onDrawn={handleToolDrawn}
              onCancelDraw={() => setDrawMode(null)}
              containerW={containerSize.width}
              containerH={containerSize.height}
              sensorW={frameStats.raw?.width ?? DEFAULT_WIDTH}
              sensorH={frameStats.raw?.height ?? DEFAULT_HEIGHT}
            />
          )}
          {isStreaming && showRadiometry && (
            <HoverReadout
              pointer={pointer}
              containerW={pointer?.w ?? 0}
              containerH={pointer?.h ?? 0}
              raw={frameStats.raw}
              tag={hold === "max" ? "max" : hold === "min" ? "min" : undefined}
              reference={diffActive && reference ? { ...reference, thresholdC: diff.thresholdC } : null}
            />
          )}
        </div>
        {isStreaming && (
          <div className="status-bar">
            <div className="status-indicator">
              <div className="status-dot live" />
              <span>LIVE</span>
              {hold !== "off" && <span className="status-badge">{hold.toUpperCase()} HOLD</span>}
              {contrast === "equalized" && <span className="status-badge">HEQ</span>}
            </div>
            <span>{frameStats.width}x{frameStats.height}</span>
          </div>
        )}
        </div>
        {isStreaming && graphOpen && tools.length > 0 && (
          <ChartDock
            tools={tools}
            stats={toolStatsMap}
            samples={samplesRef.current}
            sampleTick={sampleTick}
            selectedId={selectedTool}
            metric={trendMetric}
            onMetric={setTrendMetric}
            windowS={trendWindow}
            onWindow={setTrendWindow}
          />
        )}
      </main>
      {isStreaming && (
        <aside className="info-panel">
          {showRadiometry && (
            <TemperatureDisplay
              getSpotTemperature={camera.getSpotTemperature}
              streaming={isStreaming}
            />
          )}
          {diffActive ? (
            <DiffLegend settings={diff} onChange={setDiff} />
          ) : (
            <PaletteBar
              minVal={frameStats.minVal}
              maxVal={frameStats.maxVal}
              palette={palette}
              range={displayRange}
              onRangeChange={setDisplayRange}
              equalized={contrast === "equalized"}
              ticks={frameStats.scaleTicks}
            />
          )}
        </aside>
      )}
    </div>
  );
}

export default App;
