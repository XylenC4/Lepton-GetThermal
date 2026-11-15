import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ConnectionState, DeviceInfo, Palette } from "../lib/types";

export function useCamera() {
  const [state, setState] = useState<ConnectionState>("disconnected");
  const [deviceInfo, setDeviceInfo] = useState<DeviceInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  /** Opens the camera; resolves to the error message, or null on success. */
  const connect = useCallback(async (): Promise<string | null> => {
    try {
      setState("connecting");
      setError(null);
      await invoke("connect_camera");
      const info = await invoke<DeviceInfo>("get_device_info");
      setDeviceInfo(info);
      setState("connected");
      return null;
    } catch (e) {
      setError(String(e));
      setState("error");
      return String(e);
    }
  }, []);

  const startStream = useCallback(async (): Promise<string | null> => {
    try {
      await invoke("start_stream");
      setState("streaming");
      return null;
    } catch (e) {
      setError(String(e));
      setState("error");
      return String(e);
    }
  }, []);

  /** Releases the camera (unplugged / not responding) and returns to "disconnected". */
  const disconnect = useCallback(async () => {
    await invoke("disconnect_camera").catch(() => {});
    setDeviceInfo(null);
    setState("disconnected");
  }, []);

  /** Cheap presence check for auto-connect (opens nothing). */
  const getStatus = useCallback(
    async () => invoke<{ present: boolean | null; problem: string | null }>("camera_status"),
    []
  );

  const stopStream = useCallback(async () => {
    try {
      await invoke("stop_stream");
      setState("connected");
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setPalette = useCallback(async (palette: Palette) => {
    try {
      await invoke("set_palette", { palette });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const performFfc = useCallback(async () => {
    try {
      await invoke("perform_ffc");
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setPolarity = useCallback(async (polarity: number) => {
    await invoke("set_polarity", { polarity });
  }, []);

  const setIsotherm = useCallback(async (rawThreshold: number) => {
    await invoke("set_isotherm", { rawThreshold });
  }, []);

  /** Fix the color mapping to raw Y16 min..max; null = automatic. */
  const setDisplayRange = useCallback(async (minRaw: number | null, maxRaw: number | null) => {
    await invoke("set_display_range", { minRaw, maxRaw });
  }, []);

  /** Difference mode: record each pixel's min/max over `frames` as reference. */
  const recordReference = useCallback(async (frames = 16) => {
    await invoke("record_reference", { frames });
  }, []);

  const clearReference = useCallback(async () => {
    await invoke("clear_reference");
  }, []);

  /** Difference mode settings, in raw Y16 units. */
  const setDiffMode = useCallback(
    async (enabled: boolean, thresholdRaw: number, warmFullRaw: number, coldFullRaw: number) => {
      await invoke("set_diff_mode", { enabled, thresholdRaw, warmFullRaw, coldFullRaw });
    },
    []
  );

  const setContrastMode = useCallback(async (mode: "linear" | "equalized") => {
    await invoke("set_contrast_mode", { mode });
  }, []);

  const setHoldMode = useCallback(async (mode: "off" | "max" | "min") => {
    await invoke("set_hold_mode", { mode });
  }, []);

  const resetHold = useCallback(async () => {
    await invoke("reset_hold");
  }, []);

  const getGainMode = useCallback(async () => invoke<"high" | "low" | "auto">("get_gain_mode"), []);

  /** Changes the Lepton gain and runs an FFC. */
  const setGainMode = useCallback(async (mode: "high" | "low" | "auto") => {
    await invoke("set_gain_mode", { mode });
  }, []);

  const setEmissivity = useCallback(async (emissivity: number, reflectedC: number) => {
    await invoke("set_emissivity", { emissivity, reflectedC });
  }, []);

  /** Difference reference envelope (per-pixel min/max), or null. */
  const getReference = useCallback(async () => {
    const r = await invoke<{ min: string; max: string; width: number; height: number } | null>("get_reference");
    if (!r) return null;
    const dec = (b64: string) => {
      const bin = atob(b64);
      const out = new Uint16Array(bin.length >> 1);
      for (let i = 0; i < out.length; i++) out[i] = bin.charCodeAt(2 * i) | (bin.charCodeAt(2 * i + 1) << 8);
      return out;
    };
    return { min: dec(r.min), max: dec(r.max), width: r.width, height: r.height };
  }, []);

  const setUpscale = useCallback(async (enabled: boolean) => {
    try {
      await invoke("set_upscale", { enabled });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /** Display rotation, degrees clockwise. */
  const setRotation = useCallback(async (degrees: number) => {
    await invoke("set_rotation", { degrees });
  }, []);

  /** Dismisses the error message (the connection state is left alone). */
  const clearError = useCallback(() => {
    setError(null);
  }, []);

  const getSpotTemperature = useCallback(async (): Promise<number> => {
    return invoke<number>("get_spot_temperature");
  }, []);

  const setSpotmeterRoi = useCallback(
    async (rowStart: number, colStart: number, rowEnd: number, colEnd: number) => {
      try {
        await invoke("set_spotmeter_roi", {
          rowStart: Math.round(rowStart),
          colStart: Math.round(colStart),
          rowEnd: Math.round(rowEnd),
          colEnd: Math.round(colEnd),
        });
      } catch (e) {
        setError(`ROI error: ${String(e)}`);
      }
    },
    []
  );

  return {
    state,
    deviceInfo,
    error,
    connect,
    startStream,
    stopStream,
    setPalette,
    performFfc,
    setPolarity,
    setIsotherm,
    setRotation,
    disconnect,
    getStatus,
    setDisplayRange,
    recordReference,
    clearReference,
    setDiffMode,
    setContrastMode,
    setHoldMode,
    resetHold,
    getGainMode,
    setGainMode,
    setEmissivity,
    getReference,
    clearError,
    getSpotTemperature,
    setSpotmeterRoi,
    setUpscale,
  };
}
