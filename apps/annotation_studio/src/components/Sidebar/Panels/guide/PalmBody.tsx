// apps/annotation_studio/src/components/Sidebar/Panels/guide/PalmBody.tsx
// Segment: palm outline + decorative contours + wrist bracelets.
import React from 'react';
import { PALM_OUTLINE_D, PALM_OUTLINE_LOWER_D, ANATOMY_CONTOURS } from './handAnatomy';
import { mirrorTransform, type Handedness } from './mirror';

interface PalmBodyProps {
  outlineStroke: string;
  handedness: Handedness;
}

export const PalmBody: React.FC<PalmBodyProps> = ({ outlineStroke, handedness }) => (
  <g transform={mirrorTransform(handedness)}>
    <path
      d={PALM_OUTLINE_D}
      fill="none"
      stroke={outlineStroke}
      strokeWidth={2.5}
      strokeLinejoin="round"
    />
    <path
      d={PALM_OUTLINE_LOWER_D}
      fill="none"
      stroke={outlineStroke}
      strokeWidth={2.5}
      strokeLinejoin="round"
    />
    {ANATOMY_CONTOURS.map((c, i) => (
      <path
        key={i}
        d={c.d}
        fill="none"
        stroke="rgba(148,163,184,0.35)"
        strokeWidth={c.width}
      />
    ))}
  </g>
);
