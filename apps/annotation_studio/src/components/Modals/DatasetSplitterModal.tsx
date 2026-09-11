// apps/annotation_studio/src/components/Modals/DatasetSplitterModal.tsx
import React, { useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore, addAuditLog } from '../../core/store';
import { useDatasetSplitMutation } from '../../api/queries';

interface DatasetSplitterModalProps {
  isOpen: boolean;
  onClose: () => void;
  onToast: (msg: string) => void;
}

export const DatasetSplitterModal: React.FC<DatasetSplitterModalProps> = ({
  isOpen,
  onClose,
  onToast,
}) => {
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const samples = useStore(studioStore, s => s.samples);

  const [trainPct, setTrainPct] = useState(70);
  const [validPct, setValidPct] = useState(20);
  const [labeledOnly, setLabeledOnly] = useState(true);

  const splitMutation = useDatasetSplitMutation();

  if (!isOpen) return null;

  let tr = Math.max(0, Math.min(100, trainPct));
  let va = Math.max(0, Math.min(100 - tr, validPct));
  let te = Math.max(0, 100 - tr - va);

  const targetSamples = labeledOnly ? samples.filter(s => s.has_truth) : samples;
  const n = targetSamples.length;

  const countTrain = Math.round((tr / 100) * n);
  const countValid = Math.round((va / 100) * n);
  const countTest = Math.max(0, n - countTrain - countValid);

  const handleTrainChange = (val: number) => {
    const clamped = Math.max(0, Math.min(100, val));
    setTrainPct(clamped);
    if (clamped + validPct > 100) {
      setValidPct(100 - clamped);
    }
  };

  const handleValidChange = (val: number) => {
    const clamped = Math.max(0, Math.min(100 - trainPct, val));
    setValidPct(clamped);
  };

  const handleCommit = async () => {
    if (!currentDomain) return;

    try {
      const payload = {
        domain: currentDomain.id,
        split: {
          train: tr,
          valid: va,
          test: te,
        },
        labeled_only: labeledOnly,
      };

      const result = await splitMutation.mutateAsync(payload);
      if (result.ok) {
        onToast(`Partition saved to ${result.manifest_file || 'manifest.json'}`);

        // Update samples in studioStore
        if (result.samples) {
          studioStore.setState(prev => {
            const updated = prev.samples.map(s => {
              const resSample = result.samples.find(x => x.stem === s.stem);
              return resSample ? { ...s, split: resSample.split } : s;
            });
            const cur = prev.currentSample
              ? updated.find(s => s.stem === prev.currentSample?.stem) || null
              : null;
            return {
              ...prev,
              samples: updated,
              currentSample: cur,
            };
          });
        }

        addAuditLog(
          'Dataset Split',
          `Committed split: ${tr}% Train / ${va}% Valid / ${te}% Test`
        );
        onClose();
      } else {
        onToast('Split failed: ' + (result.error || 'Unknown error'));
      }
    } catch (err: any) {
      onToast('Error: ' + err.message);
    }
  };

  return (
    <div
      className="modal-backdrop show"
      id="datasetSplitModal"
      onClick={onClose}
    >
      <div
        className="modal-window"
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <span className="modal-title">Dataset Partition & Split (Train / Valid / Test)</span>
          <button className="modal-close" id="btnCloseSplitter" onClick={onClose}>
            ✕
          </button>
        </div>

        <div className="modal-body">
          <div style={{ fontSize: '12.5px', color: 'var(--txt-muted)' }}>
            Partition domain dataset into Train, Validation, and Test splits. Commits assignments directly to <code>manifest.json</code>.
          </div>

          {/* Segmented 3-way Split Bar */}
          <div className="split-bar">
            <div className="split-segment train" id="segTrain" style={{ width: `${tr}%` }}>
              {tr}% Train
            </div>
            <div className="split-segment valid" id="segValid" style={{ width: `${va}%` }}>
              {va}% Valid
            </div>
            <div className="split-segment test" id="segTest" style={{ width: `${te}%` }}>
              {te}% Test
            </div>
          </div>

          {/* Controls Grid */}
          <div className="split-controls-grid">
            <div className="split-col" style={{ borderTop: '3px solid var(--train-color)' }}>
              <div className="split-col-header">
                <span>Train</span>
                <input
                  type="number"
                  id="splitTrainPct"
                  className="split-pct-input"
                  min="0"
                  max="100"
                  value={tr}
                  onChange={e => handleTrainChange(parseInt(e.target.value || '0', 10))}
                />
                %
              </div>
              <span id="countTrain" style={{ fontSize: '11px', color: 'var(--txt-muted)' }}>
                {countTrain} images
              </span>
            </div>

            <div className="split-col" style={{ borderTop: '3px solid var(--valid-color)' }}>
              <div className="split-col-header">
                <span>Valid</span>
                <input
                  type="number"
                  id="splitValidPct"
                  className="split-pct-input"
                  min="0"
                  max="100"
                  value={va}
                  onChange={e => handleValidChange(parseInt(e.target.value || '0', 10))}
                />
                %
              </div>
              <span id="countValid" style={{ fontSize: '11px', color: 'var(--txt-muted)' }}>
                {countValid} images
              </span>
            </div>

            <div className="split-col" style={{ borderTop: '3px solid var(--test-color)' }}>
              <div className="split-col-header">
                <span>Test</span>
                <input
                  type="number"
                  id="splitTestPct"
                  className="split-pct-input"
                  min="0"
                  max="100"
                  value={te}
                  readOnly
                />
                %
              </div>
              <span id="countTest" style={{ fontSize: '11px', color: 'var(--txt-muted)' }}>
                {countTest} images
              </span>
            </div>
          </div>

          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              paddingTop: '4px',
            }}
          >
            <label style={{ fontSize: '12px', display: 'flex', alignItems: 'center', gap: '6px' }}>
              <input
                type="checkbox"
                id="chkSplitLabeledOnly"
                checked={labeledOnly}
                onChange={e => setLabeledOnly(e.target.checked)}
              />
              Partition reviewed / labeled samples only
            </label>
            <span id="countSplitTotal" style={{ fontSize: '11.5px', color: 'var(--txt-muted)' }}>
              {n} total images
            </span>
          </div>
        </div>

        <div className="modal-footer">
          <button id="btnCancelSplitter" onClick={onClose}>
            Cancel
          </button>
          <button
            id="btnCommitSplit"
            className="btn-purple"
            disabled={splitMutation.isPending}
            onClick={handleCommit}
          >
            {splitMutation.isPending ? 'Partitioning...' : 'Commit Partition to manifest.json'}
          </button>
        </div>
      </div>
    </div>
  );
};
