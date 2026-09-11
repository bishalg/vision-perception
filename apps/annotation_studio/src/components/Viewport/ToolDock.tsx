// apps/annotation_studio/src/components/Viewport/ToolDock.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import {
  studioStore,
  setTool,
  performUndo,
  performRedo,
  toggleAbsentClass,
} from '../../core/store';
import type { ToolType } from '../../core/types';
import {
  IconHand,
  IconBBox,
  IconPolygon,
  IconBrush,
  IconWand,
  IconPolyline,
  IconUndo,
  IconRedo,
  IconAbsent,
} from '../Common/Icons';

interface ToolDockProps {
  onToggleBrushPopover: () => void;
  showBrushPopover: boolean;
}

export const ToolDock: React.FC<ToolDockProps> = ({ onToggleBrushPopover }) => {
  const activeTool = useStore(studioStore, s => s.activeTool);
  const activeClassId = useStore(studioStore, s => s.activeClassId);

  const tools: Array<{
    id: ToolType;
    icon: React.ReactNode;
    title: string;
    badge?: string;
  }> = [
    { id: 'pan', icon: <IconHand size={18} />, title: 'Select / Pan (H / Space)' },
    { id: 'bbox', icon: <IconBBox size={18} />, title: 'Bounding Box (R)' },
    { id: 'polygon', icon: <IconPolygon size={18} />, title: 'Polygon Area Tool (P)' },
    { id: 'brush', icon: <IconBrush size={18} />, title: 'Brush Stroke Tool (B)' },
    { id: 'wand', icon: <IconWand size={18} />, title: 'AI Contour Snap (S)', badge: 'AI' },
    { id: 'polyline', icon: <IconPolyline size={18} />, title: 'Polyline / Crease Tool (L)' },
  ];

  const handleToolClick = (e: React.MouseEvent, tool: ToolType) => {
    e.stopPropagation();
    e.preventDefault();
    setTool(tool);
    if (tool === 'brush') {
      onToggleBrushPopover();
    }
  };

  const handleUndo = (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    performUndo();
  };

  const handleRedo = (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    performRedo();
  };

  const handleAbsent = (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    if (activeClassId) toggleAbsentClass(activeClassId);
  };

  return (
    <div className="tool-dock" onClick={e => e.stopPropagation()} onMouseDown={e => e.stopPropagation()}>
      {tools.map(t => {
        const isActive = activeTool === t.id;
        return (
          <button
            key={t.id}
            className={`dock-btn ${isActive ? 'active' : ''}`}
            data-tool={t.id}
            title={t.title}
            onClick={e => handleToolClick(e, t.id)}
          >
            {t.icon}
            {t.badge && <span className="dock-ai-badge">{t.badge}</span>}
          </button>
        );
      })}

      <div className="dock-divider" />

      <button
        className="dock-btn"
        id="toolUndo"
        title="Undo (Ctrl+Z / Cmd+Z)"
        onClick={handleUndo}
      >
        <IconUndo size={17} />
      </button>
      <button
        className="dock-btn"
        id="toolRedo"
        title="Redo (Ctrl+Y / Cmd+Y)"
        onClick={handleRedo}
      >
        <IconRedo size={17} />
      </button>
      <button
        className="dock-btn"
        id="toolAbsent"
        title="Mark Feature Absent (A)"
        onClick={handleAbsent}
      >
        <IconAbsent size={17} />
      </button>
    </div>
  );
};
