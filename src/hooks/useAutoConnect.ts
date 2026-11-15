import { useEffect, useRef, useState } from "react";

/** What the waiting screen shows while no camera is streaming. */
export type WaitPhase =
  | { kind: "searching" } // nothing plugged in
  | { kind: "connecting" }
  | { kind: "problem"; message: string } // plugged in, but not usable (driver)
  | { kind: "failed"; message: string }; // connect failed, retrying

interface Options {
  /** True while a stream is running (polling pauses). */
  streaming: boolean;
  getStatus: () => Promise<{ present: boolean | null; problem: string | null }>;
  /** Connect + start the stream; resolves to the error message, or null on success. */
  connect: () => Promise<string | null>;
}

const POLL_MS = 1500;
/** Wait between connect attempts on a camera that is present but fails to open. */
const RETRY_MS = 4000;

/**
 * Watches for the PureThermal and connects as soon as it is plugged in, also
 * after it was unplugged during a session.
 */
export function useAutoConnect({ streaming, getStatus, connect }: Options) {
  const [phase, setPhase] = useState<WaitPhase>({ kind: "searching" });
  const lastAttempt = useRef(0);
  const failed = useRef(false);
  const busy = useRef(false);

  // Ask for an immediate attempt ("Try now").
  const [kick, setKick] = useState(0);

  useEffect(() => {
    if (streaming) {
      failed.current = false;
      return;
    }
    let alive = true;

    const tick = async () => {
      if (busy.current || !alive) return;
      busy.current = true;
      try {
        const s = await getStatus().catch(() => ({ present: null, problem: null }));
        if (!alive) return;
        if (s.present === false) {
          failed.current = false;
          setPhase({ kind: "searching" });
          return;
        }
        if (s.problem) {
          setPhase({ kind: "problem", message: s.problem });
          return;
        }
        const now = Date.now();
        if (failed.current && now - lastAttempt.current < RETRY_MS) return;
        lastAttempt.current = now;
        setPhase({ kind: "connecting" });
        const err = await connect();
        if (!alive) return;
        failed.current = err !== null;
        if (err !== null) {
          // macOS can't check presence: a missing camera is just "searching".
          const msg = err || "Unknown error";
          setPhase(
            s.present === null && /not found/i.test(msg) ? { kind: "searching" } : { kind: "failed", message: msg }
          );
        }
      } finally {
        busy.current = false;
      }
    };

    void tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [streaming, getStatus, connect, kick]);

  return {
    phase,
    retryNow: () => {
      failed.current = false;
      setKick((k) => k + 1);
    },
  };
}
