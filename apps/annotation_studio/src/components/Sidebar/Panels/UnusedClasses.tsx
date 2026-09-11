// apps/annotation_studio/src/components/Sidebar/Panels/UnusedClasses.tsx
import React from 'react';
import { selectClass } from '../../../core/store';
import type { AnnotationClass } from '../../../core/types';

interface UnusedClassesProps {
  classes: AnnotationClass[];
}

export const UnusedClasses: React.FC<UnusedClassesProps> = ({ classes }) => {
  if (classes.length === 0) {
    return (
      <div style={{ fontSize: '11px', color: 'var(--txt-muted)', padding: '4px' }}>
        All classes in use
      </div>
    );
  }

  return (
    <div id="unusedClassList" className="unused-class-list">
      {classes.map(c => (
        <div
          key={c.id}
          className="unused-class-item"
          data-class={c.id}
          title={`Click to activate ${c.label}`}
          onClick={() => selectClass(c.id)}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span className="class-dot" style={{ background: c.color, opacity: 0.65 }} />
            <span>{c.label}</span>
          </div>
          <span style={{ fontSize: '12px', color: 'var(--txt-muted)' }}>+</span>
        </div>
      ))}
    </div>
  );
};
