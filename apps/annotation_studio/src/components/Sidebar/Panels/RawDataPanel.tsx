// apps/annotation_studio/src/components/Sidebar/Panels/RawDataPanel.tsx
import React, { useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../../core/store';
import { IconCopy, IconCheck } from '../../Common/Icons';

export const RawDataPanel: React.FC = () => {
  const [copied, setCopied] = useState(false);
  const state = useStore(studioStore, s => s);

  const data = {
    domain: state.currentDomain?.id,
    stem: state.currentSample?.stem,
    filename: state.currentSample?.filename,
    split: state.currentSample?.split,
    has_truth: state.currentSample?.has_truth,
    truth_file: state.currentSample?.truth_file,
    absent_classes: state.absentClasses,
    annotations: {
      polylines: state.polylines,
      polygons: state.polygons,
      bboxes: state.bboxes,
      brushStrokes: state.brushStrokes,
    },
    auto_detected_curves: state.aiLines,
  };

  const rawStr = JSON.stringify(data, null, 2);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(rawStr);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span className="sidebar-section-title">Source & Truth JSON</span>
        <button
          onClick={handleCopy}
          style={{ fontSize: '11px', padding: '3px 8px', display: 'flex', alignItems: 'center', gap: '4px' }}
        >
          {copied ? (
            <>
              <IconCheck size={12} style={{ color: 'var(--green)' }} />
              <span>Copied!</span>
            </>
          ) : (
            <>
              <IconCopy size={12} />
              <span>Copy JSON</span>
            </>
          )}
        </button>
      </div>
      <div id="rawJsonInspector" className="json-inspector-box">
        {rawStr}
      </div>
    </div>
  );
};
