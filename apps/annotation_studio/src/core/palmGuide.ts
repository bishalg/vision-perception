// apps/annotation_studio/src/core/palmGuide.ts
// Pure data + mapping for the palm crease hint guide (no DOM/React).
// The guide recreates the palmistry reference chart as vector paths so the
// currently-selected annotation class can be highlighted in its own color.
//
// Geometry contract (viewBox 0 0 220 250, right hand, thumb on the right):
// - Heart / head lines run near-horizontal with a slight tilt.
// - Life line is a C-curve arcing around the Venus (thenar) mount.
// - Fate line runs near-vertical with a slight tilt, wrist -> Saturn mount.

export interface PalmGuideLine {
  /** Annotation class id this guide line corresponds to. */
  id: string;
  /** Short label rendered next to the line in the preview. */
  label: string;
  /** One-line operator hint shown under the preview. */
  description: string;
  /** SVG path data (viewBox 0 0 220 250) tracing the crease. */
  path: string;
  /** Anchor for the text label [x, y]. */
  labelAt: [number, number];
}

export const PALM_GUIDE_LINES: PalmGuideLine[] = [
  {
    id: 'heart_line',
    label: 'Heart line',
    description: 'Upper transverse crease below MCP joints.',
    path: 'M42,152 C80,145 118,139 142,133',
    labelAt: [96, 134],
  },
  {
    id: 'head_line',
    label: 'Head line',
    description: 'Middle diagonal crease across thenar space.',
    path: 'M46,174 C86,168 120,160 148,152',
    labelAt: [124, 168],
  },
  {
    id: 'life_line',
    label: 'Life line',
    description: 'C-curve arcing around the Venus (thenar) mount.',
    path: 'M140,135 C126,146 110,158 106,178 C103,196 110,218 125,238',
    labelAt: [118, 195],
  },
  {
    id: 'fate_line',
    label: 'Fate line',
    description: 'Near-vertical line ascending towards Saturn finger.',
    path: 'M108,238 C106,205 110,175 105,136',
    labelAt: [88, 180],
  },
];

/**
 * Palm-zone guard: finger valleys (MCP joints) sit at y <= FINGER_ZONE_MAX_Y.
 * Crease geometry must live at y >= CREASE_ZONE_MIN_Y so lines can never
 * drift into the finger region. Enforced by tests + an SVG clip region.
 */
export const FINGER_ZONE_MAX_Y = 128;
export const CREASE_ZONE_MIN_Y = 130;
export const PALM_REGION_X: [number, number] = [38, 182];

/** Fallback stroke colors when a domain class has no explicit color. */
export const PALM_GUIDE_FALLBACK_COLORS: Record<string, string> = {
  heart_line: '#F43F5E',
  head_line: '#06B6D4',
  life_line: '#10B981',
  fate_line: '#8B5CF6',
};

/**
 * Infers the on-screen hand from a capture filename (e.g. 01_left_bishal.jpg).
 * Returns null when the name carries no handedness signal — callers keep
 * their current toggle state instead of guessing.
 */
export function inferHandednessFromFilename(filename: string | null | undefined): 'left' | 'right' | null {
  if (!filename) return null;
  const lower = filename.toLowerCase();
  // NOTE: \b does not fire next to underscores (01_left_bishal), so boundaries
  // are explicit non-alphanumerics instead.
  const B = '(?:^|[^a-z0-9])';
  const E = '(?:[^a-z0-9]|$)';
  if (new RegExp(`${B}left${E}`).test(lower) || new RegExp(`${B}l${E}`).test(lower)) return 'left';
  if (new RegExp(`${B}right${E}`).test(lower) || new RegExp(`${B}r${E}`).test(lower)) return 'right';
  return null;
}

export interface GuideHighlight {
  line: PalmGuideLine;
  /** True when the class is currently selected for drawing. */
  isActive: boolean;
}

/**
 * Resolves which guide line to emphasize for a given active class id.
 * Returns null for classes with no palm mapping (other domains) so callers
 * can render the neutral diagram.
 */
export function getGuideHighlight(activeClassId: string | null): GuideHighlight | null {
  if (!activeClassId) return null;
  const line = PALM_GUIDE_LINES.find(l => l.id === activeClassId);
  if (!line) return null;
  return { line, isActive: true };
}
