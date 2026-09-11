// apps/annotation_studio/src/state.ts — Fully Typed Reactive State Store
import type {
  DomainConfig,
  SampleItem,
  ToolType,
  Point2D,
  Polyline,
  Polygon,
  BoundingBox,
  BrushStroke,
  BrushMode,
  AuditEntry,
  StateSnapshot,
  ClassDefinition,
} from './types';

export class StudioStore {
  domains: DomainConfig[] = [];
  currentDomain: DomainConfig | null = null;
  samples: SampleItem[] = [];
  currentSampleIdx = 0;
  currentSample: SampleItem | null = null;

  // Tool & selection
  activeTool: ToolType = 'polyline';
  activeClassId: string = '';
  classVisibility: Record<string, boolean> = {};

  // Viewport
  image: HTMLImageElement | null = null;
  pan: { x: number; y: number } = { x: 0, y: 0 };
  zoom = 1.0;
  isPanning = false;
  panStart: { x: number; y: number } = { x: 0, y: 0 };

  // Layer visibility & opacities
  showImage = true;
  showAI = true;
  showTruth = true;
  imageOpacity = 1.0;
  aiOpacity = 0.75;
  truthOpacity = 1.0;

  // Geometries
  polylines: Record<string, Polyline[]> = {};
  polygons: Record<string, Polygon[]> = {};
  bboxes: Record<string, BoundingBox[]> = {};
  absentClasses: string[] = [];
  aiLines: Record<string, Point2D[]> = {};

  // Drafts
  currentDraft: Point2D[] = [];
  currentCursor: Point2D | null = null;
  bboxStart: Point2D | null = null;

  // Brush settings
  brushSize = 16;
  brushMode: BrushMode = 'draw';
  brushStrokes: BrushStroke[] = [];

  // History
  undoStack: StateSnapshot[] = [];
  redoStack: StateSnapshot[] = [];
  auditLog: AuditEntry[] = [];

  // Event listeners
  private _listeners: Record<string, Array<(data?: any) => void>> = {};

  on(event: string, fn: (data?: any) => void): void {
    if (!this._listeners[event]) this._listeners[event] = [];
    this._listeners[event].push(fn);
  }

  emit(event: string, data?: any): void {
    if (this._listeners[event]) {
      for (const fn of this._listeners[event]) {
        fn(data);
      }
    }
  }

  recordAudit(action: string, desc: string): void {
    const time = new Date().toLocaleTimeString();
    this.auditLog.unshift({ time, action, desc });
    if (this.auditLog.length > 50) this.auditLog.pop();
    this.emit('audit_updated', this.auditLog);
  }

  pushUndo(): void {
    const snapshot: StateSnapshot = {
      polylines: JSON.parse(JSON.stringify(this.polylines)),
      polygons: JSON.parse(JSON.stringify(this.polygons)),
      bboxes: JSON.parse(JSON.stringify(this.bboxes)),
      absentClasses: [...this.absentClasses],
      brushStrokes: JSON.parse(JSON.stringify(this.brushStrokes)),
    };
    this.undoStack.push(snapshot);
    if (this.undoStack.length > 30) this.undoStack.shift();
    this.redoStack = [];
  }

  undo(): boolean {
    if (this.undoStack.length === 0) return false;
    const current: StateSnapshot = {
      polylines: JSON.parse(JSON.stringify(this.polylines)),
      polygons: JSON.parse(JSON.stringify(this.polygons)),
      bboxes: JSON.parse(JSON.stringify(this.bboxes)),
      absentClasses: [...this.absentClasses],
      brushStrokes: JSON.parse(JSON.stringify(this.brushStrokes)),
    };
    this.redoStack.push(current);
    const prev = this.undoStack.pop()!;
    this.polylines = prev.polylines;
    this.polygons = prev.polygons;
    this.bboxes = prev.bboxes;
    this.absentClasses = prev.absentClasses;
    this.brushStrokes = prev.brushStrokes;
    this.recordAudit('Undo', 'Reverted last annotation change');
    this.emit('state_changed');
    return true;
  }

  redo(): boolean {
    if (this.redoStack.length === 0) return false;
    const next = this.redoStack.pop()!;
    const current: StateSnapshot = {
      polylines: JSON.parse(JSON.stringify(this.polylines)),
      polygons: JSON.parse(JSON.stringify(this.polygons)),
      bboxes: JSON.parse(JSON.stringify(this.bboxes)),
      absentClasses: [...this.absentClasses],
      brushStrokes: JSON.parse(JSON.stringify(this.brushStrokes)),
    };
    this.undoStack.push(current);
    this.polylines = next.polylines;
    this.polygons = next.polygons;
    this.bboxes = next.bboxes;
    this.absentClasses = next.absentClasses;
    this.brushStrokes = next.brushStrokes;
    this.recordAudit('Redo', 'Reapplied undone annotation change');
    this.emit('state_changed');
    return true;
  }

  getActiveClass(): ClassDefinition | null {
    if (!this.currentDomain?.classes) return null;
    return this.currentDomain.classes.find(c => c.id === this.activeClassId) || this.currentDomain.classes[0] || null;
  }

  countTotalAnnotations(): number {
    let count = 0;
    for (const arr of Object.values(this.polylines)) count += arr.length;
    for (const arr of Object.values(this.polygons)) count += arr.length;
    for (const arr of Object.values(this.bboxes)) count += arr.length;
    count += this.brushStrokes.length;
    return count;
  }

  countClassAnnotations(classId: string): number {
    let count = 0;
    if (this.polylines[classId]) count += this.polylines[classId].length;
    if (this.polygons[classId]) count += this.polygons[classId].length;
    if (this.bboxes[classId]) count += this.bboxes[classId].length;
    count += this.brushStrokes.filter(s => s.classId === classId).length;
    return count;
  }
}

export const state = new StudioStore();
