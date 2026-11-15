import { useCallback } from "react";
import { containedRect } from "../lib/measure";
import { Rotation, displayToSensor, rotatedSize, sensorToDisplay } from "../lib/rotation";

interface SpotmeterOverlayProps {
  /** Spot position, sensor pixel. */
  col: number;
  row: number;
  /** Sensor resolution (unrotated). */
  sensorW: number;
  sensorH: number;
  rotation: Rotation;
  containerW: number;
  containerH: number;
  /** New spot position, sensor pixel. */
  onSpotChange: (col: number, row: number) => void;
}

/** Spot meter reticle; click the image to move it. */
export function SpotmeterOverlay({
  col,
  row,
  sensorW,
  sensorH,
  rotation,
  containerW,
  containerH,
  onSpotChange,
}: SpotmeterOverlayProps) {
  const disp = rotatedSize(sensorW, sensorH, rotation);
  const r = containedRect(containerW || 1, containerH || 1, disp.w, disp.h);

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const box = e.currentTarget.getBoundingClientRect();
      const u = Math.floor(((e.clientX - box.left - r.left) / r.width) * disp.w);
      const v = Math.floor(((e.clientY - box.top - r.top) / r.height) * disp.h);
      if (u < 0 || v < 0 || u >= disp.w || v >= disp.h) return; // outside the image
      const s = displayToSensor(u, v, sensorW, sensorH, rotation);
      onSpotChange(s.x, s.y);
    },
    [r.left, r.top, r.width, r.height, disp.w, disp.h, sensorW, sensorH, rotation, onSpotChange]
  );

  const d = sensorToDisplay(col, row, sensorW, sensorH, rotation);
  const cx = r.left + (d.x + 0.5) * r.scale;
  const cy = r.top + (d.y + 0.5) * r.scale;

  return (
    <div
      className="spotmeter-overlay"
      onClick={handleClick}
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        width: "100%",
        height: "100%",
        cursor: "crosshair",
        zIndex: 5,
      }}
    >
      {/* Crosshair reticle */}
      <div
        style={{
          position: "absolute",
          left: cx,
          top: cy,
          transform: "translate(-50%, -50%)",
          pointerEvents: "none",
        }}
      >
        {/* Outer ring */}
        <div
          style={{
            width: 24,
            height: 24,
            border: "1.5px solid rgba(0, 230, 118, 0.8)",
            borderRadius: "50%",
            position: "relative",
          }}
        >
          {/* Center dot */}
          <div
            style={{
              position: "absolute",
              left: "50%",
              top: "50%",
              width: 3,
              height: 3,
              background: "#00e676",
              borderRadius: "50%",
              transform: "translate(-50%, -50%)",
              boxShadow: "0 0 4px rgba(0, 230, 118, 0.6)",
            }}
          />
        </div>
        {/* Crosshair lines */}
        <div style={{
          position: "absolute", top: "50%", left: -6, width: 6, height: 1,
          background: "rgba(0, 230, 118, 0.5)", transform: "translateY(-50%)",
        }} />
        <div style={{
          position: "absolute", top: "50%", right: -6, width: 6, height: 1,
          background: "rgba(0, 230, 118, 0.5)", transform: "translateY(-50%)",
        }} />
        <div style={{
          position: "absolute", left: "50%", top: -6, width: 1, height: 6,
          background: "rgba(0, 230, 118, 0.5)", transform: "translateX(-50%)",
        }} />
        <div style={{
          position: "absolute", left: "50%", bottom: -6, width: 1, height: 6,
          background: "rgba(0, 230, 118, 0.5)", transform: "translateX(-50%)",
        }} />
      </div>
    </div>
  );
}
