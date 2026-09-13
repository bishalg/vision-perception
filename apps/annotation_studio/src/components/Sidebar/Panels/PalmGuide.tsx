// apps/annotation_studio/src/components/Sidebar/Panels/PalmGuide.tsx
// Composer: assembles the modular hand segments (palm body, fingers, thumb,
// creases) into the palm-crease hint preview.
//
// - The selected class's crease renders in its own class color; the rest dim.
// - Creases are clipped to the hand silhouette, so they can never paint
//   outside the hand or drift into the finger zone.
// - handedness ('right' default | 'left') mirrors the artwork for left hands:
//   the life-line C flips sides with the thumb, labels stay readable.
// - Reusable outside the sidebar (e.g. camera overlay views): control labels
//   via showAnatomyLabels, size via className/style. The clip id is instance
//   scoped, so multiple guides can mount on one page.
import React, { useId } from 'react';
import { MOUNT_LABELS } from './guide/handAnatomy';
import { mirrorX, type Handedness } from './guide/mirror';
import { PalmBody } from './guide/PalmBody';
import { Fingers } from './guide/Fingers';
import { Thumb } from './guide/Thumb';
import { Creases } from './guide/Creases';

interface PalmGuideProps {
  /** Currently selected annotation class id (null = neutral diagram). */
  highlightId: string | null;
  /** Resolves an annotation class id to its display color. */
  colorFor: (classId: string) => string;
  /** Show finger / mount anatomy labels. Defaults to true. */
  showAnatomyLabels?: boolean;
  /**
   * Which hand the capture shows. Defaults to 'left' (thumb on the left of
   * the screen) to match canonical captures (see canonical_palm.png) and
   * mirrored front-camera selfie previews.
   */
  handedness?: Handedness;
  className?: string;
  style?: React.CSSProperties;
}

const OUTLINE = 'rgba(226,232,240,0.9)';

export const PalmGuide: React.FC<PalmGuideProps> = ({
  highlightId,
  colorFor,
  showAnatomyLabels = true,
  handedness = 'left',
  className,
  style,
}) => {
  const clipId = `palm-crease-clip-${useId().replace(/:/g, '')}`;

  return (
    <svg
      viewBox="0 0 220 250"
      role="img"
      aria-label="Palm crease reference diagram"
      className={className}
      style={{ width: '100%', height: 'auto', display: 'block', ...style }}
    >
      <PalmBody outlineStroke={OUTLINE} handedness={handedness} />
      <Fingers showLabels={showAnatomyLabels} handedness={handedness} />
      <Thumb outlineStroke={OUTLINE} handedness={handedness} />
      {showAnatomyLabels && (
        <g fontSize={6.5} fill="rgba(148,163,184,0.85)">
          {MOUNT_LABELS.map(m => (
            <text key={m.label} x={mirrorX(m.x, handedness)} y={m.y} textAnchor="middle">
              {m.label}
            </text>
          ))}
        </g>
      )}
      <Creases clipId={clipId} highlightId={highlightId} colorFor={colorFor} handedness={handedness} />
    </svg>
  );
};
