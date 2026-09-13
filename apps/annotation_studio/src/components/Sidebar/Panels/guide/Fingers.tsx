// apps/annotation_studio/src/components/Sidebar/Panels/guide/Fingers.tsx
// Segment: per-finger joint ticks + fingertip labels.
// The finger bodies live in the palm outline path; each finger's ticks and
// label are isolated here so one finger can be retuned without touching the
// others. Joint ticks never extend past their finger's x-band by construction.
import React from 'react';
import { GUIDE_FINGERS } from './handAnatomy';
import { mirrorTransform, mirrorX, type Handedness } from './mirror';

interface FingersProps {
  showLabels: boolean;
  handedness: Handedness;
}

export const Fingers: React.FC<FingersProps> = ({ showLabels, handedness }) => (
  <g>
    <g transform={mirrorTransform(handedness)}>
      {GUIDE_FINGERS.map(finger => (
        <g key={finger.id}>
          {finger.jointTicks.map(([x1, y1, x2, y2], i) => (
            <line
              key={i}
              x1={x1}
              y1={y1}
              x2={x2}
              y2={y2}
              stroke="rgba(148,163,184,0.35)"
              strokeWidth={1.5}
            />
          ))}
        </g>
      ))}
    </g>
    {showLabels &&
      GUIDE_FINGERS.map(finger => (
        <text
          key={finger.id}
          x={mirrorX(finger.labelAt[0], handedness)}
          y={finger.labelAt[1]}
          textAnchor="middle"
          fontSize={7}
          fill="rgba(148,163,184,0.85)"
        >
          {finger.label}
        </text>
      ))}
  </g>
);
