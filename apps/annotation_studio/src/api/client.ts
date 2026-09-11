// apps/annotation_studio/src/api/client.ts — Type-Safe REST Client for CurvStudio
import type { DomainConfig, SampleItem, FloodDeltaData } from '../core/types';

export interface SaveTruthPayload {
  domain: string;
  stem: string;
  target_file?: string;
  data: Record<string, any>;
  auto_lines?: Record<string, any>;
}

export interface SaveTruthResponse {
  ok: boolean;
  saved?: string;
  error?: string;
  metrics?: Record<string, any>;
}

export interface DatasetSplitPayload {
  domain: string;
  split: { train: number; valid: number; test: number };
  labeled_only?: boolean;
}

export interface DatasetSplitResponse {
  ok: boolean;
  manifest_file: string;
  counts: { train: number; valid: number; test: number; total: number };
  samples: Array<{ stem: string; split: 'train' | 'valid' | 'test' }>;
  error?: string;
}

export interface InferencePayload {
  domain: string;
  image_url?: string;
  image_base64?: string;
  confidence_threshold?: number;
}

export interface InferencePrediction {
  class_id: string;
  label?: string;
  confidence: number;
  points?: [number, number][];
  length_px?: number;
}

export interface InferenceResponse {
  ok: boolean;
  duration_ms?: number;
  predictions?: InferencePrediction[];
  overlay_url?: string | null;
  evidence?: Record<string, any>;
  stdout?: string;
  error?: string;
}

export async function fetchDomains(): Promise<DomainConfig[]> {
  const res = await fetch('/api/domains');
  if (!res.ok) throw new Error(`HTTP error ${res.status}`);
  const data = await res.json();
  return data.domains || [];
}

export async function fetchSamples(domainId: string): Promise<{ domain: DomainConfig; samples: SampleItem[] }> {
  const res = await fetch(`/api/samples?domain=${encodeURIComponent(domainId)}`);
  if (!res.ok) throw new Error(`HTTP error ${res.status}`);
  return await res.json();
}

export async function saveGroundTruth(payload: SaveTruthPayload): Promise<SaveTruthResponse> {
  const res = await fetch('/api/save_truth', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  return await res.json();
}

export async function partitionDataset(payload: DatasetSplitPayload): Promise<DatasetSplitResponse> {
  const res = await fetch('/api/dataset_split', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  return await res.json();
}

export async function runInference(payload: InferencePayload): Promise<InferenceResponse> {
  const res = await fetch('/api/run_inference', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  return await res.json();
}

export async function fetchFloodDelta(): Promise<{ ok: boolean; delta?: FloodDeltaData; message?: string }> {
  const res = await fetch('/api/flood_delta');
  return await res.json();
}
