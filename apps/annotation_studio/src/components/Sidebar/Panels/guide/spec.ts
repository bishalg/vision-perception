// apps/annotation_studio/src/components/Sidebar/Panels/guide/spec.ts
// Canonical palm-guide specification builder (no DOM/React).
// This is the SINGLE SOURCE OF TRUTH shared by web, Android, and iOS:
// `pnpm guide:export` serializes it to shared/palm-guide/palm_guide.json,
// which the native camera overlays consume (see that folder's README).
// ViewBox space is 220x250; normalize with x/220, y/250.
import {
  PALM_GUIDE_LINES,
  PALM_GUIDE_FALLBACK_COLORS,
  FINGER_ZONE_MAX_Y,
  CREASE_ZONE_MIN_Y,
  PALM_REGION_X,
} from '../../../../core/palmGuide';
import {
  PALM_OUTLINE_D,
  PALM_OUTLINE_LOWER_D,
  PALM_SILHOUETTE_D,
  GUIDE_FINGERS,
  GUIDE_THUMB,
  ANATOMY_CONTOURS,
  MOUNT_LABELS,
} from './handAnatomy';
import { GUIDE_VB_W } from './mirror';

export const GUIDE_SPEC_VERSION = '1.0.0';
export const GUIDE_VIEWBOX_H = 250;

export function buildGuideSpec(): Record<string, unknown> {
  return {
    spec_version: GUIDE_SPEC_VERSION,
    viewBox: { width: GUIDE_VB_W, height: GUIDE_VIEWBOX_H },
    base_handedness: 'right',
    normalization: 'divide x by 220 and y by 250 for 0..1 landmark-style coordinates',
    zones: {
      finger_zone_max_y: FINGER_ZONE_MAX_Y,
      crease_zone_min_y: CREASE_ZONE_MIN_Y,
      palm_region_x: [...PALM_REGION_X],
    },
    silhouette_path: PALM_SILHOUETTE_D,
    outline_upper_path: PALM_OUTLINE_D,
    outline_lower_path: PALM_OUTLINE_LOWER_D,
    thumb: { ...GUIDE_THUMB },
    fingers: GUIDE_FINGERS.map(f => ({ ...f, jointTicks: f.jointTicks.map(t => [...t]) })),
    contours: ANATOMY_CONTOURS.map(c => ({ ...c })),
    mount_labels: MOUNT_LABELS.map(m => ({ ...m })),
    creases: PALM_GUIDE_LINES.map(l => ({ ...l, labelAt: [...l.labelAt] })),
    fallback_colors: { ...PALM_GUIDE_FALLBACK_COLORS },
  };
}
