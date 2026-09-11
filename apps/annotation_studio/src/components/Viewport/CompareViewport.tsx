// apps/annotation_studio/src/components/Viewport/CompareViewport.tsx
// Three synced panes: Source | Engine | Yours — same zoom/pan, different layer masks.
import React, { useRef, useEffect, useCallback, useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore, zoomStep, resetView } from '../../core/store';
import { renderStudioFrame, type RenderLayerMask } from '../../core/renderDriver';
import { ThumbnailStrip } from './ThumbnailStrip';
import type { SampleMetrics } from '../../core/types';

interface CompareViewportProps {
  onSelectSample?: (idx: number) => void;
}

type PaneId = 'source' | 'engine' | 'yours';

const PANE_MASKS: Record<PaneId, RenderLayerMask> = {
  source: { image: true, ai: false, truth: false, draft: false, ghostTruth: true },
  engine: { image: true, ai: true, truth: false, draft: false },
  yours: { image: true, ai: false, truth: true, draft: false },
};

const PANE_TITLES: Record<PaneId, string> = {
  source: 'Source',
  engine: 'Engine',
  yours: 'Yours',
};

const PANE_HINTS: Record<PaneId, string> = {
  source: 'Photo + faint import/truth ghost',
  engine: 'Pipeline proposal (dashed)',
  yours: 'Human-confirmed drawing',
};

function aggregateMetrics(
  metrics: Record<string, SampleMetrics> | undefined
): { tp: number; fp: number; fn: number; tn: number; meanErr: number | null; lenRatio: number | null } {
  if (!metrics) return { tp: 0, fp: 0, fn: 0, tn: 0, meanErr: null, lenRatio: null };
  let tp = 0, fp = 0, fn = 0, tn = 0;
  const errs: number[] = [];
  const ratios: number[] = [];
  for (const m of Object.values(metrics)) {
    tp += m.tp || 0;
    fp += m.fp || 0;
    fn += m.fn || 0;
    tn += m.tn || 0;
    if (m.mean_err_px != null) errs.push(m.mean_err_px);
    if (m.len_ratio != null) ratios.push(m.len_ratio);
  }
  return {
    tp, fp, fn, tn,
    meanErr: errs.length ? errs.reduce((a, b) => a + b, 0) / errs.length : null,
    lenRatio: ratios.length ? ratios.reduce((a, b) => a + b, 0) / ratios.length : null,
  };
}

export const CompareViewport: React.FC<CompareViewportProps> = ({ onSelectSample }) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const paneRefs = useRef<Record<PaneId, HTMLCanvasElement | null>>({
    source: null,
    engine: null,
    yours: null,
  });
  const imageRef = useRef<HTMLImageElement | null>(null);
  const preImageRef = useRef<HTMLImageElement | null>(null);
  const postImageRef = useRef<HTMLImageElement | null>(null);
  const [showFilmstrip, setShowFilmstrip] = useState(true);
  const isPanningRef = useRef(false);
  const lastMousePosRef = useRef({ x: 0, y: 0 });

  const currentSample = useStore(studioStore, s => s.currentSample);
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const samples = useStore(studioStore, s => s.samples);
  const currentSampleIdx = useStore(studioStore, s => s.currentSampleIdx);
  const sourceTemporal = useStore(studioStore, s => s.sourceTemporal);
  const zoom = useStore(studioStore, s => s.zoom);

  const supportsPrePost = (currentDomain?.compare_modes || []).includes('pre_post');
  const agg = aggregateMetrics(currentSample?.metrics);

  const resolveSourceImage = useCallback((): CanvasImageSource | null => {
    if (supportsPrePost && sourceTemporal === 'pre' && preImageRef.current) {
      return preImageRef.current;
    }
    if (supportsPrePost && sourceTemporal === 'post' && postImageRef.current) {
      return postImageRef.current;
    }
    return imageRef.current;
  }, [supportsPrePost, sourceTemporal]);

  const redrawAll = useCallback(() => {
    const state = studioStore.state;
    for (const id of Object.keys(PANE_MASKS) as PaneId[]) {
      const canvas = paneRefs.current[id];
      if (!canvas) continue;
      const ctx = canvas.getContext('2d');
      if (!ctx) continue;
      const img = id === 'source' ? resolveSourceImage() : imageRef.current;
      renderStudioFrame(ctx, state, img, canvas.width, canvas.height, PANE_MASKS[id]);
    }
  }, [resolveSourceImage]);

  // Load primary + optional pre/post images
  useEffect(() => {
    if (!currentSample?.image_url) {
      imageRef.current = null;
      redrawAll();
      return;
    }
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      imageRef.current = img;
      studioStore.setState(prev => ({
        ...prev,
        imageBounds: {
          width: img.naturalWidth || 640,
          height: img.naturalHeight || 640,
        },
      }));
      if (containerRef.current) {
        const paneW = Math.max(100, Math.floor(containerRef.current.clientWidth / 3) - 8);
        const paneH = Math.max(100, containerRef.current.clientHeight - 120);
        resetView(paneW, paneH);
      }
      redrawAll();
    };
    img.src = currentSample.image_url;

    if (currentSample.pre_image_url) {
      const pre = new Image();
      pre.crossOrigin = 'anonymous';
      pre.onload = () => {
        preImageRef.current = pre;
        redrawAll();
      };
      pre.src = currentSample.pre_image_url;
    } else {
      preImageRef.current = null;
    }

    if (currentSample.post_image_url) {
      const post = new Image();
      post.crossOrigin = 'anonymous';
      post.onload = () => {
        postImageRef.current = post;
        redrawAll();
      };
      post.src = currentSample.post_image_url;
    } else {
      postImageRef.current = null;
    }
  }, [currentSample?.image_url, currentSample?.pre_image_url, currentSample?.post_image_url, redrawAll]);

  // Sync canvas sizes
  useEffect(() => {
    const update = () => {
      if (!containerRef.current) return;
      const w = Math.max(100, Math.floor(containerRef.current.clientWidth / 3) - 8);
      const h = Math.max(100, containerRef.current.clientHeight - 120);
      for (const id of Object.keys(PANE_MASKS) as PaneId[]) {
        const canvas = paneRefs.current[id];
        if (canvas && (canvas.width !== w || canvas.height !== h)) {
          canvas.width = w;
          canvas.height = h;
        }
      }
      redrawAll();
    };
    update();
    const ro = new ResizeObserver(update);
    if (containerRef.current) ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, [redrawAll]);

  useEffect(() => {
    const unsub = studioStore.subscribe(() => {
      requestAnimationFrame(redrawAll);
    });
    redrawAll();
    return () => unsub();
  }, [redrawAll]);

  const handleWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    const canvas = e.currentTarget as HTMLCanvasElement;
    const rect = canvas.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;
    zoomStep(e.deltaY < 0 ? 1.05 : 0.95, cx, cy);
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    isPanningRef.current = true;
    lastMousePosRef.current = { x: e.clientX, y: e.clientY };
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!isPanningRef.current) return;
    const dx = e.clientX - lastMousePosRef.current.x;
    const dy = e.clientY - lastMousePosRef.current.y;
    lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    studioStore.setState(prev => ({
      ...prev,
      pan: { x: prev.pan.x + dx, y: prev.pan.y + dy },
    }));
  };

  const handleMouseUp = () => {
    isPanningRef.current = false;
  };

  return (
    <div
      className="canvas-container compare-viewport"
      ref={containerRef}
      style={{ position: 'relative', flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}
    >
      {supportsPrePost && (
        <div className="compare-temporal-bar">
          <span className="compare-temporal-label">Source pane</span>
          {(['current', 'pre', 'post'] as const).map(t => (
            <button
              key={t}
              type="button"
              className={`compare-temporal-btn ${sourceTemporal === t ? 'active' : ''}`}
              disabled={t === 'pre' && !currentSample?.pre_image_url}
              onClick={() => studioStore.setState(prev => ({ ...prev, sourceTemporal: t }))}
            >
              {t === 'current' ? 'Current' : t === 'pre' ? 'Pre-event' : 'Post-event'}
            </button>
          ))}
        </div>
      )}

      <div className="compare-panes">
        {(Object.keys(PANE_MASKS) as PaneId[]).map(id => (
          <div key={id} className="compare-pane">
            <div className="compare-pane-header">
              <strong>{PANE_TITLES[id]}</strong>
              <span>{PANE_HINTS[id]}</span>
            </div>
            <canvas
              ref={el => {
                paneRefs.current[id] = el;
              }}
              className="compare-pane-canvas"
              onWheel={handleWheel}
              onMouseDown={handleMouseDown}
              onMouseMove={handleMouseMove}
              onMouseUp={handleMouseUp}
              onMouseLeave={handleMouseUp}
            />
          </div>
        ))}
      </div>

      <div className="compare-metrics-strip">
        <span>TP {agg.tp}</span>
        <span>FP {agg.fp}</span>
        <span>FN {agg.fn}</span>
        <span>TN {agg.tn}</span>
        <span>mean err {agg.meanErr != null ? `${agg.meanErr.toFixed(1)} px` : '—'}</span>
        <span>len ratio {agg.lenRatio != null ? `${agg.lenRatio.toFixed(2)}×` : '—'}</span>
        <span>zoom {Math.round(zoom * 100)}%</span>
      </div>

      {samples.length > 0 && onSelectSample && (
        <ThumbnailStrip
          samples={samples}
          currentIdx={currentSampleIdx}
          onSelectSample={onSelectSample}
          isOpen={showFilmstrip}
          onToggle={() => setShowFilmstrip(v => !v)}
        />
      )}
    </div>
  );
};
