// apps/annotation_studio/src/components/Sidebar/TwoTierSidebar.tsx
import React, { useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore, countTotalAnnotations, toggleAbsentClass } from '../../core/store';
import { SidebarRail } from './SidebarRail';
import { ClassPalette } from './Panels/ClassPalette';
import { UnusedClasses } from './Panels/UnusedClasses';
import { TagsManager } from './Panels/TagsManager';
import { LayersPanel } from './Panels/LayersPanel';
import { RawDataPanel } from './Panels/RawDataPanel';
import { AuditHistoryView } from './Panels/AuditHistoryView';
import { CopilotPanel } from './Panels/CopilotPanel';

export const TwoTierSidebar: React.FC = () => {
  const [subtab, setSubtab] = useState<'classes' | 'layers'>('classes');
  const activeRailTab = useStore(studioStore, s => s.activeRailTab);
  const collapsed = useStore(studioStore, s => s.sidebarCollapsed);
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const activeClassId = useStore(studioStore, s => s.activeClassId);
  const state = useStore(studioStore, s => s);

  const totalAnnotations = countTotalAnnotations(state);
  const classes = currentDomain?.classes || [];

  // Separate annotated vs unused classes
  const usedClasses: typeof classes = [];
  const unusedClasses: typeof classes = [];
  for (const c of classes) {
    const polys = (state.polylines[c.id] || []).length;
    const pgs = (state.polygons[c.id] || []).length;
    const bbs = (state.bboxes[c.id] || []).length;
    const strokes = state.brushStrokes.filter(s => s.classId === c.id).length;
    const count = polys + pgs + bbs + strokes;
    if (count > 0 || c.id === activeClassId) {
      usedClasses.push(c);
    } else {
      unusedClasses.push(c);
    }
  }

  return (
    <div className="two-tier-sidebar" id="twoTierSidebar">
      <SidebarRail />

      {!collapsed && (
        <div className="sidebar-panel" id="sidebarPanel">
          {activeRailTab === 'labels' && (
            <div id="tab-labels" className="tab-pane active">
              {/* Annotations header */}
              <div className="panel-header">
                <div className="panel-title-row">
                  <span className="panel-title">Annotations</span>
                  <span id="totalAnnotationCount" className="class-count-badge">
                    {totalAnnotations}
                  </span>
                </div>
                <div className="panel-actions">
                  <button
                    id="btnToggleAbsent"
                    className="panel-icon-btn"
                    title="Mark Active Feature Absent (A)"
                    onClick={() => {
                      if (activeClassId) toggleAbsentClass(activeClassId);
                    }}
                  >
                    ⇄
                  </button>
                </div>
              </div>

              {/* Classes & Layers Subtabs */}
              <div className="panel-subtabs">
                <button
                  className={`panel-subtab-btn ${subtab === 'classes' ? 'active' : ''}`}
                  onClick={() => setSubtab('classes')}
                >
                  Classes
                </button>
                <button
                  className={`panel-subtab-btn ${subtab === 'layers' ? 'active' : ''}`}
                  onClick={() => setSubtab('layers')}
                >
                  Layers
                </button>
              </div>

              {subtab === 'classes' ? (
                <>
                  <div className="panel-group-header">
                    <span id="panelGroupLabel" className="panel-group-label">
                      Group: {currentDomain?.id || 'active'}
                    </span>
                  </div>

                  {/* Active Annotated Classes */}
                  <ClassPalette classes={usedClasses} />

                  {/* Unused Classes Section */}
                  <div className="sidebar-section-title" style={{ marginTop: '14px' }}>
                    UNUSED CLASSES
                  </div>
                  <UnusedClasses classes={unusedClasses} />

                  {/* Tags Manager */}
                  <TagsManager />
                </>
              ) : (
                <LayersPanel />
              )}
            </div>
          )}

          {activeRailTab === 'layers' && (
            <div id="tab-layers" className="tab-pane active">
              <LayersPanel />
            </div>
          )}

          {activeRailTab === 'raw' && (
            <div id="tab-raw" className="tab-pane active">
              <RawDataPanel />
            </div>
          )}

          {activeRailTab === 'history' && (
            <div id="tab-history" className="tab-pane active">
              <AuditHistoryView />
            </div>
          )}

          {activeRailTab === 'copilot' && (
            <div id="tab-copilot" className="tab-pane active">
              <CopilotPanel />
            </div>
          )}
        </div>
      )}
    </div>
  );
};
