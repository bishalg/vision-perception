// apps/annotation_studio/src/components/Modals/TestBenchModal.tsx
import React, { useState, useRef } from 'react';
import { useStore } from '@tanstack/react-store';
import { studioStore, addAuditLog } from '../../core/store';
import { useInferenceMutation } from '../../api/queries';
import type { InferencePrediction, InferencePayload } from '../../api/client';

interface TestBenchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onToast: (msg: string) => void;
}

export const TestBenchModal: React.FC<TestBenchModalProps> = ({
  isOpen,
  onClose,
  onToast,
}) => {
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const currentSample = useStore(studioStore, s => s.currentSample);

  const [imageSource, setImageSource] = useState<'current' | 'upload'>('current');
  const [uploadedBase64, setUploadedBase64] = useState<string | null>(null);
  const [uploadedName, setUploadedName] = useState<string>('');
  const [confidence, setConfidence] = useState(0.5);
  const [statusText, setStatusText] = useState('Ready to run inference.');
  const [predictions, setPredictions] = useState<InferencePrediction[]>([]);
  const [overlayUrl, setOverlayUrl] = useState<string | null>(null);

  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const inferMutation = useInferenceMutation();

  if (!isOpen) return null;

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = ev => {
        setUploadedBase64(ev.target?.result as string);
        setUploadedName(file.name);
        onToast(`Loaded ${file.name} for inference test`);
      };
      reader.readAsDataURL(file);
    }
  };

  const handleRun = async () => {
    setStatusText('Executing C++ CurvEngine pipeline...');
    try {
      const payload: InferencePayload = {
        domain: currentDomain?.id || 'palm',
        confidence_threshold: confidence,
        image_base64: imageSource === 'upload' && uploadedBase64 ? uploadedBase64 : undefined,
        image_url: imageSource === 'current' ? currentSample?.image_url : undefined,
      };

      const result = await inferMutation.mutateAsync(payload);
      if (result.ok) {
        setStatusText(
          `Completed in ${result.duration_ms} ms (${result.predictions?.length || 0} detections)`
        );
        setPredictions(result.predictions || []);
        if (result.overlay_url) {
          setOverlayUrl(`${result.overlay_url}?t=${Date.now()}`);
        }
        addAuditLog(
          'Model Inference',
          `Evaluated ${currentSample?.stem || 'uploaded'} (${result.duration_ms} ms)`
        );
      } else {
        setStatusText('Error: ' + (result.error || 'Execution failed'));
      }
    } catch (err: any) {
      setStatusText('Network error: ' + err.message);
    }
  };

  return (
    <div
      className="modal-backdrop show"
      id="testBenchModal"
      onClick={onClose}
    >
      <div
        className="modal-window"
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <span className="modal-title">Live Model Test Bench & Inference Playground</span>
          <button className="modal-close" id="btnCloseTestBench" onClick={onClose}>
            ✕
          </button>
        </div>

        <div className="modal-body">
          <div style={{ fontSize: '12.5px', color: 'var(--txt-muted)' }}>
            Execute the native C++ CurvEngine pipeline on-the-fly and inspect predicted contours and confidence scores.
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <label style={{ fontSize: '12px', fontWeight: 600 }}>Image Source</label>
            <select
              id="inferSourceSelect"
              value={imageSource}
              onChange={e => {
                const val = e.target.value as 'current' | 'upload';
                setImageSource(val);
                if (val === 'upload' && !uploadedBase64) {
                  fileInputRef.current?.click();
                }
              }}
            >
              <option value="current">Current Canvas Sample ({currentSample?.filename || 'None'})</option>
              <option value="upload">
                Upload Custom Image… {uploadedName ? `(${uploadedName})` : ''}
              </option>
            </select>
            <input
              ref={fileInputRef}
              type="file"
              id="inferFileInput"
              accept="image/*"
              style={{ display: 'none' }}
              onChange={handleFileChange}
            />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', fontWeight: 600 }}>
              <span>Confidence Threshold</span>
              <span id="lblInferConfidence">{Math.round(confidence * 100)}%</span>
            </div>
            <input
              type="range"
              id="rngInferConfidence"
              min="0"
              max="1"
              step="0.05"
              value={confidence}
              style={{ accentColor: 'var(--blue)' }}
              onChange={e => setConfidence(parseFloat(e.target.value))}
            />
          </div>

          <div
            id="inferStatusBox"
            style={{
              fontSize: '11.5px',
              color: 'var(--txt-muted)',
              padding: '8px 10px',
              background: 'var(--surface-2)',
              borderRadius: '6px',
              border: '1px solid var(--border)',
            }}
          >
            {statusText}
          </div>

          <div
            id="inferResultsList"
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '6px',
              maxHeight: '180px',
              overflowY: 'auto',
            }}
          >
            {predictions.length > 0 ? (
              predictions.map((p, i) => (
                <div
                  key={i}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '8px 10px',
                    background: 'var(--surface-2)',
                    border: '1px solid var(--border)',
                    borderRadius: '6px',
                  }}
                >
                  <span style={{ fontWeight: 600, fontSize: '12.5px' }}>{p.class_id}</span>
                  <span
                    style={{
                      fontSize: '11px',
                      background: 'rgba(16,185,129,0.15)',
                      color: 'var(--green)',
                      border: '1px solid var(--green)',
                      padding: '2px 8px',
                      borderRadius: '999px',
                      fontWeight: 700,
                    }}
                  >
                    {Math.round(p.confidence * 100)}% Confidence
                  </span>
                </div>
              ))
            ) : (
              <div style={{ color: 'var(--txt-muted)', fontSize: '12px' }}>
                No features evaluated yet.
              </div>
            )}
          </div>

          {overlayUrl && (
            <img
              id="inferOverlayPreview"
              src={overlayUrl}
              alt="Overlay preview"
              style={{
                width: '100%',
                maxHeight: '180px',
                objectFit: 'contain',
                background: '#000',
                borderRadius: '6px',
                border: '1px solid var(--border)',
              }}
            />
          )}
        </div>

        <div className="modal-footer">
          <button
            id="btnRunInference"
            className="btn-primary"
            style={{ width: '100%', justifyContent: 'center' }}
            disabled={inferMutation.isPending}
            onClick={handleRun}
          >
            {inferMutation.isPending ? 'Running Pipeline...' : '▶ Run CurvEngine Inference'}
          </button>
        </div>
      </div>
    </div>
  );
};
