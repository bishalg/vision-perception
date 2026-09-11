// apps/annotation_studio/src/components/Sidebar/SidebarRail.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../core/store';

export const SidebarRail: React.FC = () => {
  const activeTab = useStore(studioStore, s => s.activeRailTab);

  const tabs: Array<{ id: 'labels' | 'layers' | 'history' | 'raw' | 'copilot'; icon: string; label: string }> = [
    { id: 'labels', icon: '🏷️', label: 'Labels' },
    { id: 'layers', icon: '☰', label: 'Layers' },
    { id: 'history', icon: '🕒', label: 'History' },
    { id: 'raw', icon: '</>', label: 'Raw Data' },
    { id: 'copilot', icon: '✨', label: 'Copilot' },
  ];

  const handleSelect = (tab: typeof activeTab) => {
    studioStore.setState(prev => ({
      ...prev,
      activeRailTab: tab,
      sidebarCollapsed: false,
    }));
  };

  return (
    <div className="sidebar-rail" id="sidebarRail">
      {tabs.map(t => {
        const isActive = t.id === activeTab;
        return (
          <button
            key={t.id}
            className={`rail-btn ${isActive ? 'active' : ''}`}
            data-tab={t.id}
            title={t.label}
            onClick={() => handleSelect(t.id)}
          >
            <span className="rail-icon">{t.icon}</span>
            <span className="rail-label">{t.label}</span>
          </button>
        );
      })}
    </div>
  );
};
