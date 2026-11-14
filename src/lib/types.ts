export interface FrameEvent {
  data: string; // base64-encoded RGBA
  width: number;
  height: number;
  min_val: number;
  max_val: number;
  min_pos: number;
  max_pos: number;
  reference_ready: boolean;
  recording_reference: boolean;
  raw: string; // base64 raw Y16 LE of the displayed frame, sensor resolution
  raw_width: number;
  raw_height: number;
  hold_frames: number; // peak hold: frames accumulated (0 = off)
  scale_ticks: number[]; // raw values at every 1/16 of the color scale (17 points)
}

export interface DeviceInfo {
  serial_number: string;
  part_number: string;
  firmware_version: string;
  supports_radiometry: boolean;
  supports_hw_pseudo_color: boolean;
  width: number;
  height: number;
  fps: number;
  usb_driver: string;
}

export type Palette = "ironblack" | "rainbow" | "grayscale";

export type ConnectionState =
  | "disconnected"
  | "connecting"
  | "connected"
  | "streaming"
  | "error";

// Default Lepton sensor dimensions
export const DEFAULT_WIDTH = 160;
export const DEFAULT_HEIGHT = 120;
export const DEFAULT_FPS = 9;
