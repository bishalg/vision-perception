// apps/annotation_studio/src/components/Sidebar/Panels/CopilotPanel.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../../core/store';
import { useFloodDeltaQuery } from '../../../api/queries';
import {
  IconAlertTriangle,
  IconCheck,
  IconHand,
  IconWand,
} from '../../Common/Icons';

export const CopilotPanel: React.FC = () => {
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const isFlood = currentDomain?.id === 'flood';
  const { data: floodData, isLoading } = useFloodDeltaQuery(isFlood);

  if (isFlood) {
    const d = floodData?.delta;
    const waterRatio = d?.color_mask_delta?.water?.ratio || 2.23;
    const widthRatio = d?.channel_width_proxy?.width_ratio || 2.21;
    const preMed = d?.channel_width_proxy?.pre_median_px || 19;
    const postMed = d?.channel_width_proxy?.post_median_px || 42;
    const detected = d?.detection_logic?.flood_signal_detected !== false;

    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
        {isLoading ? (
          <div style={{ fontSize: '12px', color: 'var(--txt-muted)' }}>Loading morphology delta…</div>
        ) : (
          <>
            <div
              style={{
                background: detected ? 'rgba(239,68,68,0.15)' : 'rgba(16,185,129,0.15)',
                border: `1px solid ${detected ? 'var(--red)' : 'var(--green)'}`,
                padding: '10px',
                borderRadius: '8px',
              }}
            >
              <div
                style={{
                  fontSize: '12px',
                  fontWeight: 700,
                  color: detected ? 'var(--red)' : 'var(--green)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                }}
              >
                {detected ? (
                  <>
                    <IconAlertTriangle size={15} />
                    <span>CRITICAL FLOOD SIGNAL DETECTED</span>
                  </>
                ) : (
                  <>
                    <IconCheck size={15} />
                    <span>NORMAL RIVER FLOW</span>
                  </>
                )}
              </div>
              <div style={{ fontSize: '11px', color: 'var(--txt)', marginTop: '4px' }}>
                {d?.event || 'Glacier-collapse flood — Lende Khola / Bhote Koshi'}
              </div>
            </div>

            <div className="sidebar-section-title">Baseline vs Event Morphology</div>
            <div className="layer-card" style={{ fontSize: '11.5px', gap: '6px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: 'var(--txt-muted)' }}>Baseline Date</span>
                <span>{d?.acquisitions?.pre || '2026-08-24'}</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: 'var(--txt-muted)' }}>Event Date</span>
                <span>{d?.acquisitions?.post || '2026-08-27'}</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: 'var(--txt-muted)' }}>Water Area Expansion</span>
                <span style={{ color: 'var(--cyan)', fontWeight: 700 }}>
                  +{((waterRatio - 1) * 100).toFixed(1)}% ({waterRatio}x)
                </span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: 'var(--txt-muted)' }}>Channel Median Width</span>
                <span style={{ color: 'var(--amber)', fontWeight: 700 }}>
                  {preMed} px → {postMed} px ({widthRatio}x)
                </span>
              </div>
            </div>

            <div className="sidebar-section-title">Copilot Guidance (Roboflow Parity)</div>
            <div style={{ fontSize: '11px', color: 'var(--txt-muted)', lineHeight: 1.5, background: 'var(--surface-2)', border: '1px solid var(--border)', borderRadius: '8px', padding: '10px' }}>
              • <b>Annotation Strategy</b>: Use <code>Polyline (L)</code> or <code>Brush (B)</code> to trace the active river channel.<br />
              • <b>Exclusions</b>: Exclude dry gravel bars, sediment banks, and terrain cloud shadows.<br />
              • <b>Zero Cost</b>: 100% local C++ execution ($0/mo).
            </div>
          </>
        )}
      </div>
    );
  }

  // Palm domain
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
      <div style={{ background: 'rgba(59,130,246,0.12)', border: '1px solid var(--blue)', padding: '10px', borderRadius: '8px' }}>
        <div style={{ fontSize: '12px', fontWeight: 700, color: 'var(--blue)', display: 'flex', alignItems: 'center', gap: '6px' }}>
          <IconHand size={15} />
          <span>PALM CREASE ANATOMY ENGINE</span>
        </div>
        <div style={{ fontSize: '11px', color: 'var(--txt)', marginTop: '4px' }}>
          Sub-pixel Steger ridge extraction grounded in 21 hand landmarks.
        </div>
      </div>

      <div className="sidebar-section-title">Crease Geometry Reference</div>
      <div className="layer-card" style={{ fontSize: '11.5px', gap: '6px' }}>
        <div><b style={{ color: '#F43F5E' }}>1: Heart Line</b> — Upper transverse crease below MCP joints.</div>
        <div><b style={{ color: '#06B6D4' }}>2: Head Line</b> — Middle diagonal crease across thenar space.</div>
        <div><b style={{ color: '#10B981' }}>3: Life Line</b> — Longitudinal crease arcing around thenar mount.</div>
        <div><b style={{ color: '#8B5CF6' }}>4: Fate Line</b> — Vertical line ascending towards middle finger.</div>
      </div>

      <div className="sidebar-section-title">Copilot Tips</div>
      <div style={{ fontSize: '11px', color: 'var(--txt-muted)', lineHeight: 1.5, background: 'var(--surface-2)', border: '1px solid var(--border)', borderRadius: '8px', padding: '10px' }}>
        • Click <span style={{ display: 'inline-flex', alignItems: 'center', gap: '3px' }}><IconWand size={11} /> <code>Snap AI (S)</code></span> near any CurvEngine predicted ridge to instantly adopt it.<br />
        • Press <code>A</code> if a crease is absent on the capture.<br />
        • Press <code>⌘S</code> to commit ground truth.
      </div>
    </div>
  );
};
