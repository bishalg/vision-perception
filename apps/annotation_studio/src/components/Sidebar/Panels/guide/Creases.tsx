// apps/annotation_studio/src/components/Sidebar/Panels/guide/Creases.tsx
// Segment: the four crease lines. Rendered inside an SVG clip region built
// from the closed hand silhouette, so a crease can NEVER paint outside the
// hand or drift into the finger zone — even if its path data is later
// mistuned. The clip id is caller-scoped (clipId prop) so multiple guide
// instances can coexist on one page (sidebar + camera overlay).
// Crease labels sit outside the clip (and mirror) so they always read well.
import React from 'react';
import { PALM_GUIDE_LINES, PALM_GUIDE_FALLBACK_COLORS } from '../../../../core/palmGuide';
import { PALM_SILHOUETTE_D } from './handAnatomy';
import { mirrorTransform, mirrorX, type Handedness } from './mirror';

interface CreasesProps {
  clipId: string;
  highlightId: string | null;
  colorFor: (classId: string) => string;
  handedness: Handedness;
}

const DIM_LINE = 'rgba(148,163,184,0.45)';

export const Creases: React.FC<CreasesProps> = ({ clipId, highlightId, colorFor, handedness }) => {
  const colorOf = (id: string): string => {
    try {
      const c = colorFor(id);
      if (c) return c;
    } catch {
      /* fall through to fallback */
    }
    return PALM_GUIDE_FALLBACK_COLORS[id] ?? '#94A3B8';
  };

  return (
    <g>
      <defs>
        <clipPath id={clipId}>
          <path d={PALM_SILHOUETTE_D} transform={mirrorTransform(handedness)} />
        </clipPath>
      </defs>
      <g clipPath={`url(#${clipId})`}>
        <g transform={mirrorTransform(handedness)}>
          {PALM_GUIDE_LINES.map(line => {
            const isActive = line.id === highlightId;
            const stroke = isActive ? colorOf(line.id) : DIM_LINE;
            return (
              <g key={line.id}>
                {isActive && (
                  <path
                    d={line.path}
                    fill="none"
                    stroke={stroke}
                    strokeWidth={7}
                    strokeLinecap="round"
                    opacity={0.3}
                  />
                )}
                <path
                  d={line.path}
                  fill="none"
                  stroke={stroke}
                  strokeWidth={isActive ? 3.5 : 2}
                  strokeLinecap="round"
                  opacity={isActive ? 1 : 0.8}
                />
              </g>
            );
          })}
        </g>
      </g>
      {PALM_GUIDE_LINES.map(line => {
        const isActive = line.id === highlightId;
        const stroke = isActive ? colorOf(line.id) : DIM_LINE;
        return (
          <text
            key={line.id}
            x={mirrorX(line.labelAt[0], handedness)}
            y={line.labelAt[1]}
            textAnchor="middle"
            fontSize={isActive ? 10 : 8.5}
            fontWeight={isActive ? 700 : 400}
            fill={isActive ? stroke : 'rgba(148,163,184,0.8)'}
          >
            {line.label}
          </text>
        );
      })}
    </g>
  );
};
