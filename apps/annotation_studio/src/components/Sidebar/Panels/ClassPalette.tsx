// apps/annotation_studio/src/components/Sidebar/Panels/ClassPalette.tsx
import React, { useEffect } from 'react';
import { useStore } from '@tanstack/react-store';
import {
  studioStore,
  selectClass,
  toggleClassVisibility,
  countClassAnnotations,
} from '../../../core/store';
import { getGuideHighlight, PALM_GUIDE_FALLBACK_COLORS, inferHandednessFromFilename } from '../../../core/palmGuide';
import type { AnnotationClass } from '../../../core/types';
import { IconEye, IconEyeOff, IconInfo } from '../../Common/Icons';
import { PalmGuide } from './PalmGuide';

interface ClassPaletteProps {
  classes: AnnotationClass[];
}

export const ClassPalette: React.FC<ClassPaletteProps> = ({ classes }) => {
  const activeClassId = useStore(studioStore, s => s.activeClassId);
  const classVisibility = useStore(studioStore, s => s.classVisibility);
  const absentClasses = useStore(studioStore, s => s.absentClasses);
  const state = useStore(studioStore, s => s);
  const [guideOpen, setGuideOpen] = React.useState(false);
  const [guideHand, setGuideHand] = React.useState<'right' | 'left'>('left');
  const [guideHandAuto, setGuideHandAuto] = React.useState(true);
  const sampleFilename = useStore(studioStore, s => s.currentSample?.filename);

  // Follow the on-screen sample: 01_left_bishal.jpg -> left-artwork and the
  // life C flips with the thumb, so the hint can never disagree with the
  // capture. Manual R|L taps override until the sample changes.
  useEffect(() => {
    const inferred = inferHandednessFromFilename(sampleFilename);
    if (inferred) {
      setGuideHand(inferred);
      setGuideHandAuto(true);
    }
  }, [sampleFilename]);

  // Close the hint preview on Escape.
  useEffect(() => {
    if (!guideOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setGuideOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [guideOpen]);

  const colorFor = (classId: string): string =>
    classes.find(c => c.id === classId)?.color ?? PALM_GUIDE_FALLBACK_COLORS[classId] ?? '#94A3B8';

  const highlight = getGuideHighlight(activeClassId);
  const activeClass = classes.find(c => c.id === activeClassId);

  return (
    <div id="classList" className="class-list">
      {classes.map(c => {
        const isSelected = c.id === activeClassId;
        const isVisible = classVisibility[c.id] !== false;
        const count = countClassAnnotations(state, c.id);
        const isAbsent = absentClasses.includes(c.id);

        return (
          <div
            key={c.id}
            className={`class-item-row ${isSelected ? 'active' : ''}`}
            data-class={c.id}
            style={{ color: c.color }}
            onClick={() => selectClass(c.id)}
          >
            <div className="class-left">
              <span className="class-dot" style={{ background: c.color }} />
              <span style={{ color: 'var(--txt)' }}>{c.label}</span>
              {isAbsent && (
                <span
                  style={{
                    fontSize: '9.5px',
                    color: 'var(--amber)',
                    background: 'rgba(245,158,11,0.15)',
                    padding: '1px 5px',
                    borderRadius: '4px',
                  }}
                >
                  ABSENT
                </span>
              )}
            </div>
            <div className="class-right">
              <span className="class-count-badge">{count}</span>
              <button
                className="class-guide-btn"
                data-guide-class={c.id}
                title={`What is the ${c.label}?`}
                aria-label={`Show ${c.label} hint`}
                aria-expanded={guideOpen}
                onClick={e => {
                  e.stopPropagation();
                  selectClass(c.id);
                  setGuideOpen(v => !v);
                }}
              >
                <IconInfo size={14} />
              </button>
              <button
                className="class-visibility-btn"
                data-vis-class={c.id}
                title="Toggle Visibility"
                onClick={e => {
                  e.stopPropagation();
                  toggleClassVisibility(c.id);
                }}
              >
                {isVisible ? <IconEye size={14} /> : <IconEyeOff size={14} />}
              </button>
            </div>
          </div>
        );
      })}
      {guideOpen && (
        <div className="class-guide-card" role="dialog" aria-label="Crease hint preview">
          <div className="class-guide-header">
            <span className="class-guide-title" style={{ color: activeClass?.color ?? 'var(--txt)' }}>
              {highlight ? highlight.line.label : (activeClass?.label ?? 'Crease guide')}
            </span>
            <span className="class-guide-hand-toggle" role="group" aria-label="Guide hand">
              {(['right', 'left'] as const).map(h => (
                <button
                  key={h}
                  className={`class-guide-hand-btn ${guideHand === h ? 'active' : ''}`}
                  title={guideHandAuto && guideHand === h ? `Following sample (${sampleFilename}) — tap to override` : `Show ${h} hand`}
                  aria-pressed={guideHand === h}
                  onClick={() => {
                    setGuideHand(h);
                    setGuideHandAuto(false);
                  }}
                >
                  {h === 'right' ? 'R' : 'L'}
                </button>
              ))}
            </span>
            <button
              className="class-guide-close"
              title="Close hint (Esc)"
              aria-label="Close hint"
              onClick={() => setGuideOpen(false)}
            >
              ×
            </button>
          </div>
          <PalmGuide highlightId={highlight ? highlight.line.id : null} colorFor={colorFor} handedness={guideHand} />
          <div className="class-guide-desc">
            {highlight ? highlight.line.description : 'Select a crease class to see where it runs.'}
          </div>
        </div>
      )}
    </div>
  );
};
