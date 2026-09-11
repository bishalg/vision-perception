// apps/annotation_studio/src/components/Modals/ShortcutsModal.tsx
import React from 'react';

interface ShortcutsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const ShortcutsModal: React.FC<ShortcutsModalProps> = ({ isOpen, onClose }) => {
  if (!isOpen) return null;

  return (
    <div
      className="modal-backdrop show"
      id="shortcutsModal"
      onClick={onClose}
    >
      <div
        className="modal-window"
        style={{ maxWidth: '540px' }}
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <span className="modal-title">Keyboard Shortcuts</span>
          <button className="modal-close" id="btnCloseShortcuts" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="modal-body" style={{ gap: '12px' }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px', fontSize: '12px' }}>
            <div style={{ background: 'var(--surface-2)', padding: '10px', borderRadius: '6px', border: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <b style={{ color: 'var(--txt)' }}>Navigation</b>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Pan / Hand</span><code>H / Space</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Prev Image</span><code>[</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Next Image</span><code>]</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Reset View</span><code>0</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Zoom In/Out</span><code>Scroll / +/-</code></div>
            </div>
            <div style={{ background: 'var(--surface-2)', padding: '10px', borderRadius: '6px', border: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <b style={{ color: 'var(--txt)' }}>Annotation Tools</b>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Polyline Tool</span><code>L</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Polygon Tool</span><code>P</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Brush Tool</span><code>B</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>BBox Tool</span><code>R</code></div>
            </div>
            <div style={{ background: 'var(--surface-2)', padding: '10px', borderRadius: '6px', border: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <b style={{ color: 'var(--txt)' }}>AI & Actions</b>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>AI Contour Snap</span><code>S</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Mark Absent</span><code>A</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Save Truth</span><code>Cmd+S / Ctrl+S</code></div>
            </div>
            <div style={{ background: 'var(--surface-2)', padding: '10px', borderRadius: '6px', border: '1px solid var(--border)', display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <b style={{ color: 'var(--txt)' }}>History & Classes</b>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Undo</span><code>Cmd+Z / Ctrl+Z</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Redo</span><code>Cmd+Y / Ctrl+Y</code></div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}><span style={{ color: 'var(--txt-muted)' }}>Select Class</span><code>1, 2, 3, 4</code></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
