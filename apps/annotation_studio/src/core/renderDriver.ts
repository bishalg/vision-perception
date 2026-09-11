// apps/annotation_studio/src/core/renderDriver.ts — Abstract Render Driver for HTML5 Canvas & Skia
// Framework-agnostic render pipeline with zero direct component bindings.

import type { StudioState } from './store';
import type { Point2D } from './types';

export function hexToRgba(hex: string, alpha: number): string {
  let c = hex.replace('#', '');
  if (c.length === 3) c = c.split('').map(x => x + x).join('');
  const num = parseInt(c, 16);
  return `rgba(${(num >> 16) & 255}, ${(num >> 8) & 255}, ${num & 255}, ${alpha})`;
}

export function renderStudioFrame(
  ctx: CanvasRenderingContext2D,
  state: StudioState,
  imageElement: CanvasImageSource | null,
  viewportWidth: number,
  viewportHeight: number
): void {
  ctx.clearRect(0, 0, viewportWidth, viewportHeight);
  if (!imageElement) return;

  const iw = state.imageBounds.width || 640;
  const ih = state.imageBounds.height || 640;

  ctx.save();
  ctx.translate(state.pan.x, state.pan.y);
  ctx.scale(state.zoom, state.zoom);

  // 1. Draw Background Image with clean border and drop-shadow
  if (state.layerVisibility.image && state.layerOpacities.image > 0) {
    ctx.save();
    ctx.shadowColor = 'rgba(0, 0, 0, 0.7)';
    ctx.shadowBlur = 20 / state.zoom;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = 6 / state.zoom;
    ctx.fillStyle = '#0e0e13';
    ctx.fillRect(0, 0, iw, ih);
    ctx.restore();

    ctx.globalAlpha = state.layerOpacities.image;
    ctx.drawImage(imageElement, 0, 0, iw, ih);

    // Modern clean hairline outline
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.15)';
    ctx.lineWidth = 1.0 / state.zoom;
    ctx.strokeRect(0, 0, iw, ih);
  }

  const isNorm = state.currentDomain?.coord_space === 'norm_512';
  const scaleX = isNorm ? iw / 512.0 : 1.0;
  const scaleY = isNorm ? ih / 512.0 : 1.0;

  // 2. Draw CurvEngine AI Predictions / Steger Ridges
  if (state.layerVisibility.ai && state.layerOpacities.ai > 0 && state.currentDomain?.classes) {
    ctx.globalAlpha = state.layerOpacities.ai;
    for (const cls of state.currentDomain.classes) {
      if (state.classVisibility[cls.id] === false) continue;
      const pts = state.aiLines[cls.id];
      if (!pts || pts.length < 2) continue;

      ctx.strokeStyle = cls.color;
      ctx.lineWidth = 2.5 / state.zoom;
      ctx.setLineDash([4 / state.zoom, 4 / state.zoom]);
      ctx.beginPath();
      ctx.moveTo(pts[0][0] * scaleX, pts[0][1] * scaleY);
      for (let i = 1; i < pts.length; ++i) {
        ctx.lineTo(pts[i][0] * scaleX, pts[i][1] * scaleY);
      }
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }

  // 3. Draw Ground Truth Annotations
  if (state.layerVisibility.truth && state.layerOpacities.truth > 0 && state.currentDomain?.classes) {
    ctx.globalAlpha = state.layerOpacities.truth;
    for (const cls of state.currentDomain.classes) {
      if (state.classVisibility[cls.id] === false) continue;

      // Draw Polylines
      const polys = state.polylines[cls.id] || [];
      ctx.strokeStyle = cls.color;
      ctx.fillStyle = cls.color;
      ctx.lineWidth = 3.5 / state.zoom;

      for (const poly of polys) {
        if (!poly || poly.length === 0) continue;
        ctx.beginPath();
        ctx.moveTo(poly[0][0] * scaleX, poly[0][1] * scaleY);
        for (let i = 1; i < poly.length; ++i) {
          ctx.lineTo(poly[i][0] * scaleX, poly[i][1] * scaleY);
        }
        ctx.stroke();

        // Vertex handles
        const r = 4 / state.zoom;
        for (const pt of poly) {
          ctx.beginPath();
          ctx.arc(pt[0] * scaleX, pt[1] * scaleY, r, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Draw Polygons
      const pgs = state.polygons[cls.id] || [];
      for (const pg of pgs) {
        if (!pg || pg.length < 3) continue;
        ctx.beginPath();
        ctx.moveTo(pg[0][0] * scaleX, pg[0][1] * scaleY);
        for (let i = 1; i < pg.length; ++i) {
          ctx.lineTo(pg[i][0] * scaleX, pg[i][1] * scaleY);
        }
        ctx.closePath();
        ctx.fillStyle = hexToRgba(cls.color, 0.2);
        ctx.fill();
        ctx.stroke();

        const r = 4 / state.zoom;
        ctx.fillStyle = cls.color;
        for (const pt of pg) {
          ctx.beginPath();
          ctx.arc(pt[0] * scaleX, pt[1] * scaleY, r, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Draw Bounding Boxes
      const bboxes = state.bboxes[cls.id] || [];
      for (const bb of bboxes) {
        if (!bb || bb.length < 4) continue;
        const [x1, y1, x2, y2] = bb;
        const bx = Math.min(x1, x2) * scaleX;
        const by = Math.min(y1, y2) * scaleY;
        const bw = Math.abs(x2 - x1) * scaleX;
        const bh = Math.abs(y2 - y1) * scaleY;
        ctx.fillStyle = hexToRgba(cls.color, 0.2);
        ctx.fillRect(bx, by, bw, bh);
        ctx.strokeRect(bx, by, bw, bh);
      }
    }

    // Draw Brush Strokes
    for (const stroke of state.brushStrokes) {
      if (state.classVisibility[stroke.classId] === false) continue;
      const cls = state.currentDomain?.classes?.find(c => c.id === stroke.classId) || { color: '#3b82f6' };
      ctx.strokeStyle = stroke.mode === 'erase' ? 'rgba(0,0,0,0.85)' : hexToRgba(cls.color, 0.6);
      ctx.lineWidth = stroke.size / state.zoom;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      if (stroke.points.length > 1) {
        ctx.beginPath();
        ctx.moveTo(stroke.points[0][0] * scaleX, stroke.points[0][1] * scaleY);
        for (let i = 1; i < stroke.points.length; ++i) {
          ctx.lineTo(stroke.points[i][0] * scaleX, stroke.points[i][1] * scaleY);
        }
        ctx.stroke();
      }
    }
  }

  // 4. Draw Current Draft Shape
  if (state.currentDraft.length > 0) {
    const activeCls = state.currentDomain?.classes?.find(c => c.id === state.activeClassId) || { color: '#3b82f6' };
    ctx.globalAlpha = 1.0;
    ctx.strokeStyle = activeCls.color;
    ctx.fillStyle = activeCls.color;
    ctx.lineWidth = 3.5 / state.zoom;

    ctx.beginPath();
    ctx.moveTo(state.currentDraft[0][0] * scaleX, state.currentDraft[0][1] * scaleY);
    for (let i = 1; i < state.currentDraft.length; ++i) {
      ctx.lineTo(state.currentDraft[i][0] * scaleX, state.currentDraft[i][1] * scaleY);
    }

    if (state.activeTool === 'polygon' && state.currentDraft.length >= 2) {
      ctx.stroke();
      // Closing dashed line
      ctx.beginPath();
      ctx.setLineDash([4 / state.zoom, 4 / state.zoom]);
      ctx.moveTo(
        state.currentDraft[state.currentDraft.length - 1][0] * scaleX,
        state.currentDraft[state.currentDraft.length - 1][1] * scaleY
      );
      ctx.lineTo(state.currentDraft[0][0] * scaleX, state.currentDraft[0][1] * scaleY);
      ctx.stroke();
      ctx.setLineDash([]);
    } else {
      ctx.stroke();
    }

    // Handles
    const r = 4.5 / state.zoom;
    for (const pt of state.currentDraft) {
      ctx.beginPath();
      ctx.arc(pt[0] * scaleX, pt[1] * scaleY, r, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // 5. Draw BBox Drag Preview
  if (state.activeTool === 'bbox' && state.bboxStart && state.currentCursor) {
    const activeCls = state.currentDomain?.classes?.find(c => c.id === state.activeClassId) || { color: '#3b82f6' };
    const [x1, y1] = state.bboxStart;
    const [x2, y2] = state.currentCursor;
    const bx = Math.min(x1, x2) * scaleX;
    const by = Math.min(y1, y2) * scaleY;
    const bw = Math.abs(x2 - x1) * scaleX;
    const bh = Math.abs(y2 - y1) * scaleY;
    ctx.strokeStyle = activeCls.color;
    ctx.fillStyle = hexToRgba(activeCls.color, 0.25);
    ctx.lineWidth = 2.5 / state.zoom;
    ctx.fillRect(bx, by, bw, bh);
    ctx.strokeRect(bx, by, bw, bh);
  }

  ctx.restore();
}
