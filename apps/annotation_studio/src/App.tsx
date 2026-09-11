// apps/annotation_studio/src/App.tsx
// Root Headless React Component for CurvStudio — Roboflow-grade Annotation Studio.
import React, { useState, useEffect, useCallback, useRef } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useStore } from '@tanstack/react-store';
import {
  studioStore,
  setTool,
  selectClass,
  performUndo,
  performRedo,
  toggleAbsentClass,
  addAuditLog,
} from './core/store';
import type { SampleItem, Point2D } from './core/types';
import {
  useDomainsQuery,
  useSamplesQuery,
  useSaveTruthMutation,
} from './api/queries';

import { StudioHeader } from './components/Header/StudioHeader';
import { SubHeader } from './components/Header/SubHeader';
import { TwoTierSidebar } from './components/Sidebar/TwoTierSidebar';
import { CanvasViewport } from './components/Viewport/CanvasViewport';
import { DatasetSplitterModal } from './components/Modals/DatasetSplitterModal';
import { TestBenchModal } from './components/Modals/TestBenchModal';
import { ShortcutsModal } from './components/Modals/ShortcutsModal';
import { Toast } from './components/Common/Toast';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function loadSampleIntoStore(sample: SampleItem): void {
  const state = studioStore.state;
  const truth = sample.truth_data || {};
  const polylines: Record<string, Point2D[][]> = {};
  const polygons: Record<string, Point2D[][]> = {};
  const bboxes: Record<string, [number, number, number, number][]> = {};
  const absentClasses: string[] = [];

  if (state.currentDomain?.classes) {
    for (const cls of state.currentDomain.classes) {
      const val = truth[cls.id] || truth[cls.id.replace('_line', '')];
      if (val && typeof val === 'object' && 'absent' in val && val.absent) {
        absentClasses.push(cls.id);
        continue;
      }
      if (Array.isArray(val) && val.length > 0) {
        if (Array.isArray(val[0]) && typeof val[0][0] === 'number') {
          polylines[cls.id] = [val as Point2D[]];
        } else {
          polylines[cls.id] = val as Point2D[][];
        }
      } else {
        polylines[cls.id] = [];
      }
    }
  }

  const brushStrokes = Array.isArray(truth.brush_strokes) ? truth.brush_strokes : [];

  studioStore.setState(prev => ({
    ...prev,
    currentSample: sample,
    currentDraft: [],
    bboxStart: null,
    currentCursor: null,
    undoStack: [],
    redoStack: [],
    polylines,
    polygons,
    bboxes,
    brushStrokes,
    absentClasses,
    aiLines: sample.auto_lines || {},
  }));
}

const StudioApp: React.FC = () => {
  const currentDomain = useStore(studioStore, s => s.currentDomain);
  const currentSampleIdx = useStore(studioStore, s => s.currentSampleIdx);
  const samples = useStore(studioStore, s => s.samples);
  const activeClassId = useStore(studioStore, s => s.activeClassId);

  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const toastTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [isSplitterOpen, setIsSplitterOpen] = useState(false);
  const [isTestBenchOpen, setIsTestBenchOpen] = useState(false);
  const [isShortcutsOpen, setIsShortcutsOpen] = useState(false);

  const showToast = useCallback((msg: string) => {
    if (toastTimeoutRef.current) clearTimeout(toastTimeoutRef.current);
    setToastMsg(msg);
    toastTimeoutRef.current = setTimeout(() => {
      setToastMsg(null);
    }, 3000);
  }, []);

  // 1. Fetch Domains
  const { data: domains = [] } = useDomainsQuery();

  // Initialize domain when domains query resolves
  useEffect(() => {
    if (domains.length > 0 && !currentDomain) {
      const initialDomain = domains[0];
      studioStore.setState(prev => ({
        ...prev,
        domains,
        currentDomain: initialDomain,
        activeClassId: initialDomain.classes[0]?.id || '',
      }));
    }
  }, [domains, currentDomain]);

  // 2. Fetch Samples for current domain
  const { data: samplesResponse } = useSamplesQuery(currentDomain?.id || null);

  useEffect(() => {
    if (samplesResponse?.samples && samplesResponse.samples.length > 0) {
      const sList = samplesResponse.samples;
      studioStore.setState(prev => ({
        ...prev,
        samples: sList,
        currentSampleIdx: 0,
      }));
      loadSampleIntoStore(sList[0]);
    }
  }, [samplesResponse]);

  // 3. Select Sample Navigation
  const handleSelectSample = useCallback((idx: number) => {
    if (idx < 0 || idx >= samples.length) return;
    studioStore.setState(prev => ({ ...prev, currentSampleIdx: idx }));
    loadSampleIntoStore(samples[idx]);
  }, [samples]);

  const handlePrevSample = useCallback(() => {
    if (samples.length === 0) return;
    const nextIdx = Math.max(0, currentSampleIdx - 1);
    handleSelectSample(nextIdx);
  }, [samples.length, currentSampleIdx, handleSelectSample]);

  const handleNextSample = useCallback(() => {
    if (samples.length === 0) return;
    const nextIdx = Math.min(samples.length - 1, currentSampleIdx + 1);
    handleSelectSample(nextIdx);
  }, [samples.length, currentSampleIdx, handleSelectSample]);

  // 4. Domain Selection
  const handleSelectDomain = useCallback((domainId: string) => {
    const d = domains.find(x => x.id === domainId);
    if (!d || d.id === currentDomain?.id) return;
    studioStore.setState(prev => ({
      ...prev,
      currentDomain: d,
      activeClassId: d.classes[0]?.id || '',
      currentSampleIdx: 0,
      samples: [],
      currentSample: null,
    }));
    addAuditLog('Domain Switch', `Switched domain to ${d.label}`);
  }, [domains, currentDomain?.id]);

  // 5. Save Ground Truth Mutation
  const saveTruthMutation = useSaveTruthMutation();

  const handleSaveTruth = useCallback(async () => {
    const state = studioStore.state;
    if (!state.currentSample || !state.currentDomain) return;

    const data: Record<string, any> = {};
    for (const cls of state.currentDomain.classes) {
      if (state.absentClasses.includes(cls.id)) {
        data[cls.id] = { absent: true };
      } else {
        data[cls.id] = state.polylines[cls.id] || [];
      }
    }
    if (state.brushStrokes.length > 0) {
      data.brush_strokes = state.brushStrokes;
    }

    try {
      const res = await saveTruthMutation.mutateAsync({
        domain: state.currentDomain.id,
        stem: state.currentSample.stem,
        target_file: state.currentSample.truth_file,
        data,
        auto_lines: state.aiLines,
      });

      if (res.ok) {
        studioStore.setState(prev => {
          const updated = prev.samples.map(s =>
            s.stem === prev.currentSample?.stem
              ? { ...s, has_truth: true, truth_data: data, metrics: res.metrics || s.metrics }
              : s
          );
          const cur = updated.find(s => s.stem === prev.currentSample?.stem) || null;
          return {
            ...prev,
            samples: updated,
            currentSample: cur,
          };
        });
        showToast(`Saved ground truth to ${res.saved || 'manifest'}`);
        addAuditLog('Save Ground Truth', `Committed ground truth for ${state.currentSample.filename}`);
      } else {
        showToast(`Save failed: ${res.error || 'unknown'}`);
      }
    } catch (err: any) {
      showToast(`Network error saving: ${err.message}`);
    }
  }, [saveTruthMutation, showToast]);

  // 6. Global Keyboard Shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName?.toLowerCase();
      if (tag === 'input' || tag === 'textarea' || tag === 'select') return;

      const isCmdOrCtrl = e.metaKey || e.ctrlKey;

      if (isCmdOrCtrl && e.key.toLowerCase() === 's') {
        e.preventDefault();
        handleSaveTruth();
      } else if (isCmdOrCtrl && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        if (e.shiftKey) {
          performRedo();
        } else {
          performUndo();
        }
      } else if (isCmdOrCtrl && e.key.toLowerCase() === 'y') {
        e.preventDefault();
        performRedo();
      } else if (e.key === '[') {
        e.preventDefault();
        handlePrevSample();
      } else if (e.key === ']') {
        e.preventDefault();
        handleNextSample();
      } else if (e.key.toLowerCase() === 'h' || e.code === 'Space') {
        e.preventDefault();
        setTool('pan');
      } else if (e.key.toLowerCase() === 'l') {
        setTool('polyline');
      } else if (e.key.toLowerCase() === 'p') {
        setTool('polygon');
      } else if (e.key.toLowerCase() === 'b') {
        setTool('brush');
      } else if (e.key.toLowerCase() === 'r') {
        setTool('bbox');
      } else if (e.key.toLowerCase() === 's') {
        setTool('wand');
      } else if (e.key.toLowerCase() === 'a') {
        if (activeClassId) toggleAbsentClass(activeClassId);
      } else if (e.key === '?') {
        setIsShortcutsOpen(prev => !prev);
      } else if (e.key >= '1' && e.key <= '9') {
        const idx = parseInt(e.key, 10) - 1;
        const cls = currentDomain?.classes[idx];
        if (cls) selectClass(cls.id);
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [
    handleSaveTruth,
    handlePrevSample,
    handleNextSample,
    activeClassId,
    currentDomain?.classes,
  ]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', width: '100vw', overflow: 'hidden' }}>
      <StudioHeader
        domains={domains}
        onSelectDomain={handleSelectDomain}
        onPrevSample={handlePrevSample}
        onNextSample={handleNextSample}
        onOpenSplitter={() => setIsSplitterOpen(true)}
        onOpenTestBench={() => setIsTestBenchOpen(true)}
        onSaveTruth={handleSaveTruth}
        isSaving={saveTruthMutation.isPending}
      />

      <SubHeader />

      <div className="app-body">
        <TwoTierSidebar />
        <CanvasViewport
          onOpenShortcuts={() => setIsShortcutsOpen(true)}
          onSelectSample={handleSelectSample}
        />
      </div>

      <DatasetSplitterModal
        isOpen={isSplitterOpen}
        onClose={() => setIsSplitterOpen(false)}
        onToast={showToast}
      />

      <TestBenchModal
        isOpen={isTestBenchOpen}
        onClose={() => setIsTestBenchOpen(false)}
        onToast={showToast}
      />

      <ShortcutsModal
        isOpen={isShortcutsOpen}
        onClose={() => setIsShortcutsOpen(false)}
      />

      <Toast message={toastMsg} />
    </div>
  );
};

export const App: React.FC = () => {
  return (
    <QueryClientProvider client={queryClient}>
      <StudioApp />
    </QueryClientProvider>
  );
};
