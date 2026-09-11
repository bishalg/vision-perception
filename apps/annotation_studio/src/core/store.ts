// apps/annotation_studio/src/core/store.ts — Headless TanStack State Store
// 100% Framework-Agnostic Reactive Store for CurvStudio annotation engine.

import { Store } from '@tanstack/store';
import type {
  Point2D,
  ToolType,
  DomainConfig,
  SampleItem,
  BrushStroke,
  AuditEntry,
  StudioSnapshot,
  ViewportMode,
} from './types';
import { calculateAutoFit } from './geometry';

export interface StudioState {
  domains: DomainConfig[];
  currentDomain: DomainConfig | null;
  samples: SampleItem[];
  currentSampleIdx: number;
  currentSample: SampleItem | null;
  activeTool: ToolType;
  activeClassId: string;
  classVisibility: Record<string, boolean>;
  absentClasses: string[];
  zoom: number;
  pan: { x: number; y: number };
  imageBounds: { width: number; height: number };
  polylines: Record<string, Point2D[][]>;
  polygons: Record<string, Point2D[][]>;
  bboxes: Record<string, [number, number, number, number][]>;
  brushStrokes: BrushStroke[];
  brushSettings: { mode: 'draw' | 'erase'; size: number };
  aiLines: Record<string, Point2D[]>;
  currentDraft: Point2D[];
  draftClassId: string | null;
  bboxStart: Point2D | null;
  currentCursor: Point2D | null;
  layerVisibility: { image: boolean; ai: boolean; truth: boolean };
  layerOpacities: { image: number; ai: number; truth: number };
  undoStack: StudioSnapshot[];
  redoStack: StudioSnapshot[];
  auditLog: AuditEntry[];
  customTags: Record<string, string[]>;
  activeRailTab: 'labels' | 'layers' | 'history' | 'raw' | 'copilot';
  sidebarCollapsed: boolean;
  contrastMode: boolean;
  viewportMode: ViewportMode;
  /** Flood pre/post: which still the Source pane shows */
  sourceTemporal: 'current' | 'pre' | 'post';
}

export const initialStudioState: StudioState = {
  domains: [],
  currentDomain: null,
  samples: [],
  currentSampleIdx: 0,
  currentSample: null,
  activeTool: 'polyline',
  activeClassId: '',
  classVisibility: {},
  absentClasses: [],
  zoom: 1.0,
  pan: { x: 0, y: 0 },
  imageBounds: { width: 640, height: 640 },
  polylines: {},
  polygons: {},
  bboxes: {},
  brushStrokes: [],
  brushSettings: { mode: 'draw', size: 15 },
  aiLines: {},
  currentDraft: [],
  draftClassId: null,
  bboxStart: null,
  currentCursor: null,
  layerVisibility: { image: true, ai: true, truth: true },
  layerOpacities: { image: 1.0, ai: 0.75, truth: 1.0 },
  undoStack: [],
  redoStack: [],
  auditLog: [],
  customTags: {},
  activeRailTab: 'labels',
  sidebarCollapsed: false,
  contrastMode: false,
  viewportMode: 'draw',
  sourceTemporal: 'current',
};

export const studioStore = new Store<StudioState>(initialStudioState);

// --- State Mutations & Action Helpers ---

export function createSnapshot(state: StudioState): StudioSnapshot {
  return {
    polylines: JSON.parse(JSON.stringify(state.polylines)),
    polygons: JSON.parse(JSON.stringify(state.polygons)),
    bboxes: JSON.parse(JSON.stringify(state.bboxes)),
    brushStrokes: JSON.parse(JSON.stringify(state.brushStrokes)),
    absentClasses: [...state.absentClasses],
  };
}

export function recordSnapshot(): void {
  studioStore.setState(prev => ({
    ...prev,
    undoStack: [...prev.undoStack.slice(-24), createSnapshot(prev)],
    redoStack: [],
  }));
}

export function performUndo(): boolean {
  let restored = false;
  studioStore.setState(prev => {
    // 1. If drafting a shape, pop the last vertex
    if (prev.currentDraft.length > 0) {
      restored = true;
      const nextDraft = prev.currentDraft.slice(0, -1);
      return {
        ...prev,
        currentDraft: nextDraft,
        draftClassId: nextDraft.length === 0 ? null : prev.draftClassId,
      };
    }

    // 2. Otherwise revert from snapshot history
    if (prev.undoStack.length === 0) return prev;
    restored = true;
    const nextUndo = [...prev.undoStack];
    const snap = nextUndo.pop()!;
    const currentSnap = createSnapshot(prev);

    return {
      ...prev,
      undoStack: nextUndo,
      redoStack: [...prev.redoStack, currentSnap],
      polylines: snap.polylines,
      polygons: snap.polygons,
      bboxes: snap.bboxes,
      brushStrokes: snap.brushStrokes,
      absentClasses: snap.absentClasses,
    };
  });
  return restored;
}

export function performRedo(): boolean {
  let restored = false;
  studioStore.setState(prev => {
    if (prev.redoStack.length === 0) return prev;
    restored = true;
    const nextRedo = [...prev.redoStack];
    const snap = nextRedo.pop()!;
    const currentSnap = createSnapshot(prev);

    return {
      ...prev,
      redoStack: nextRedo,
      undoStack: [...prev.undoStack, currentSnap],
      polylines: snap.polylines,
      polygons: snap.polygons,
      bboxes: snap.bboxes,
      brushStrokes: snap.brushStrokes,
      absentClasses: snap.absentClasses,
    };
  });
  return restored;
}

export function commitCurrentDraft(): boolean {
  let committed = false;
  studioStore.setState(prev => {
    const targetClass = prev.draftClassId || prev.activeClassId;
    const pts = prev.currentDraft;
    if (!targetClass || pts.length < 2) {
      return { ...prev, currentDraft: [], draftClassId: null };
    }

    committed = true;
    const nextUndo = [...prev.undoStack.slice(-24), createSnapshot(prev)];

    if (prev.activeTool === 'polygon' && pts.length >= 3) {
      return {
        ...prev,
        undoStack: nextUndo,
        redoStack: [],
        polygons: {
          ...prev.polygons,
          [targetClass]: [...(prev.polygons[targetClass] || []), pts],
        },
        currentDraft: [],
        draftClassId: null,
      };
    } else {
      return {
        ...prev,
        undoStack: nextUndo,
        redoStack: [],
        polylines: {
          ...prev.polylines,
          [targetClass]: [...(prev.polylines[targetClass] || []), pts],
        },
        currentDraft: [],
        draftClassId: null,
      };
    }
  });

  if (committed) {
    const active = studioStore.state.activeClassId;
    addAuditLog('Shape Committed', `Committed annotation for ${active}`);
  }
  return committed;
}

export function cancelCurrentDraft(): void {
  studioStore.setState(prev => ({
    ...prev,
    currentDraft: [],
    draftClassId: null,
    bboxStart: null,
  }));
}

export function setTool(tool: ToolType): void {
  if (studioStore.state.activeTool === tool) return;

  if (studioStore.state.currentDraft.length >= 2) {
    commitCurrentDraft();
  } else {
    cancelCurrentDraft();
  }

  studioStore.setState(prev => ({
    ...prev,
    activeTool: tool,
    currentDraft: [],
    draftClassId: null,
    bboxStart: null,
  }));
}

export function selectClass(classId: string): void {
  if (studioStore.state.activeClassId === classId) return;

  if (studioStore.state.currentDraft.length >= 2) {
    commitCurrentDraft();
  } else {
    cancelCurrentDraft();
  }

  const cls = studioStore.state.currentDomain?.classes.find(c => c.id === classId);
  const defaultTool = cls?.default_tool;

  studioStore.setState(prev => ({
    ...prev,
    activeClassId: classId,
    ...(defaultTool && prev.activeTool !== 'pan' ? { activeTool: defaultTool } : {}),
  }));

  addAuditLog('Class Selected', `Active class switched to ${classId}`);
}

export function setViewportMode(mode: ViewportMode): void {
  studioStore.setState(prev => ({
    ...prev,
    viewportMode: mode,
  }));
  addAuditLog('Viewport Mode', `Switched to ${mode} mode`);
}

export function toggleClassVisibility(classId: string): void {
  studioStore.setState(prev => ({
    ...prev,
    classVisibility: {
      ...prev.classVisibility,
      [classId]: prev.classVisibility[classId] === false,
    },
  }));
}

export function toggleAbsentClass(classId: string): void {
  recordSnapshot();
  studioStore.setState(prev => {
    const isAbsent = prev.absentClasses.includes(classId);
    const updated = isAbsent
      ? prev.absentClasses.filter(c => c !== classId)
      : [...prev.absentClasses, classId];
    return {
      ...prev,
      absentClasses: updated,
    };
  });
}

export function setZoomPan(zoom: number, pan: { x: number; y: number }): void {
  studioStore.setState(prev => ({
    ...prev,
    zoom: Math.max(0.05, Math.min(zoom, 15.0)),
    pan,
  }));
}

export function resetView(viewportWidth: number, viewportHeight: number): void {
  studioStore.setState(prev => {
    const fit = calculateAutoFit(
      viewportWidth,
      viewportHeight,
      prev.imageBounds.width,
      prev.imageBounds.height,
      64,
      1.0
    );
    return {
      ...prev,
      zoom: fit.zoom,
      pan: fit.pan,
    };
  });
}

export function zoomStep(factor: number, centerX: number, centerY: number): void {
  studioStore.setState(prev => {
    const newZoom = Math.max(0.05, Math.min(prev.zoom * factor, 15.0));
    const newPan = {
      x: centerX - (centerX - prev.pan.x) * factor,
      y: centerY - (centerY - prev.pan.y) * factor,
    };
    return {
      ...prev,
      zoom: newZoom,
      pan: newPan,
    };
  });
}

export function addAuditLog(action: string, desc: string): void {
  const entry: AuditEntry = {
    id: Math.random().toString(36).substring(2, 9),
    time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
    action,
    desc,
  };
  studioStore.setState(prev => ({
    ...prev,
    auditLog: [entry, ...prev.auditLog.slice(0, 49)],
  }));
}

export function countClassAnnotations(state: StudioState, classId: string): number {
  const polys = (state.polylines[classId] || []).length;
  const pgs = (state.polygons[classId] || []).length;
  const bbs = (state.bboxes[classId] || []).length;
  const strokes = state.brushStrokes.filter(s => s.classId === classId).length;
  return polys + pgs + bbs + strokes;
}

export function countTotalAnnotations(state: StudioState): number {
  let count = 0;
  for (const pts of Object.values(state.polylines)) count += pts.length;
  for (const pts of Object.values(state.polygons)) count += pts.length;
  for (const b of Object.values(state.bboxes)) count += b.length;
  count += state.brushStrokes.length;
  return count;
}
