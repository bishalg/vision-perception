// apps/annotation_studio/src/components/Viewport/FloatingBottomBar.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore, zoomStep, resetView } from '../../core/store';

interface FloatingBottomBarProps {
  onOpenShortcuts: () => void;
  viewportWidth: number;
  viewportHeight: number;
}

export const FloatingBottomBar: React.FC<FloatingBottomBarProps> = ({
  onOpenShortcuts,
  viewportWidth,
  viewportHeight,
}) => {
  const zoom = useStore(studioStore, s => s.zoom);
  const contrastMode = useStore(studioStore, s => s.contrastMode);

  const handleZoomOut = (e: React.MouseEvent) => {
    e.stopPropagation();
    zoomStep(0.95, viewportWidth / 2, viewportHeight / 2);
  };

  const handleZoomIn = (e: React.MouseEvent) => {
    e.stopPropagation();
    zoomStep(1.05, viewportWidth / 2, viewportHeight / 2);
  };

  const handleReset = (e: React.MouseEvent) => {
    e.stopPropagation();
    resetView(viewportWidth, viewportHeight);
  };

  const toggleContrast = (e: React.MouseEvent) => {
    e.stopPropagation();
    studioStore.setState(prev => ({
      ...prev,
      contrastMode: !prev.contrastMode,
    }));
  };

  return (
    <div
      className="floating-bottom-bar"
      id="floatingBottomBar"
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
    >
      <div className="zoom-pill">
        <button
          className="zoom-btn"
          id="btnZoomOut"
          title="Zoom Out (-5%)"
          onClick={handleZoomOut}
        >
          —
        </button>
        <span className="zoom-text" id="lblZoomPct">
          {Math.round(zoom * 100)}%
        </span>
        <button
          className="zoom-btn"
          id="btnZoomIn"
          title="Zoom In (+5%)"
          onClick={handleZoomIn}
        >
          +
        </button>
        <button
          className="zoom-reset-btn"
          id="btnResetView"
          title="Reset view and fit to screen (0)"
          onClick={handleReset}
        >
          RESET
        </button>
      </div>

      <button
        className="bottom-circle-btn"
        id="btnToggleTheme"
        title="Toggle Brightness / High-Contrast"
        onClick={toggleContrast}
      >
        {contrastMode ? '🌙' : '☀️'}
      </button>

      <button
        className="bottom-circle-btn"
        id="btnOpenShortcuts"
        title="Keyboard Shortcuts (?)"
        onClick={e => {
          e.stopPropagation();
          onOpenShortcuts();
        }}
      >
        ⌨️
      </button>
    </div>
  );
};
