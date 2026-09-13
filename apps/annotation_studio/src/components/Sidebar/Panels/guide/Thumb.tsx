// apps/annotation_studio/src/components/Sidebar/Panels/guide/Thumb.tsx
// Segment: thumb outer/inner edges + joint tick, bridging the silhouette gap
// between PALM_OUTLINE_D and PALM_OUTLINE_LOWER_D without overlaps.
import React from 'react';
import { GUIDE_THUMB } from './handAnatomy';
import { mirrorTransform, type Handedness } from './mirror';

interface ThumbProps {
  outlineStroke: string;
  handedness: Handedness;
}

export const Thumb: React.FC<ThumbProps> = ({ outlineStroke, handedness }) => {
  const [tx1, ty1, tx2, ty2] = GUIDE_THUMB.jointTick;
  return (
    <g transform={mirrorTransform(handedness)}>
      <path
        d={GUIDE_THUMB.outerEdgeD}
        fill="none"
        stroke={outlineStroke}
        strokeWidth={2.5}
        strokeLinecap="round"
      />
      <path
        d={GUIDE_THUMB.innerEdgeD}
        fill="none"
        stroke={outlineStroke}
        strokeWidth={2.5}
        strokeLinecap="round"
      />
      <line x1={tx1} y1={ty1} x2={tx2} y2={ty2} stroke="rgba(148,163,184,0.35)" strokeWidth={1.5} />
    </g>
  );
};
