// apps/annotation_studio/src/components/Header/SubHeader.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../core/store';

export const SubHeader: React.FC = () => {
  const currentSample = useStore(studioStore, s => s.currentSample);
  const currentDomain = useStore(studioStore, s => s.currentDomain);

  const filename = currentSample?.filename || 'No sample selected';
  const split = currentSample?.split || 'unassigned';
  const hasTruth = Boolean(currentSample?.has_truth);
  const domainLabel = currentDomain?.label || 'CurvEngine';

  return (
    <div className="sub-header-bar">
      <div className="sub-header-left">
        <span id="subHeaderFilename" className="active-filename">
          {filename}
        </span>
        <span id="subHeaderSplitBadge" className={`split-pill ${split}`}>
          {split}
        </span>
        {hasTruth && (
          <span id="subHeaderReviewedBadge" className="status-badge">
            Reviewed
          </span>
        )}
      </div>
      <div className="sub-header-right">
        <span id="subHeaderDomainMeta" className="domain-sub-meta">
          {domainLabel} • Local Engine ($0/mo)
        </span>
      </div>
    </div>
  );
};
