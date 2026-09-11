// apps/annotation_studio/src/components/Viewport/ThumbnailStrip.tsx
// Sleek, high-performance dataset image filmstrip with real image thumbnails and active selection sync.
import React, { useEffect, useRef } from 'react';
import type { SampleItem } from '../../core/types';
import { IconCheck } from '../Common/Icons';

interface ThumbnailStripProps {
  samples: SampleItem[];
  currentIdx: number;
  onSelectSample: (idx: number) => void;
  isOpen: boolean;
  onToggle: () => void;
}

export const ThumbnailStrip: React.FC<ThumbnailStripProps> = ({
  samples,
  currentIdx,
  onSelectSample,
  isOpen,
  onToggle,
}) => {
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);

  // Auto-scroll active thumbnail into view
  useEffect(() => {
    if (!isOpen || !scrollContainerRef.current) return;
    const activeEl = scrollContainerRef.current.querySelector<HTMLElement>(`[data-idx="${currentIdx}"]`);
    if (activeEl) {
      activeEl.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' });
    }
  }, [currentIdx, isOpen]);

  if (!isOpen) {
    return (
      <div
        style={{
          position: 'absolute',
          bottom: '18px',
          left: '20px',
          zIndex: 40,
        }}
      >
        <button
          onClick={onToggle}
          title="Show Image Filmstrip"
          style={{
            background: 'var(--surface-glass)',
            backdropFilter: 'blur(14px)',
            WebkitBackdropFilter: 'blur(14px)',
            border: '1px solid rgba(255, 255, 255, 0.12)',
            borderRadius: '9999px',
            color: 'var(--txt-muted)',
            padding: '5px 12px',
            fontSize: '11px',
            fontWeight: 600,
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            cursor: 'pointer',
            boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
          }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect width="18" height="18" x="3" y="3" rx="2" />
            <path d="M7 3v18M17 3v18M3 7.5h4M3 12h18M3 16.5h4M17 7.5h4M17 16.5h4" />
          </svg>
          <span>Images ({samples.length})</span>
        </button>
      </div>
    );
  }

  return (
    <div
      style={{
        position: 'absolute',
        bottom: 0,
        left: 0,
        right: 0,
        height: '92px',
        background: 'rgba(10, 10, 14, 0.92)',
        backdropFilter: 'blur(16px)',
        WebkitBackdropFilter: 'blur(16px)',
        borderTop: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
        zIndex: 40,
        animation: 'slideUp 0.15s ease',
      }}
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '4px 14px 2px 14px',
          fontSize: '11px',
          color: 'var(--txt-muted)',
        }}
      >
        <span style={{ fontWeight: 600, display: 'flex', alignItems: 'center', gap: '6px' }}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect width="18" height="18" x="3" y="3" rx="2" />
            <path d="M7 3v18M17 3v18M3 7.5h4M3 12h18M3 16.5h4M17 7.5h4M17 16.5h4" />
          </svg>
          Dataset Samples ({currentIdx + 1} of {samples.length})
        </span>
        <button
          onClick={onToggle}
          style={{
            background: 'transparent',
            border: 'none',
            color: 'var(--txt-muted)',
            cursor: 'pointer',
            fontSize: '13px',
            padding: '2px 6px',
          }}
          title="Hide Filmstrip"
        >
          ✕
        </button>
      </div>

      <div
        ref={scrollContainerRef}
        style={{
          display: 'flex',
          gap: '8px',
          padding: '4px 14px 8px 14px',
          overflowX: 'auto',
          overflowY: 'hidden',
          scrollbarWidth: 'thin',
        }}
      >
        {samples.map((s, idx) => {
          const isActive = idx === currentIdx;
          return (
            <div
              key={s.stem}
              data-idx={idx}
              onClick={() => onSelectSample(idx)}
              title={`${s.filename} (${s.split || 'raw'})`}
              style={{
                position: 'relative',
                flex: 'none',
                width: '56px',
                height: '56px',
                borderRadius: '6px',
                overflow: 'hidden',
                cursor: 'pointer',
                border: isActive
                  ? '2px solid var(--blue)'
                  : '1px solid rgba(255, 255, 255, 0.1)',
                boxShadow: isActive ? '0 0 10px rgba(59, 130, 246, 0.5)' : 'none',
                transition: 'all 0.15s ease',
                background: '#050508',
              }}
            >
              <img
                src={s.image_url}
                alt={s.stem}
                loading="lazy"
                style={{
                  width: '100%',
                  height: '100%',
                  objectFit: 'cover',
                  display: 'block',
                  opacity: isActive ? 1.0 : 0.82,
                }}
              />

              {/* Origin badge: gold vs bootstrap */}
              {s.origin && (
                <span
                  className={`origin-pill ${s.origin}`}
                  style={{
                    position: 'absolute',
                    top: '2px',
                    left: '2px',
                    fontSize: '7px',
                    padding: '1px 3px',
                    lineHeight: 1,
                    borderRadius: '3px',
                    fontWeight: 700,
                    letterSpacing: '0.02em',
                    background: s.origin === 'gold' ? 'rgba(245, 158, 11, 0.9)' : 'rgba(99, 102, 241, 0.9)',
                    color: '#fff',
                  }}
                  title={s.origin === 'gold' ? 'Primary / gold dataset' : 'Bootstrap import'}
                >
                  {s.origin === 'gold' ? 'G' : 'B'}
                </span>
              )}

              {/* Status Indicator */}
              {s.has_truth && (
                <span
                  style={{
                    position: 'absolute',
                    top: '3px',
                    right: '3px',
                    background: 'var(--green)',
                    color: '#fff',
                    borderRadius: '50%',
                    width: '13px',
                    height: '13px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}
                  title="Ground truth reviewed"
                >
                  <IconCheck size={8} />
                </span>
              )}

              {/* Split Pill */}
              {s.split && s.split !== 'unassigned' && (
                <span
                  className={`split-pill ${s.split}`}
                  style={{
                    position: 'absolute',
                    bottom: '2px',
                    left: '2px',
                    fontSize: '7.5px',
                    padding: '1px 3px',
                    lineHeight: 1,
                  }}
                >
                  {s.split[0].toUpperCase()}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
