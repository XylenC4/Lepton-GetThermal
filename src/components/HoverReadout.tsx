import { RawFrame } from "../hooks/useFrameStream";

interface HoverReadoutProps {
  /** Pointer position relative to the video container (px), or null when outside. */
  pointer: { x: number; y: number } | null;
  /** Container size (px). */
  containerW: number;
  containerH: number;
  raw: RawFrame | null;
  /** Shown next to the value, e.g. "max" in max-hold mode. */
  tag?: string;
}

/** Raw Y16 to °C: TLinear 0.01 K (> 10000) or 0.1 K. */
function rawToC(raw: number): number {
  return raw > 10000 ? (raw - 27315) / 100 : (raw - 2731.5) / 10;
}

/** Temperature under the mouse pointer, read from the displayed raw frame. */
export function HoverReadout({ pointer, containerW, containerH, raw, tag }: HoverReadoutProps) {
  if (!pointer || !raw || containerW === 0 || containerH === 0) return null;

  // The canvas is drawn with object-fit: contain: find the image rectangle.
  const scale = Math.min(containerW / raw.width, containerH / raw.height);
  const w = raw.width * scale;
  const h = raw.height * scale;
  const left = (containerW - w) / 2;
  const top = (containerH - h) / 2;
  const u = (pointer.x - left) / w;
  const v = (pointer.y - top) / h;
  if (u < 0 || u >= 1 || v < 0 || v >= 1) return null;

  const col = Math.min(raw.width - 1, Math.floor(u * raw.width));
  const row = Math.min(raw.height - 1, Math.floor(v * raw.height));
  const value = raw.data[row * raw.width + col];
  if (value === undefined) return null;

  // Keep the label inside the image: flip to the other side near the edges.
  const flipX = pointer.x > left + w - 90;
  const flipY = pointer.y > top + h - 34;

  return (
    <div
      className="hover-readout"
      style={{
        left: pointer.x,
        top: pointer.y,
        transform: `translate(${flipX ? "calc(-100% - 12px)" : "12px"}, ${flipY ? "calc(-100% - 12px)" : "12px"})`,
      }}
    >
      <span className="hover-value">{rawToC(value).toFixed(1)}&deg;C</span>
      {tag && <span className="hover-tag">{tag}</span>}
      <span className="hover-pos">
        {col},{row}
      </span>
    </div>
  );
}
