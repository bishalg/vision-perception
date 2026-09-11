// apps/annotation_studio/src/core/tools.ts — Headless Tool State Machine
// Framework-agnostic interaction engine for curve, polygon, brush, bbox, and AI snapping.

import { studioStore, recordSnapshot, addAuditLog } from './store';
import { findClosestRidge, dist } from './geometry';
import type { Point2D } from './types';

export function handlePointerDown(imgPt: Point2D): void {
  const state = studioStore.state;
  const tool = state.activeTool;
  const activeClass = state.activeClassId;
  if (!activeClass) return;

  if (tool === 'polyline' || tool === 'polygon') {
    // Add point to active drafting curve
    studioStore.setState(prev => ({
      ...prev,
      currentDraft: [...prev.currentDraft, imgPt],
    }));
  } else if (tool === 'bbox') {
    studioStore.setState(prev => ({
      ...prev,
      bboxStart: imgPt,
      currentCursor: imgPt,
    }));
  } else if (tool === 'brush') {
    recordSnapshot();
    const newStroke = {
      id: Math.random().toString(36).substring(2, 9),
      classId: activeClass,
      points: [imgPt],
      size: state.brushSettings.size,
      mode: state.brushSettings.mode,
    };
    studioStore.setState(prev => ({
      ...prev,
      brushStrokes: [...prev.brushStrokes, newStroke],
    }));
  } else if (tool === 'wand') {
    snapAIRidge(imgPt);
  }
}

export function handlePointerMove(imgPt: Point2D, isDragging: boolean): void {
  const state = studioStore.state;
  const tool = state.activeTool;

  studioStore.setState(prev => ({
    ...prev,
    currentCursor: imgPt,
  }));

  if (tool === 'bbox' && state.bboxStart) {
    // bbox preview updates automatically via currentCursor
  } else if (tool === 'brush' && isDragging && state.brushStrokes.length > 0) {
    studioStore.setState(prev => {
      const strokes = [...prev.brushStrokes];
      const last = strokes[strokes.length - 1];
      if (!last) return prev;
      strokes[strokes.length - 1] = {
        ...last,
        points: [...last.points, imgPt],
      };
      return {
        ...prev,
        brushStrokes: strokes,
      };
    });
  }
}

export function handlePointerUp(imgPt: Point2D): void {
  const state = studioStore.state;
  const tool = state.activeTool;
  const activeClass = state.activeClassId;

  if (tool === 'bbox' && state.bboxStart && activeClass) {
    const start = state.bboxStart;
    const w = Math.abs(imgPt[0] - start[0]);
    const h = Math.abs(imgPt[1] - start[1]);

    if (w > 4 && h > 4) {
      recordSnapshot();
      const x1 = Math.min(start[0], imgPt[0]);
      const y1 = Math.min(start[1], imgPt[1]);
      const x2 = Math.max(start[0], imgPt[0]);
      const y2 = Math.max(start[1], imgPt[1]);
      const newBox: [number, number, number, number] = [x1, y1, x2, y2];

      studioStore.setState(prev => ({
        ...prev,
        bboxes: {
          ...prev.bboxes,
          [activeClass]: [...(prev.bboxes[activeClass] || []), newBox],
        },
        bboxStart: null,
      }));
      addAuditLog('BBox Added', `Added bounding box for ${activeClass}`);
    } else {
      studioStore.setState(prev => ({ ...prev, bboxStart: null }));
    }
  }
}

export function finishDraft(): void {
  const state = studioStore.state;
  const activeClass = state.activeClassId;
  const pts = state.currentDraft;
  if (!activeClass || pts.length < 2) {
    studioStore.setState(prev => ({ ...prev, currentDraft: [] }));
    return;
  }

  recordSnapshot();
  if (state.activeTool === 'polygon' && pts.length >= 3) {
    studioStore.setState(prev => ({
      ...prev,
      polygons: {
        ...prev.polygons,
        [activeClass]: [...(prev.polygons[activeClass] || []), pts],
      },
      currentDraft: [],
    }));
    addAuditLog('Polygon Committed', `Finished polygon with ${pts.length} vertices for ${activeClass}`);
  } else {
    studioStore.setState(prev => ({
      ...prev,
      polylines: {
        ...prev.polylines,
        [activeClass]: [...(prev.polylines[activeClass] || []), pts],
      },
      currentDraft: [],
    }));
    addAuditLog('Polyline Committed', `Finished polyline with ${pts.length} vertices for ${activeClass}`);
  }
}

export function snapAIRidge(imgPt: Point2D): boolean {
  const state = studioStore.state;
  const activeClass = state.activeClassId;
  if (!activeClass) return false;

  const match = findClosestRidge(imgPt, state.aiLines, 45.0);
  if (match) {
    recordSnapshot();
    studioStore.setState(prev => ({
      ...prev,
      polylines: {
        ...prev.polylines,
        [activeClass]: [...(prev.polylines[activeClass] || []), match.points],
      },
    }));
    addAuditLog('AI Snap', `Adopted CurvEngine predicted ridge for ${activeClass}`);
    return true;
  }
  return false;
}
