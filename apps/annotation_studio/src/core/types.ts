// apps/annotation_studio/src/core/types.ts — Framework-Agnostic Core Data Types
// 100% Zero DOM & Zero React dependencies — portable to Web, React Native / Expo, & Native wrappers.

export type Point2D = [number, number];

export type ToolType = 'pan' | 'polyline' | 'polygon' | 'brush' | 'bbox' | 'wand';

export interface AnnotationClass {
  id: string;
  label: string;
  color: string;
  order?: number;
  type?: 'curve' | 'polygon' | 'bbox';
}

export interface DomainConfig {
  id: string;
  label: string;
  description?: string;
  classes: AnnotationClass[];
  coord_space?: 'pixels' | 'norm_512';
  canonical_warp?: boolean;
}

export interface SampleMetrics {
  tp: number;
  fp: number;
  fn: number;
  tn: number;
  auto_present?: boolean;
  mean_err_px?: number | null;
  len_ratio?: number | null;
}

export interface SampleItem {
  stem: string;
  filename: string;
  image_url: string;
  has_truth: boolean;
  truth_file?: string;
  split?: 'train' | 'valid' | 'test' | 'unassigned';
  truth_data?: Record<string, any>;
  auto_lines?: Record<string, Point2D[]>;
  metrics?: Record<string, SampleMetrics>;
}

export interface BrushStroke {
  id: string;
  classId: string;
  points: Point2D[];
  size: number;
  mode: 'draw' | 'erase';
}

export interface AuditEntry {
  id: string;
  time: string;
  action: string;
  desc: string;
}

export interface StudioSnapshot {
  polylines: Record<string, Point2D[][]>;
  polygons: Record<string, Point2D[][]>;
  bboxes: Record<string, [number, number, number, number][]>;
  brushStrokes: BrushStroke[];
  absentClasses: string[];
}

export interface FloodDeltaData {
  event?: string;
  acquisitions?: { pre?: string; post?: string };
  color_mask_delta?: { water?: { ratio?: number } };
  channel_width_proxy?: { width_ratio?: number; pre_median_px?: number; post_median_px?: number };
  registration?: { shift_vector_px?: { magnitude?: number } };
  detection_logic?: { flood_signal_detected?: boolean };
}
