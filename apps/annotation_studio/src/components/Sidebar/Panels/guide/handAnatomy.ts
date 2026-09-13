// apps/annotation_studio/src/components/Sidebar/Panels/guide/handAnatomy.ts
// Single source of truth for the hint-hand anatomy (no DOM/React).
// Each segment (palm body, fingers, thumb, contours, labels) is exported
// independently so it can be drawn — and tuned — in isolation.

export const PALM_OUTLINE_D =
  'M62,250 C60,220 55,190 50,165 C47,150 43,142 42,132 ' +
  'L40,94 Q40,83 49,83 Q57,83 57,94 L59,126 Q60,136 67,130 ' +
  'L64,56 Q64,44 74,44 Q84,44 84,56 L86,124 Q87,134 94,128 ' +
  'L91,26 Q91,14 102,14 Q113,14 113,26 L114,126 Q115,136 122,130 ' +
  'L119,46 Q119,34 130,34 Q141,34 141,46 L143,120 ' +
  'C147,130 153,138 160,144';

/** Lower silhouette: resumes at the thumb web-bottom, down the thenar edge. */
export const PALM_OUTLINE_LOWER_D =
  'M172,172 C175,185 178,200 176,215 C175,235 172,242 168,250 Z';

export interface GuideFinger {
  id: 'mercury' | 'apollo' | 'saturn' | 'jupiter';
  label: string;
  /** Finger label anchor [x, y] (above the fingertip). */
  labelAt: [number, number];
  /** Joint-crease ticks as [x1, y1, x2, y2]. */
  jointTicks: Array<[number, number, number, number]>;
}

/** Per-finger segments: edit one finger without touching the others. */
export const GUIDE_FINGERS: GuideFinger[] = [
  {
    id: 'mercury',
    label: 'Mercury',
    labelAt: [49, 76],
    jointTicks: [
      [41, 106, 55, 102],
      [42, 121, 56, 117],
    ],
  },
  {
    id: 'apollo',
    label: 'Apollo',
    labelAt: [75, 37],
    jointTicks: [
      [65, 71, 83, 67],
      [66, 89, 84, 85],
    ],
  },
  {
    id: 'saturn',
    label: 'Saturn',
    labelAt: [102, 8],
    jointTicks: [
      [91, 49, 111, 45],
      [92, 69, 112, 65],
    ],
  },
  {
    id: 'jupiter',
    label: 'Jupiter',
    labelAt: [130, 27],
    jointTicks: [
      [119, 63, 139, 59],
      [120, 81, 140, 77],
    ],
  },
];

export interface GuideThumb {
  outerEdgeD: string;
  innerEdgeD: string;
  jointTick: [number, number, number, number];
}
/** Thumb drawn as two independent edges sharing a rounded tip.
 * Bridges the silhouette gap: starts at the web-top end of PALM_OUTLINE_D
 * and lands on the start of PALM_OUTLINE_LOWER_D — one continuous hand.
 * Both edges curve monotonically from base to tip (and back) so they never
 * cross or loop back on themselves — that self-intersection is what made
 * the tip render as a detached blade/spike. */
export const GUIDE_THUMB: GuideThumb = {
  outerEdgeD: 'M160,144 C170,128 184,114 199,108',
  innerEdgeD: 'M199,108 C196,122 188,138 178,150 C174,155 172,163 172,172',
  jointTick: [176, 122, 186, 132],
};

export interface AnatomyStroke {
  d: string;
  width: number;
}

/** Decorative contours: hypothenar echo + wrist bracelets.
 * (No thenar echo: the life-line C-curve itself delineates the mount.) */
export const ANATOMY_CONTOURS: AnatomyStroke[] = [
  { d: 'M64,188 C68,208 72,226 76,240', width: 1 },
  { d: 'M74,239 C102,233 138,233 160,239', width: 1.5 },
  { d: 'M82,245 C106,240 132,240 152,245', width: 1.5 },
];

export interface AnatomyLabel {
  label: string;
  x: number;
  y: number;
}

export const MOUNT_LABELS: AnatomyLabel[] = [
  { label: 'Venus', x: 146, y: 214 },
  { label: 'Moon', x: 68, y: 224 },
  { label: 'Mars', x: 78, y: 190 },
];

function stripMoveto(d: string): string {
  return d.replace(/^M[\d.,\s-]+?(?=[CLQZ])/, '');
}

/**
 * Closed hand silhouette for the crease clip region: outline -> thumb outer
 * -> tip cap -> thumb inner -> lower outline -> wrist. Composed from the same
 * constants the visible PalmBody/Thumb modules draw, so the clip boundary
 * always matches the drawn silhouette with zero duplicated geometry.
 */
export const PALM_SILHOUETTE_D: string =
  PALM_OUTLINE_D +
  ' ' +
  stripMoveto(GUIDE_THUMB.outerEdgeD) +
  ' ' +
  stripMoveto(GUIDE_THUMB.innerEdgeD) +
  ' ' +
  stripMoveto(PALM_OUTLINE_LOWER_D);
