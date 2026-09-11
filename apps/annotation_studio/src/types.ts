// apps/annotation_studio/src/types.ts — Strict Type Contracts for CurvStudio

export type Point2D = [number, number]; // [x, y]
export type Polyline = Point2D[];
export type Polygon = Point2D[];
export type BoundingBox = [number, number, number, number]; // [minX, minY, maxX, maxY]

export type ToolType = 'pan' | 'polyline' | 'polygon' | 'brush' | 'bbox' | 'wand';
export type BrushMode = 'draw' | 'erase';
export type SplitType = 'train' | 'valid' | 'test' | 'unassigned';

export interface ClassDefinition {
  id: string;
  label: string;
  color: string;
  shortcut?: string;
  description?: string;
}

export interface DomainConfig {
  id: string;
  label: string;
  description: string;
  dataset_dir: string;
  evidence_dir?: string;
  image_extensions: string[];
  image_filter_exclude: string[];
  canonical_warp: boolean;
  coord_space: 'norm_512' | 'pixel';
  classes: ClassDefinition[];
  auto_evidence_suffix?: string;
  human_truth_suffix?: string;
}

export interface SampleMetrics {
  auto_present: boolean;
  human_present: boolean;
  tp: number;
  fp: number;
  fn: number;
  tn: number;
  mean_err_px?: number | null;
  len_ratio?: number | null;
  auto_conf?: number;
  auto_arc_px?: number;
}

export interface SampleItem {
  stem: string;
  filename: string;
  image_url: string;
  truth_file: string;
  has_truth: boolean;
  truth_data: Record<string, any>;
  evidence_file: string | null;
  has_evidence: boolean;
  auto_lines: Record<string, Point2D[]>;
  metrics: Record<string, SampleMetrics>;
  split: SplitType;
}

export interface BrushStroke {
  classId: string;
  points: Point2D[];
  size: number;
  mode: BrushMode;
}

export interface AuditEntry {
  time: string;
  action: string;
  desc: string;
}

export interface StateSnapshot {
  polylines: Record<string, Polyline[]>;
  polygons: Record<string, Polygon[]>;
  bboxes: Record<string, BoundingBox[]>;
  absentClasses: string[];
  brushStrokes: BrushStroke[];
}

export interface InferencePrediction {
  class_id: string;
  confidence: number;
  points: Point2D[];
  length_px?: number;
}

export interface InferenceResponse {
  ok: boolean;
  duration_ms: number;
  predictions: InferencePrediction[];
  overlay_url?: string;
  error?: string;
}

export interface DatasetSplitResponse {
  ok: boolean;
  manifest_file: string;
  counts: {
    total: number;
    train: number;
    valid: number;
    test: number;
  };
  samples: Array<{
    stem: string;
    filename: string;
    split: SplitType;
    has_truth: boolean;
  }>;
}

export interface FloodDeltaData {
  event?: string;
  acquisitions?: {
    pre: string;
    post: string;
  };
  registration?: {
    method: string;
    shift_vector_px?: {
      dx: number;
      dy: number;
      magnitude: number;
    };
  };
  color_mask_delta?: {
    water?: {
      ratio: number;
      pixels_pre: number;
      pixels_post: number;
    };
    sediment?: {
      ratio: number;
    };
  };
  channel_width_proxy?: {
    width_ratio: number;
    pre_median_px: number;
    post_median_px: number;
  };
  detection_logic?: {
    flood_signal_detected: boolean;
  };
}
