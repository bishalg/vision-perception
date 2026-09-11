// apps/annotation_studio/src/components/Viewport/BrushPopover.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../core/store';

interface BrushPopoverProps {
  isOpen: boolean;
  onClose?: () => void;
}

export const BrushPopover: React.FC<BrushPopoverProps> = ({ isOpen, onClose }) => {
  const brushSettings = useStore(studioStore, s => s.brushSettings);

  if (!isOpen) return null;

  const setMode = (mode: 'draw' | 'erase') => {
    studioStore.setState(prev => ({
      ...prev,
      brushSettings: {
        ...prev.brushSettings,
        mode,
      },
    }));
  };

  const setSize = (size: number) => {
    studioStore.setState(prev => ({
      ...prev,
      brushSettings: {
        ...prev.brushSettings,
        size,
      },
    }));
  };

  return (
    <div
      className="brush-sub-popover show"
      id="brushSubPopover"
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
    >
      <div className="brush-popover-header">
        <span>🖌️ Brush Tool</span>
        {onClose && (
          <button
            className="modal-close"
            style={{ padding: '0 4px', fontSize: '13px' }}
            onClick={onClose}
            title="Close"
          >
            ✕
          </button>
        )}
      </div>

      <div className="brush-mode-row">
        <span className="brush-label">Mode</span>
        <div className="brush-segmented-toggle">
          <button
            id="btnBrushDraw"
            className={`brush-seg-btn ${brushSettings.mode === 'draw' ? 'active' : ''}`}
            title="Draw stroke"
            onClick={() => setMode('draw')}
          >
            🖌️ Draw
          </button>
          <button
            id="btnBrushErase"
            className={`brush-seg-btn ${brushSettings.mode === 'erase' ? 'active' : ''}`}
            title="Erase stroke"
            onClick={() => setMode('erase')}
          >
            🧽 Erase
          </button>
        </div>
      </div>

      <div className="brush-size-row">
        <span className="brush-label">Size</span>
        <input
          type="range"
          id="rngBrushSize"
          min="4"
          max="64"
          value={brushSettings.size}
          className="brush-slider"
          onChange={e => setSize(parseInt(e.target.value, 10))}
        />
        <span id="lblBrushSize" className="brush-size-val">
          {brushSettings.size}px
        </span>
      </div>
    </div>
  );
};
