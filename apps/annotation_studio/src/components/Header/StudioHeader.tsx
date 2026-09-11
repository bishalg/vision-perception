// apps/annotation_studio/src/components/Header/StudioHeader.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore, setViewportMode } from '../../core/store';
import type { DomainConfig, ViewportMode } from '../../core/types';
import {
  IconWave,
  IconHand,
  IconSplit,
  IconPlay,
  IconSave,
  IconChart,
} from '../Common/Icons';

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
  const viewportMode = useStore(studioStore, s => s.viewportMode);

  const toggleSidebar = () => {
    studioStore.setState(prev => ({
      ...prev,
      sidebarCollapsed: !prev.sidebarCollapsed,
    }));
  };

  const modes: Array<{ id: ViewportMode; label: string }> = [
    { id: 'draw', label: 'Draw' },
    { id: 'compare', label: 'Compare' },
  ];

  return (
    <header>
      <div className="header-left">
        <button
          id="btnToggleSidebar"
          className="header-icon-btn"
          title="Toggle Sidebar (B)"
          onClick={toggleSidebar}
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect width="18" height="18" x="3" y="3" rx="2" />
            <path d="M9 3v18" />
          </svg>
        </button>
        <a href="/" className="header-brand" title="CurvStudio">
          <div className="logo-icon">C</div>
          <span className="brand-title">CurvStudio</span>
        </a>

        <div className="project-tabs" id="projectTabs">
          {domains.map(d => {
            const isActive = d.id === currentDomain?.id;
            const icon = d.id === 'flood' ? <IconWave size={14} /> : <IconHand size={14} />;
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

        <div className="viewport-mode-toggle" role="group" aria-label="Viewport mode">
          {modes.map(m => (
            <button
              key={m.id}
              type="button"
              className={`viewport-mode-btn ${viewportMode === m.id ? 'active' : ''}`}
              onClick={() => setViewportMode(m.id)}
              title={m.id === 'draw' ? 'Overlay draw mode' : 'Side-by-side Source | Engine | Yours'}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>

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
          <IconSplit size={14} />
          <span>Dataset Split</span>
        </button>
        <button
          id="btnOpenTestBench"
          className="btn-secondary"
          title="Live Model Inference Test Bench"
          onClick={onOpenTestBench}
        >
          <IconPlay size={13} />
          <span>Test Bench</span>
        </button>
        <button
          id="btnSaveTruth"
          className="btn-primary"
          title="Save Ground Truth (Cmd+S)"
          onClick={onSaveTruth}
          disabled={isSaving}
        >
          <IconSave size={14} />
          <span>{isSaving ? 'Saving…' : 'Save Truth'}</span>
        </button>
        <a href="/output/admin_dashboard.html" target="_blank" rel="noreferrer" style={{ textDecoration: 'none' }}>
          <button type="button" className="btn-ghost" title="Open Admin Metrics Dashboard">
            <IconChart size={15} />
          </button>
        </a>
      </div>
    </header>
  );
};
