/** Display rotation, degrees clockwise. The backend rotates every frame. */
export type Rotation = 0 | 90 | 180 | 270;

export const ROTATIONS: Rotation[] = [0, 90, 180, 270];

export function asRotation(v: unknown): Rotation {
  return ROTATIONS.includes(v as Rotation) ? (v as Rotation) : 0;
}

/** Size of the displayed image for a sensor of `w`x`h`. */
export function rotatedSize(w: number, h: number, r: Rotation) {
  return r === 90 || r === 270 ? { w: h, h: w } : { w, h };
}

/** Sensor pixel -> displayed pixel (sensor `w`x`h`). */
export function sensorToDisplay(x: number, y: number, w: number, h: number, r: Rotation) {
  switch (r) {
    case 90:
      return { x: h - 1 - y, y: x };
    case 180:
      return { x: w - 1 - x, y: h - 1 - y };
    case 270:
      return { x: y, y: w - 1 - x };
    default:
      return { x, y };
  }
}

/** Displayed pixel -> sensor pixel (sensor `w`x`h`). */
export function displayToSensor(x: number, y: number, w: number, h: number, r: Rotation) {
  switch (r) {
    case 90:
      return { x: y, y: h - 1 - x };
    case 180:
      return { x: w - 1 - x, y: h - 1 - y };
    case 270:
      return { x: w - 1 - y, y: x };
    default:
      return { x, y };
  }
}

/** Moves a displayed point from rotation `from` to rotation `to` (same scene pixel). */
export function rerotatePoint(x: number, y: number, w: number, h: number, from: Rotation, to: Rotation) {
  const s = displayToSensor(x, y, w, h, from);
  return sensorToDisplay(s.x, s.y, w, h, to);
}
