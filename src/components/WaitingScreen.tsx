import type { WaitPhase } from "../hooks/useAutoConnect";

interface WaitingScreenProps {
  phase: WaitPhase;
  onRetry: () => void;
  onSimulate?: () => void;
}

const TEXT: Record<WaitPhase["kind"], { title: string; body: string }> = {
  searching: {
    title: "No camera connected",
    body: "Plug in the PureThermal board with the Lepton. It connects automatically.",
  },
  connecting: { title: "Connecting…", body: "Camera found, starting the stream." },
  problem: { title: "Camera found, but it can't be used", body: "" },
  failed: { title: "Couldn't connect to the camera", body: "" },
};

/** Shown instead of the image while no camera is streaming. */
export function WaitingScreen({ phase, onRetry, onSimulate }: WaitingScreenProps) {
  const t = TEXT[phase.kind];
  const busy = phase.kind === "searching" || phase.kind === "connecting";
  const message = "message" in phase ? phase.message : null;

  return (
    <div className={`waiting-screen ${phase.kind}`} role="status" aria-live="polite">
      <div className={`waiting-icon ${busy ? "pulse" : ""}`} aria-hidden="true">
        <svg viewBox="0 0 24 24">
          {phase.kind === "problem" || phase.kind === "failed" ? (
            <>
              <path d="M12 3 2 20h20L12 3z" />
              <path d="M12 10v4M12 17v.5" />
            </>
          ) : (
            <>
              <rect x="3" y="6" width="14" height="12" rx="2" />
              <path d="m17 10 4-2v8l-4-2" />
              <circle cx="10" cy="12" r="2.5" />
            </>
          )}
        </svg>
      </div>
      <h2 className="waiting-title">{t.title}</h2>
      {t.body && <p className="waiting-body">{t.body}</p>}
      {message && <p className="waiting-message">{message}</p>}
      {phase.kind === "failed" && <p className="waiting-body">Retrying every few seconds.</p>}
      {phase.kind === "problem" && <p className="waiting-body">Checking again every few seconds.</p>}
      {(phase.kind === "failed" || phase.kind === "problem") && (
        <button type="button" className="waiting-retry" onClick={onRetry}>
          Try now
        </button>
      )}
      {onSimulate && phase.kind !== "connecting" && (
        <button type="button" className="waiting-retry waiting-sim" onClick={onSimulate}>
          No camera? Run simulation
        </button>
      )}
      {phase.kind === "searching" && (
        <div className="waiting-dots" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
      )}
    </div>
  );
}
