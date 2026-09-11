// apps/annotation_studio/src/components/Sidebar/Panels/TagsManager.tsx
import React, { useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore } from '../../../core/store';

export const TagsManager: React.FC = () => {
  const [inpTag, setInpTag] = useState('');
  const currentSample = useStore(studioStore, s => s.currentSample);
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const customTags = useStore(studioStore, s => s.customTags);

  if (!currentSample) return null;

  const stem = currentSample.stem;
  const sampleCustom = customTags[stem] || [];

  const defaultTags: Array<{ label: string; cls: string; removable?: boolean }> = [];
  if (currentSample.has_truth) defaultTags.push({ label: 'Reviewed', cls: 'active' });
  else defaultTags.push({ label: 'Needs Review', cls: '' });

  if (currentDomain?.canonical_warp) defaultTags.push({ label: 'Warped', cls: '' });
  if (currentSample.split) defaultTags.push({ label: currentSample.split.toUpperCase(), cls: 'active' });

  const allTags = [
    ...defaultTags,
    ...sampleCustom.map(t => ({ label: t, cls: 'active', removable: true })),
  ];

  const handleAdd = () => {
    const val = inpTag.trim();
    if (!val) return;
    studioStore.setState(prev => {
      const existing = prev.customTags[stem] || [];
      if (existing.includes(val)) return prev;
      return {
        ...prev,
        customTags: {
          ...prev.customTags,
          [stem]: [...existing, val],
        },
      };
    });
    setInpTag('');
  };

  const handleRemove = (tag: string) => {
    studioStore.setState(prev => {
      const existing = prev.customTags[stem] || [];
      return {
        ...prev,
        customTags: {
          ...prev.customTags,
          [stem]: existing.filter(x => x !== tag),
        },
      };
    });
  };

  return (
    <div>
      <div className="sidebar-section-title" style={{ marginTop: '16px', marginBottom: '8px' }}>
        Tags
      </div>
      <div id="sampleTags" className="tags-container">
        {allTags.map((t, idx) => (
          <span key={idx} className={`pill-tag ${t.cls}`} style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
            {t.label}
            {t.removable && (
              <span
                className="tag-remove"
                style={{ cursor: 'pointer', fontSize: '10px', marginLeft: '2px' }}
                onClick={() => handleRemove(t.label)}
              >
                ✕
              </span>
            )}
          </span>
        ))}
      </div>
      <div className="tag-input-row" style={{ marginTop: '8px' }}>
        <input
          type="text"
          id="inpNewTag"
          className="tag-input"
          placeholder="Add tag…"
          value={inpTag}
          onChange={e => setInpTag(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') handleAdd();
          }}
        />
        <button id="btnAddTag" className="btn-tag-add" onClick={handleAdd}>
          + Add
        </button>
      </div>
    </div>
  );
};
