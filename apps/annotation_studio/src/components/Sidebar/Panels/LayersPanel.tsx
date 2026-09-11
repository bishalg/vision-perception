// apps/annotation_studio/src/components/Sidebar/Panels/LayersPanel.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../../core/store';

export const LayersPanel: React.FC = () => {
  const layerVis = useStore(studioStore, s => s.layerVisibility);
  const layerOpac = useStore(studioStore, s => s.layerOpacities);
  const activeClassId = useStore(studioStore, s => s.activeClassId);
  const currentSample = useStore(studioStore, s => s.currentSample);
  const absentClasses = useStore(studioStore, s => s.absentClasses);

  const metrics = currentSample?.metrics?.[activeClassId];
  let statusStr = currentSample?.has_truth ? 'Labeled' : 'Needs Label';
  let presenceStr = absentClasses.includes(activeClassId) ? 'Absent (Human)' : '—';
  let errStr = '—';
  let lenRatioStr = '—';

  if (metrics) {
    statusStr = metrics.tp ? 'Verified Match' : (metrics.auto_present ? 'Review Discrepancy' : 'Needs Inspection');
    presenceStr = metrics.tp ? 'TP (Match)' : (metrics.fp ? 'FP (Auto Only)' : (metrics.fn ? 'FN (Human Only)' : 'TN'));
    errStr = metrics.mean_err_px != null ? `${metrics.mean_err_px.toFixed(1)} px` : '—';
    lenRatioStr = metrics.len_ratio != null ? `${metrics.len_ratio.toFixed(2)}x` : '—';
  }

  const setVisibility = (key: keyof typeof layerVis, val: boolean) => {
    studioStore.setState(prev => ({
      ...prev,
      layerVisibility: { ...prev.layerVisibility, [key]: val },
    }));
  };

  const setOpacity = (key: keyof typeof layerOpac, val: number) => {
    studioStore.setState(prev => ({
      ...prev,
      layerOpacities: { ...prev.layerOpacities, [key]: val },
    }));
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
      <div className="sidebar-section-title">Layer Visibility & Opacity</div>
      <div className="layer-card">
        <div className="layer-row">
          <label>
            <input
              type="checkbox"
              checked={layerVis.image}
              onChange={e => setVisibility('image', e.target.checked)}
            />
            {' '}Source Image
          </label>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={layerOpac.image}
            onChange={e => setOpacity('image', parseFloat(e.target.value))}
          />
        </div>

        <div className="layer-row">
          <label>
            <input
              type="checkbox"
              checked={layerVis.ai}
              onChange={e => setVisibility('ai', e.target.checked)}
            />
            {' '}CurvEngine AI Ridges
          </label>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={layerOpac.ai}
            onChange={e => setOpacity('ai', parseFloat(e.target.value))}
          />
        </div>

        <div className="layer-row">
          <label>
            <input
              type="checkbox"
              checked={layerVis.truth}
              onChange={e => setVisibility('truth', e.target.checked)}
            />
            {' '}Ground Truth
          </label>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={layerOpac.truth}
            onChange={e => setOpacity('truth', parseFloat(e.target.value))}
          />
        </div>
      </div>

      <div className="sidebar-section-title">Live Accuracy Metrics</div>
      <div className="layer-card" style={{ fontSize: '12px', gap: '6px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', color: 'var(--txt-muted)' }}>
          <span>Status</span><span style={{ color: 'var(--txt)', fontWeight: 600 }}>{statusStr}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', color: 'var(--txt-muted)' }}>
          <span>Active Class</span><span style={{ color: 'var(--txt)', fontWeight: 600 }}>{activeClassId || '—'}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', color: 'var(--txt-muted)' }}>
          <span>Presence</span><span style={{ color: 'var(--txt)', fontWeight: 600 }}>{presenceStr}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', color: 'var(--txt-muted)' }}>
          <span>Mean Error</span><span style={{ color: 'var(--txt)', fontWeight: 600 }}>{errStr}</span>
        </div>
        <div style={{ display: 'flex', justifyContent: 'space-between', color: 'var(--txt-muted)' }}>
          <span>Length Ratio</span><span style={{ color: 'var(--txt)', fontWeight: 600 }}>{lenRatioStr}</span>
        </div>
      </div>
    </div>
  );
};
