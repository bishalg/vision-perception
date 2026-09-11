// apps/annotation_studio/src/components/Sidebar/Panels/ClassPalette.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import {
  studioStore,
  selectClass,
  toggleClassVisibility,
  countClassAnnotations,
} from '../../../core/store';
import type { AnnotationClass } from '../../../core/types';

interface ClassPaletteProps {
  classes: AnnotationClass[];
}

export const ClassPalette: React.FC<ClassPaletteProps> = ({ classes }) => {
  const activeClassId = useStore(studioStore, s => s.activeClassId);
  const classVisibility = useStore(studioStore, s => s.classVisibility);
  const absentClasses = useStore(studioStore, s => s.absentClasses);
  const state = useStore(studioStore, s => s);

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
                className="class-visibility-btn"
                data-vis-class={c.id}
                title="Toggle Visibility"
                onClick={e => {
                  e.stopPropagation();
                  toggleClassVisibility(c.id);
                }}
              >
                {isVisible ? '👁️' : '🕶️'}
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
};
