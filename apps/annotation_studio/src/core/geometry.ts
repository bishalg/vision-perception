// apps/annotation_studio/src/core/geometry.ts — Pure Math & Coordinate Transformations
// Framework-agnostic geometry algorithms with zero DOM dependencies.

import type { Point2D } from './types';

export function dist(a: Point2D, b: Point2D): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1]);
}

export function polyArcLength(poly: Point2D[]): number {
  if (!poly || poly.length < 2) return 0;
  let total = 0;
  for (let i = 1; i < poly.length; ++i) {
    total += dist(poly[i - 1], poly[i]);
  }
  return total;
}

export function pointToSegmentDist(p: Point2D, a: Point2D, b: Point2D): number {
  const l2 = (b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2;
  if (l2 === 0) return dist(p, a);
  let t = ((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / l2;
  t = Math.max(0, Math.min(1, t));
  return dist(p, [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
}

export function pointToPolylineDist(p: Point2D, poly: Point2D[]): number {
  if (!poly || poly.length < 2) return Infinity;
  let minDist = Infinity;
  for (let i = 0; i < poly.length - 1; ++i) {
    const d = pointToSegmentDist(p, poly[i], poly[i + 1]);
    if (d < minDist) minDist = d;
  }
  return minDist;
}

export function screenToImageCoords(
  sx: number,
  sy: number,
  pan: { x: number; y: number },
  zoom: number,
  imageWidth: number,
  imageHeight: number,
  coordSpace: 'pixels' | 'norm_512' = 'pixels'
): Point2D {
  if (zoom <= 0) return [0, 0];
  const ix = (sx - pan.x) / zoom;
  const iy = (sy - pan.y) / zoom;
  if (coordSpace === 'norm_512' && imageWidth > 0 && imageHeight > 0) {
    return [(ix / imageWidth) * 512.0, (iy / imageHeight) * 512.0];
  }
  return [ix, iy];
}

export function imageToScreenCoords(
  ix: number,
  iy: number,
  pan: { x: number; y: number },
  zoom: number,
  imageWidth: number,
  imageHeight: number,
  coordSpace: 'pixels' | 'norm_512' = 'pixels'
): Point2D {
  let rx = ix;
  let ry = iy;
  if (coordSpace === 'norm_512' && imageWidth > 0 && imageHeight > 0) {
    rx = (ix / 512.0) * imageWidth;
    ry = (iy / 512.0) * imageHeight;
  }
  return [rx * zoom + pan.x, ry * zoom + pan.y];
}

export function findClosestRidge(
  clickPt: Point2D,
  aiLines: Record<string, Point2D[]>,
  maxThreshold = 25.0
): { classId: string; points: Point2D[]; distance: number } | null {
  let best: { classId: string; points: Point2D[]; distance: number } | null = null;
  let bestDist = maxThreshold;

  for (const [cid, pts] of Object.entries(aiLines)) {
    if (!pts || pts.length < 2) continue;
    const d = pointToPolylineDist(clickPt, pts);
    if (d < bestDist) {
      bestDist = d;
      best = { classId: cid, points: pts, distance: d };
    }
  }

  return best;
}

export function calculateAutoFit(
  containerWidth: number,
  containerHeight: number,
  imageWidth: number,
  imageHeight: number,
  padding = 64,
  maxScale = 1.0
): { zoom: number; pan: { x: number; y: number } } {
  if (imageWidth <= 0 || imageHeight <= 0) {
    return { zoom: 1.0, pan: { x: 0, y: 0 } };
  }
  const scaleX = (containerWidth - padding) / imageWidth;
  const scaleY = (containerHeight - padding) / imageHeight;
  const scale = Math.max(0.05, Math.min(scaleX, scaleY, maxScale));
  const panX = (containerWidth - imageWidth * scale) / 2;
  const panY = (containerHeight - imageHeight * scale) / 2;
  return { zoom: scale, pan: { x: panX, y: panY } };
}
