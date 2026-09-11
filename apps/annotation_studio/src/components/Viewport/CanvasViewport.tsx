// apps/annotation_studio/src/components/Viewport/CanvasViewport.tsx
// High-Precision 60 FPS Canvas Viewport with direct Context2D rendering and headless store integration.
import React, { useRef, useEffect, useState, useCallback } from 'react';
import { useStore } from '@tanstack/react-store';
import {
  studioStore,
  countTotalAnnotations,
  resetView,
  zoomStep,
} from '../../core/store';
import { screenToImageCoords } from '../../core/geometry';
import {
  handlePointerDown,
  handlePointerMove,
  handlePointerUp,
  finishDraft,
} from '../../core/tools';
import { renderStudioFrame } from '../../core/renderDriver';
import { ToolDock } from './ToolDock';
import { FloatingBottomBar } from './FloatingBottomBar';
import { BrushPopover } from './BrushPopover';

interface CanvasViewportProps {
  onOpenShortcuts: () => void;
}

export const CanvasViewport: React.FC<CanvasViewportProps> = ({ onOpenShortcuts }) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);

  const [viewportSize, setViewportSize] = useState({ width: 800, height: 600 });
  const [showBrushPopover, setShowBrushPopover] = useState(false);
  const [hudCoords, setHudCoords] = useState<[number, number]>([0, 0]);

  const activeTool = useStore(studioStore, s => s.activeTool);
  const currentSample = useStore(studioStore, s => s.currentSample);
  const zoom = useStore(studioStore, s => s.zoom);
  const pan = useStore(studioStore, s => s.pan);
  const contrastMode = useStore(studioStore, s => s.contrastMode);
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const totalAnnotations = useStore(studioStore, countTotalAnnotations);

  const isPanningRef = useRef(false);
  const isMouseDownRef = useRef(false);
  const lastMousePosRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });

  // 1. Synchronize canvas resolution with container dimensions
  const updateCanvasDimensions = useCallback(() => {
    if (!containerRef.current || !canvasRef.current) return;
    const w = containerRef.current.clientWidth;
    const h = containerRef.current.clientHeight;
    if (w > 0 && h > 0 && (canvasRef.current.width !== w || canvasRef.current.height !== h)) {
      canvasRef.current.width = w;
      canvasRef.current.height = h;
      setViewportSize({ width: w, height: h });
    }
  }, []);

  useEffect(() => {
    updateCanvasDimensions();
    const ro = new ResizeObserver(() => updateCanvasDimensions());
    if (containerRef.current) ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, [updateCanvasDimensions]);

  // 2. Load Sample Image when currentSample changes
  useEffect(() => {
    if (!currentSample?.image_url) {
      imageRef.current = null;
      return;
    }

    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      imageRef.current = img;
      studioStore.setState(prev => ({
        ...prev,
        imageBounds: {
          width: img.naturalWidth || 640,
          height: img.naturalHeight || 640,
        },
      }));

      // Auto-fit to viewport on image load
      if (containerRef.current) {
        resetView(containerRef.current.clientWidth, containerRef.current.clientHeight);
      }
    };
    img.src = currentSample.image_url;
  }, [currentSample?.image_url]);

  // 3. Render Loop / Frame Redraw
  const redraw = useCallback(() => {
    if (!canvasRef.current) return;
    const ctx = canvasRef.current.getContext('2d');
    if (!ctx) return;
    renderStudioFrame(
      ctx,
      studioStore.state,
      imageRef.current,
      canvasRef.current.width,
      canvasRef.current.height
    );
  }, []);

  // Request redraw whenever reactive store state changes
  useEffect(() => {
    const unsub = studioStore.subscribe(() => {
      requestAnimationFrame(redraw);
    });
    redraw();
    return () => unsub();
  }, [redraw]);

  // 4. Non-Passive Wheel Event (Zoom in/out around cursor)
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const cx = e.clientX - rect.left;
      const cy = e.clientY - rect.top;
      const factor = e.deltaY < 0 ? 1.05 : 0.95;
      zoomStep(factor, cx, cy);
    };

    canvas.addEventListener('wheel', handleWheel, { passive: false });
    return () => canvas.removeEventListener('wheel', handleWheel);
  }, []);

  // 5. Pointer Event Handlers
  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (e.button === 1 || e.altKey || activeTool === 'pan') {
      isPanningRef.current = true;
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
      return;
    }

    if (e.button !== 0 || !canvasRef.current) return;
    isMouseDownRef.current = true;
    const rect = canvasRef.current.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    const imgPt = screenToImageCoords(
      sx,
      sy,
      pan,
      zoom,
      studioStore.state.imageBounds.width,
      studioStore.state.imageBounds.height,
      currentDomain?.coord_space || 'pixels'
    );

    handlePointerDown(imgPt);
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (isPanningRef.current) {
      const dx = e.clientX - lastMousePosRef.current.x;
      const dy = e.clientY - lastMousePosRef.current.y;
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
      studioStore.setState(prev => ({
        ...prev,
        pan: { x: prev.pan.x + dx, y: prev.pan.y + dy },
      }));
      return;
    }

    if (!canvasRef.current) return;
    const rect = canvasRef.current.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    const imgPt = screenToImageCoords(
      sx,
      sy,
      pan,
      zoom,
      studioStore.state.imageBounds.width,
      studioStore.state.imageBounds.height,
      currentDomain?.coord_space || 'pixels'
    );

    setHudCoords([Math.round(imgPt[0]), Math.round(imgPt[1])]);
    handlePointerMove(imgPt, isMouseDownRef.current);
  };

  const handleMouseUp = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (isPanningRef.current) {
      isPanningRef.current = false;
      return;
    }

    if (!canvasRef.current) return;
    isMouseDownRef.current = false;
    const rect = canvasRef.current.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    const imgPt = screenToImageCoords(
      sx,
      sy,
      pan,
      zoom,
      studioStore.state.imageBounds.width,
      studioStore.state.imageBounds.height,
      currentDomain?.coord_space || 'pixels'
    );

    handlePointerUp(imgPt);
  };

  const handleDoubleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    if (activeTool === 'polyline' || activeTool === 'polygon') {
      finishDraft();
    }
  };

  return (
    <div
      className="canvas-container"
      id="canvasContainer"
      ref={containerRef}
      style={{ position: 'relative', flex: 1, overflow: 'hidden' }}
    >
      <canvas
        id="studioCanvas"
        ref={canvasRef}
        style={{
          display: 'block',
          width: '100%',
          height: '100%',
          filter: contrastMode ? 'contrast(1.35) brightness(1.2)' : 'none',
          cursor: activeTool === 'pan' || isPanningRef.current ? 'grab' : 'crosshair',
        }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onDoubleClick={handleDoubleClick}
      />

      {/* Top HUD */}
      <div className="canvas-hud">
        <span id="hudCoords">
          X: {hudCoords[0]}, Y: {hudCoords[1]}
        </span>
        <span id="hudZoom">Zoom: {Math.round(zoom * 100)}%</span>
        <span id="hudPoints">Annotations: {totalAnnotations}</span>
      </div>

      {/* Floating Bottom Bar (Zoom / Theme / Shortcuts) */}
      <FloatingBottomBar
        onOpenShortcuts={onOpenShortcuts}
        viewportWidth={viewportSize.width}
        viewportHeight={viewportSize.height}
      />

      {/* Floating Right Tool Dock */}
      <ToolDock
        onToggleBrushPopover={() => setShowBrushPopover(p => !p)}
        showBrushPopover={showBrushPopover}
      />

      {/* Brush Sub-Popover */}
      <BrushPopover
        isOpen={showBrushPopover && activeTool === 'brush'}
        onClose={() => setShowBrushPopover(false)}
      />
    </div>
  );
};
