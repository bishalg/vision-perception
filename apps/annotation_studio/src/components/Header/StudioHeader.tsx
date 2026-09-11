// apps/annotation_studio/src/components/Header/StudioHeader.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../core/store';
import type { DomainConfig } from '../../core/types';

interface StudioHeaderProps {
  domains: DomainConfig[];
  onSelectDomain: (domainId: string) => void;
  onPrevSample: () => void;
  onNextSample: () => void;
  onOpenSplitter: () => void;
  onOpenTestBench: () => void;
  onSaveTruth: () => void;
  isSaving?: boolean;
}

export const StudioHeader: React.FC<StudioHeaderProps> = ({
  domains,
  onSelectDomain,
  onPrevSample,
  onNextSample,
  onOpenSplitter,
  onOpenTestBench,
  onSaveTruth,
  isSaving,
}) => {
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const currentIdx = useStore(studioStore, s => s.currentSampleIdx);
  const totalSamples = useStore(studioStore, s => s.samples.length);

  const toggleSidebar = () => {
    studioStore.setState(prev => ({
      ...prev,
      sidebarCollapsed: !prev.sidebarCollapsed,
    }));
  };

  return (
    <header>
      <div className="header-left">
        <button
          id="btnToggleSidebar"
          className="header-icon-btn"
          title="Toggle Sidebar (B)"
          onClick={toggleSidebar}
        >
          ◫
        </button>
        <a href="/" className="header-brand" title="CurvStudio">
          <div className="logo-icon">C</div>
          <span className="brand-title">CurvStudio</span>
        </a>

        {/* Project Tabs (Roboflow Parity) */}
        <div className="project-tabs" id="projectTabs">
          {domains.map(d => {
            const isActive = d.id === currentDomain?.id;
            const icon = d.id === 'flood' ? '🌊' : '🌿';
            return (
              <button
                key={d.id}
                className={`project-tab ${isActive ? 'active' : ''}`}
                onClick={() => onSelectDomain(d.id)}
                title={d.description || d.label}
              >
                <span>{icon}</span>
                <span>{d.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Center Sample Pagination */}
      <div className="header-center">
        <div className="sample-pager">
          <button
            id="btnPrevSample"
            className="pager-arrow"
            title="Previous sample ([)"
            onClick={onPrevSample}
          >
            ‹
          </button>
          <div className="pager-info" id="pagerInfo" title="Sample pagination">
            <span id="samplePagerLabel">
              {totalSamples > 0 ? `${currentIdx + 1} / ${totalSamples}` : '0 / 0'}
            </span>
          </div>
          <button
            id="btnNextSample"
            className="pager-arrow"
            title="Next sample (])"
            onClick={onNextSample}
          >
            ›
          </button>
        </div>
      </div>

      {/* Right Top Actions */}
      <div className="header-right">
        <div className="dataset-flow-indicator">
          <span className="flow-dots">● — ● — ●</span>
          <span className="flow-label">Dataset</span>
        </div>
        <button
          id="btnOpenSplitter"
          className="btn-purple"
          title="Split dataset into Train, Valid, Test"
          onClick={onOpenSplitter}
        >
          📊 Dataset Split
        </button>
        <button
          id="btnOpenTestBench"
          className="btn-secondary"
          title="Live Model Inference Test Bench"
          onClick={onOpenTestBench}
        >
          ⚡ Test Bench
        </button>
        <button
          id="btnSaveTruth"
          className="btn-primary"
          title="Save Ground Truth (Cmd+S)"
          onClick={onSaveTruth}
          disabled={isSaving}
        >
          {isSaving ? 'Saving…' : '💾 Save Truth'}
        </button>
        <a href="/output/admin_dashboard.html" target="_blank" rel="noreferrer" style={{ textDecoration: 'none' }}>
          <button type="button" className="btn-ghost" title="Open Admin Metrics Dashboard">
            📈
          </button>
        </a>
      </div>
    </header>
  );
};
