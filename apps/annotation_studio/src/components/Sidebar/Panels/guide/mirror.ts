// apps/annotation_studio/src/components/Sidebar/Panels/guide/mirror.ts
// Left/right hand support. The canonical artwork is drawn for the RIGHT hand
// (thumb right, pinky left — matching the reference chart). For the LEFT hand
// all geometry mirrors horizontally; text labels are repositioned (never
// mirrored, so glyphs always read correctly).

export type Handedness = 'left' | 'right';

export const GUIDE_VB_W = 220;

export function mirrorTransform(handedness: Handedness): string | undefined {
  return handedness === 'left' ? `translate(${GUIDE_VB_W} 0) scale(-1 1)` : undefined;
}

export function mirrorX(x: number, handedness: Handedness): number {
  return handedness === 'left' ? GUIDE_VB_W - x : x;
}
