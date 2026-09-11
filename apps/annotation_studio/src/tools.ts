// apps/annotation_studio/src/tools.ts — Roboflow-Grade Tool Dock & In-Context Popover (TypeScript)
import { state } from './state';
import { screenToImage, redraw, updateHud } from './canvas';
import type { ToolType, Point2D, BoundingBox } from './types';

interface PendingShape {
  type: 'polyline' | 'polygon' | 'bbox';
  points: any;
}

let hoverCard: HTMLElement | null = null;
let inContextPopover: HTMLElement | null = null;
let brushSubPopover: HTMLElement | null = null;
let pendingShape: PendingShape | null = null;

export function initTools(): void {
  hoverCard = document.getElementById('hoverPreviewCard');
  inContextPopover = document.getElementById('inContextPopover');
  brushSubPopover = document.getElementById('brushSubPopover');

  setupToolDock();
  setupHoverPreviews();
  setupInContextPopover();
  setupCanvasInteractions();
}

export function setTool(toolName: ToolType): void {
  state.activeTool = toolName;
  document.querySelectorAll<HTMLElement>('.dock-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.tool === toolName);
  });

  const container = document.getElementById('canvasContainer');
  if (container) {
    container.classList.toggle('panning', toolName === 'pan');
  }

  // Brush sub-popover
  if (brushSubPopover) {
    brushSubPopover.classList.toggle('show', toolName === 'brush');
  }

  // Cancel any uncommitted draft
  state.currentDraft = [];
  state.bboxStart = null;
  redraw();
}

function setupToolDock(): void {
  document.querySelectorAll<HTMLElement>('.dock-btn[data-tool]').forEach(btn => {
    btn.onclick = () => setTool(btn.dataset.tool as ToolType);
  });

  const btnUndo = document.getElementById('toolUndo');
  if (btnUndo) btnUndo.onclick = () => { state.undo(); redraw(); };

  const btnRedo = document.getElementById('toolRedo');
  if (btnRedo) btnRedo.onclick = () => { state.redo(); redraw(); };

  const btnAbsent = document.getElementById('toolAbsent');
  if (btnAbsent) btnAbsent.onclick = () => toggleAbsentActiveFeature();

  // Brush controls
  const rngBrush = document.getElementById('rngBrushSize') as HTMLInputElement | null;
  const lblBrush = document.getElementById('lblBrushSize');
  if (rngBrush) {
    rngBrush.oninput = e => {
      state.brushSize = parseInt((e.target as HTMLInputElement).value, 10);
      if (lblBrush) lblBrush.textContent = `${state.brushSize}px`;
    };
  }

  const btnBrushDraw = document.getElementById('btnBrushDraw');
  const btnBrushErase = document.getElementById('btnBrushErase');
  if (btnBrushDraw && btnBrushErase) {
    btnBrushDraw.onclick = () => {
      state.brushMode = 'draw';
      btnBrushDraw.classList.add('btn-primary');
      btnBrushErase.classList.remove('btn-primary');
    };
    btnBrushErase.onclick = () => {
      state.brushMode = 'erase';
      btnBrushErase.classList.add('btn-primary');
      btnBrushDraw.classList.remove('btn-primary');
    };
  }
}

function toggleAbsentActiveFeature(): void {
  const cls = state.getActiveClass();
  if (!cls) return;
  state.pushUndo();

  const idx = state.absentClasses.indexOf(cls.id);
  if (idx >= 0) {
    state.absentClasses.splice(idx, 1);
    showToast(`Unmarked absence for ${cls.label}`);
    state.recordAudit('Mark Present', `Restored ${cls.label}`);
  } else {
    state.absentClasses.push(cls.id);
    state.polylines[cls.id] = [];
    state.polygons[cls.id] = [];
    state.bboxes[cls.id] = [];
    showToast(`Marked ${cls.label} as absent on this capture`);
    state.recordAudit('Mark Absent', `Marked ${cls.label} as absent`);
  }
  state.emit('state_changed');
  redraw();
}

function setupHoverPreviews(): void {
  const toolTips: Record<string, { title: string; key: string; desc: string }> = {
    pan: { title: 'Pan / Hand Tool', key: 'H / Space', desc: 'Click & drag or hold Space to pan canvas. Scroll wheel to zoom centered on cursor.' },
    polyline: { title: 'Polyline / Crease Tool', key: 'L', desc: 'Click to place curve vertices. Double-click or press Enter to complete.' },
    polygon: { title: 'Polygon Tool', key: 'P', desc: 'Create closed area perimeters. Click near start vertex or press Enter to commit.' },
    brush: { title: 'Brush Tool', key: 'B', desc: 'Freehand stroke painting with configurable brush radius and draw/erase modes.' },
    bbox: { title: 'Bounding Box', key: 'R', desc: 'Click and drag rectangular region annotation.' },
    wand: { title: 'AI Contour Snap', key: 'S', desc: 'Click near any CurvEngine Steger ridge to automatically adopt it as ground truth.' },
    undo: { title: 'Undo Action', key: 'Cmd+Z', desc: 'Revert the last annotation change.' },
    redo: { title: 'Redo Action', key: 'Cmd+Y', desc: 'Reapply the last undone action.' },
    absent: { title: 'Mark Absent / Empty', key: 'A', desc: 'Flag that the active feature/crease does not exist on this capture.' },
  };

  document.querySelectorAll<HTMLElement>('.dock-btn').forEach(btn => {
    const t = btn.dataset.tool || btn.dataset.action;
    if (!t) return;
    const info = toolTips[t];
    if (!info) return;

    btn.onmouseenter = () => {
      if (!hoverCard) return;
      const elTitle = document.getElementById('hoverCardTitle');
      const elKey = document.getElementById('hoverCardKey');
      const elDesc = document.getElementById('hoverCardDesc');
      if (elTitle) elTitle.textContent = info.title;
      if (elKey) elKey.textContent = info.key;
      if (elDesc) elDesc.textContent = info.desc;

      const rect = btn.getBoundingClientRect();
      const topOffset = Math.max(20, rect.top - 10);
      hoverCard.style.top = `${topOffset}px`;
      hoverCard.classList.add('show');
    };

    btn.onmouseleave = () => {
      if (hoverCard) hoverCard.classList.remove('show');
    };
  });
}

function setupInContextPopover(): void {
  const inpSearch = document.getElementById('popoverSearch') as HTMLInputElement | null;
  const btnSave = document.getElementById('btnPopoverSave');
  const btnDelete = document.getElementById('btnPopoverDelete');

  if (btnSave) btnSave.onclick = commitPendingShape;
  if (btnDelete) btnDelete.onclick = discardPendingShape;

  if (inpSearch) {
    inpSearch.oninput = e => renderPopoverChips((e.target as HTMLInputElement).value.toLowerCase());
    inpSearch.onkeydown = e => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commitPendingShape();
      } else if (e.key === 'Escape') {
        discardPendingShape();
      } else if (['1', '2', '3', '4'].includes(e.key)) {
        const idx = Number(e.key) - 1;
        if (state.currentDomain?.classes?.[idx]) {
          state.activeClassId = state.currentDomain.classes[idx].id;
          commitPendingShape();
        }
      }
    };
  }
}

export function openInContextPopover(shape: PendingShape, screenX: number, screenY: number): void {
  pendingShape = shape;
  if (!inContextPopover) return;

  const container = document.getElementById('canvasContainer');
  if (!container) return;
  const cRect = container.getBoundingClientRect();

  let left = screenX - cRect.left + 15;
  let top = screenY - cRect.top - 40;
  if (left + 260 > cRect.width) left = cRect.width - 270;
  if (top + 220 > cRect.height) top = cRect.height - 230;
  if (left < 10) left = 10;
  if (top < 10) top = 10;

  inContextPopover.style.left = `${left}px`;
  inContextPopover.style.top = `${top}px`;
  renderPopoverChips('');

  inContextPopover.classList.add('show');
  const btnSave = document.getElementById('btnPopoverSave');
  if (btnSave) {
    btnSave.focus();
  }
}

function renderPopoverChips(query = ''): void {
  const container = document.getElementById('popoverChips');
  if (!container || !state.currentDomain) return;

  const classes = state.currentDomain.classes || [];
  const filtered = classes.filter(c => c.label.toLowerCase().includes(query) || c.id.toLowerCase().includes(query));

  container.innerHTML = filtered.map((c, i) => {
    const isSelected = c.id === state.activeClassId;
    return `
      <button class="popover-chip-btn ${isSelected ? 'selected' : ''}" data-class="${c.id}">
        <span style="display:flex;align-items:center;gap:6px">
          <span class="class-dot" style="background:${c.color}"></span>
          <span>${c.label}</span>
        </span>
        <span class="class-key">${c.shortcut || (i + 1)}</span>
      </button>
    `;
  }).join('');

  container.querySelectorAll<HTMLElement>('.popover-chip-btn').forEach(btn => {
    btn.onclick = () => {
      state.activeClassId = btn.dataset.class!;
      commitPendingShape();
    };
  });
}

function commitPendingShape(): void {
  if (!pendingShape || !state.activeClassId) {
    closePopover();
    return;
  }

  state.pushUndo();
  const clsId = state.activeClassId;

  if (pendingShape.type === 'polyline') {
    if (!state.polylines[clsId]) state.polylines[clsId] = [];
    state.polylines[clsId].push(pendingShape.points);
    state.recordAudit('Add Polyline', `Added ${clsId} polyline (${pendingShape.points.length} pts)`);
  } else if (pendingShape.type === 'polygon') {
    if (!state.polygons[clsId]) state.polygons[clsId] = [];
    state.polygons[clsId].push(pendingShape.points);
    state.recordAudit('Add Polygon', `Added ${clsId} polygon (${pendingShape.points.length} vertices)`);
  } else if (pendingShape.type === 'bbox') {
    if (!state.bboxes[clsId]) state.bboxes[clsId] = [];
    state.bboxes[clsId].push(pendingShape.points);
    state.recordAudit('Add BBox', `Added ${clsId} bounding box`);
  }

  const absentIdx = state.absentClasses.indexOf(clsId);
  if (absentIdx >= 0) state.absentClasses.splice(absentIdx, 1);

  closePopover();
  state.emit('state_changed');
  redraw();
}

function discardPendingShape(): void {
  pendingShape = null;
  closePopover();
  state.currentDraft = [];
  state.bboxStart = null;
  redraw();
}

function closePopover(): void {
  if (inContextPopover) inContextPopover.classList.remove('show');
  pendingShape = null;
}

function setupCanvasInteractions(): void {
  const container = document.getElementById('canvasContainer');
  const canvas = document.getElementById('studioCanvas') as HTMLCanvasElement | null;
  if (!container || !canvas) return;

  let isDrawingBrush = false;
  let currentBrushStroke: any = null;

  container.onmousedown = (e: MouseEvent) => {
    if (inContextPopover?.classList.contains('show') && !inContextPopover.contains(e.target as Node)) {
      discardPendingShape();
    }

    if (e.button === 1 || state.activeTool === 'pan' || (e as any).spaceKey) {
      state.isPanning = true;
      state.panStart = { x: e.clientX - state.pan.x, y: e.clientY - state.pan.y };
      container.classList.add('panning');
      return;
    }

    const rect = canvas.getBoundingClientRect();
    const [x, y] = screenToImage(e.clientX - rect.left, e.clientY - rect.top);

    if (state.activeTool === 'wand') {
      snapToNearestAIRidge([x, y]);
      return;
    }

    if (state.activeTool === 'polyline' || state.activeTool === 'polygon') {
      if (state.activeTool === 'polygon' && state.currentDraft.length >= 3) {
        const [fx, fy] = state.currentDraft[0];
        const d = Math.hypot(x - fx, y - fy);
        if (d < 15.0 / state.zoom) {
          finishDraftShape(e.clientX, e.clientY);
          return;
        }
      }
      state.currentDraft.push([x, y]);
      redraw();
      updateHud(e);
      return;
    }

    if (state.activeTool === 'bbox') {
      state.bboxStart = [x, y];
      state.currentCursor = [x, y];
      return;
    }

    if (state.activeTool === 'brush') {
      isDrawingBrush = true;
      state.pushUndo();
      currentBrushStroke = {
        classId: state.activeClassId,
        points: [[x, y]],
        size: state.brushSize,
        mode: state.brushMode,
      };
      state.brushStrokes.push(currentBrushStroke);
      redraw();
      return;
    }
  };

  window.addEventListener('mousemove', (e: MouseEvent) => {
    if (state.isPanning) {
      state.pan.x = e.clientX - state.panStart.x;
      state.pan.y = e.clientY - state.panStart.y;
      redraw();
      updateHud(e);
      return;
    }

    const rect = canvas.getBoundingClientRect();
    const [x, y] = screenToImage(e.clientX - rect.left, e.clientY - rect.top);
    state.currentCursor = [x, y];

    if (state.activeTool === 'bbox' && state.bboxStart) {
      redraw();
      updateHud(e);
      return;
    }

    if (state.activeTool === 'brush' && isDrawingBrush && currentBrushStroke) {
      currentBrushStroke.points.push([x, y]);
      redraw();
      return;
    }

    updateHud(e);
  });

  window.addEventListener('mouseup', (e: MouseEvent) => {
    if (state.isPanning) {
      state.isPanning = false;
      container.classList.remove('panning');
      return;
    }

    if (state.activeTool === 'brush' && isDrawingBrush) {
      isDrawingBrush = false;
      state.recordAudit('Brush Stroke', `Painted ${state.activeClassId} stroke`);
      state.emit('state_changed');
      return;
    }

    if (state.activeTool === 'bbox' && state.bboxStart && state.currentCursor) {
      const [x1, y1] = state.bboxStart;
      const [x2, y2] = state.currentCursor;
      const dx = Math.abs(x2 - x1);
      const dy = Math.abs(y2 - y1);
      state.bboxStart = null;
      if (dx > 5 && dy > 5) {
        const bb: BoundingBox = [Math.min(x1, x2), Math.min(y1, y2), Math.max(x1, x2), Math.max(y1, y2)];
        openInContextPopover({ type: 'bbox', points: bb }, e.clientX, e.clientY);
      }
      redraw();
    }
  });

  container.ondblclick = (e: MouseEvent) => {
    if (state.activeTool === 'polyline' || state.activeTool === 'polygon') {
      finishDraftShape(e.clientX, e.clientY);
    }
  };
}

function finishDraftShape(screenX: number, screenY: number): void {
  if (state.currentDraft.length >= 2) {
    const shapeType = state.activeTool === 'polygon' ? 'polygon' : 'polyline';
    const draftPts = [...state.currentDraft];
    state.currentDraft = [];
    openInContextPopover({ type: shapeType, points: draftPts }, screenX, screenY);
  } else {
    state.currentDraft = [];
  }
  redraw();
}

function snapToNearestAIRidge(_imgPt: Point2D): void {
  const clsId = state.activeClassId;
  if (!clsId) return;

  const pts = state.aiLines[clsId];
  if (!pts || pts.length < 2) {
    showToast(`No AI ridges detected for ${clsId} to snap.`);
    return;
  }

  state.pushUndo();
  if (!state.polylines[clsId]) state.polylines[clsId] = [];
  state.polylines[clsId].push(pts.map(p => [p[0], p[1]]));

  const absentIdx = state.absentClasses.indexOf(clsId);
  if (absentIdx >= 0) state.absentClasses.splice(absentIdx, 1);

  showToast(`Adopted AI ridge for ${clsId}!`);
  state.recordAudit('AI Contour Snap', `Adopted CurvEngine ridge for ${clsId}`);
  state.emit('state_changed');
  redraw();
}

export function showToast(msg: string): void {
  const t = document.getElementById('toast');
  if (!t) return;
  t.textContent = msg;
  t.classList.add('show');
  setTimeout(() => t.classList.remove('show'), 2400);
}
