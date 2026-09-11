// apps/annotation_studio/src/components/Sidebar/Panels/AuditHistoryView.tsx
import React from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../../core/store';

export const AuditHistoryView: React.FC = () => {
  const auditLog = useStore(studioStore, s => s.auditLog);

  if (auditLog.length === 0) {
    return (
      <div style={{ color: 'var(--txt-muted)', fontSize: '12px', padding: '8px' }}>
        No edits recorded in this session yet.
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
      <div className="sidebar-section-title">Session Audit Trail</div>
      <div id="historyList" className="history-list">
        {auditLog.map(item => (
          <div key={item.id} className="history-item">
            <div className="history-top">
              <span style={{ fontWeight: 600, color: 'var(--blue)' }}>{item.action}</span>
              <span>{item.time}</span>
            </div>
            <div className="history-desc">{item.desc}</div>
          </div>
        ))}
      </div>
    </div>
  );
};
