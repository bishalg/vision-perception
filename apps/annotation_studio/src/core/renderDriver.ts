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

export interface RenderLayerMask {
  image?: boolean;
  ai?: boolean;
  truth?: boolean;
  draft?: boolean;
  /** Faint human/import ghost on Source pane */
  ghostTruth?: boolean;
}

export function renderStudioFrame(
  ctx: CanvasRenderingContext2D,
  state: StudioState,
  imageElement: CanvasImageSource | null,
  viewportWidth: number,
  viewportHeight: number,
  layerMask?: RenderLayerMask
): void {
  ctx.clearRect(0, 0, viewportWidth, viewportHeight);
  if (!imageElement) return;

  const showImage = layerMask?.image ?? state.layerVisibility.image;
  const showAi = layerMask?.ai ?? state.layerVisibility.ai;
  const showTruth = layerMask?.truth ?? state.layerVisibility.truth;
  const showDraft = layerMask?.draft ?? true;
  const showGhost = layerMask?.ghostTruth ?? false;

  const iw = state.imageBounds.width || 640;
  const ih = state.imageBounds.height || 640;

  ctx.save();
  ctx.translate(state.pan.x, state.pan.y);
  ctx.scale(state.zoom, state.zoom);

  // 1. Draw Background Image with clean border and drop-shadow
  if (showImage && state.layerOpacities.image > 0) {
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

  // Optional faint ghost of imported / human truth on Source pane
  if (showGhost && state.currentDomain?.classes) {
    ctx.globalAlpha = 0.28;
    for (const cls of state.currentDomain.classes) {
      const polys = state.polylines[cls.id] || [];
      ctx.strokeStyle = cls.color;
      ctx.lineWidth = 2.0 / state.zoom;
      for (const poly of polys) {
        if (!poly || poly.length < 2) continue;
        ctx.beginPath();
        ctx.moveTo(poly[0][0] * scaleX, poly[0][1] * scaleY);
        for (let i = 1; i < poly.length; ++i) {
          ctx.lineTo(poly[i][0] * scaleX, poly[i][1] * scaleY);
        }
        ctx.stroke();
      }
    }
  }

  // 2. Draw CurvEngine AI Predictions / Steger Ridges
  if (showAi && state.layerOpacities.ai > 0 && state.currentDomain?.classes) {
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
  if (showTruth && state.layerOpacities.truth > 0 && state.currentDomain?.classes) {
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
  if (showDraft && state.currentDraft.length > 0) {
    const draftTargetClass = state.draftClassId || state.activeClassId;
    const activeCls = state.currentDomain?.classes?.find(c => c.id === draftTargetClass) || { color: '#3b82f6' };
    ctx.globalAlpha = 1.0;
    ctx.strokeStyle = activeCls.color;
    ctx.fillStyle = activeCls.color;
    ctx.lineWidth = 3.5 / state.zoom;

    ctx.beginPath();
    ctx.moveTo(state.currentDraft[0][0] * scaleX, state.currentDraft[0][1] * scaleY);
    for (let i = 1; i < state.currentDraft.length; ++i) {
      ctx.lineTo(state.currentDraft[i][0] * scaleX, state.currentDraft[i][1] * scaleY);
    }
    ctx.stroke();

    // Elastic guide line to current cursor in drafting mode
    if (state.currentCursor) {
      const last = state.currentDraft[state.currentDraft.length - 1];
      ctx.beginPath();
      ctx.setLineDash([4 / state.zoom, 4 / state.zoom]);
      ctx.strokeStyle = hexToRgba(activeCls.color, 0.75);
      ctx.lineWidth = 2.0 / state.zoom;
      ctx.moveTo(last[0] * scaleX, last[1] * scaleY);
      ctx.lineTo(state.currentCursor[0] * scaleX, state.currentCursor[1] * scaleY);
      ctx.stroke();

      if (state.activeTool === 'polygon' && state.currentDraft.length >= 2) {
        ctx.beginPath();
        ctx.strokeStyle = hexToRgba(activeCls.color, 0.35);
        ctx.moveTo(state.currentCursor[0] * scaleX, state.currentCursor[1] * scaleY);
        ctx.lineTo(state.currentDraft[0][0] * scaleX, state.currentDraft[0][1] * scaleY);
        ctx.stroke();
      }
      ctx.setLineDash([]);
    }

    // Handles with outline for contrast against any palm/satellite background
    const r = 4.5 / state.zoom;
    for (let i = 0; i < state.currentDraft.length; ++i) {
      const pt = state.currentDraft[i];
      ctx.beginPath();
      ctx.arc(pt[0] * scaleX, pt[1] * scaleY, r, 0, Math.PI * 2);
      ctx.fillStyle = i === 0 ? '#FFFFFF' : activeCls.color;
      ctx.fill();
      ctx.strokeStyle = '#0e0e13';
      ctx.lineWidth = 1.2 / state.zoom;
      ctx.stroke();
    }
  }

  // 5. Draw BBox Drag Preview
  if (showDraft && state.activeTool === 'bbox' && state.bboxStart && state.currentCursor) {
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
