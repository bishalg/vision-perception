// apps/annotation_studio/src/main.ts — CurvStudio Roboflow Clone Main Entry (TypeScript)
import { state } from './state';
import { initCanvas, resetView, redraw } from './canvas';
import { initTools, setTool, showToast } from './tools';
import { initSidebar, renderLabelsTab, selectClass } from './sidebar';
import { initModals } from './modals';
import type { ToolType, DomainConfig } from './types';

export async function bootstrapStudio(): Promise<void> {
  const canvasEl = document.getElementById('studioCanvas') as HTMLCanvasElement | null;
  const containerEl = document.getElementById('canvasContainer');

  if (!canvasEl || !containerEl) return;

  initCanvas(canvasEl, containerEl);
  initTools();
  initSidebar();
  initModals();

  setupTopControls();
  setupKeyboardShortcuts();

  state.on('samples_updated', () => {
    renderGallery();
    updateSampleSelect();
  });

  await loadDomainRegistry();
}

async function loadDomainRegistry(): Promise<void> {
  try {
    const res = await fetch('/api/domains');
    const data: { domains?: DomainConfig[] } = await res.json();
    state.domains = data.domains || [];

    const sel = document.getElementById('domainSelect') as HTMLSelectElement | null;
    if (sel) {
      sel.innerHTML = state.domains.map(d => `<option value="${d.id}">${d.label}</option>`).join('');
      sel.onchange = e => switchDomain((e.target as HTMLSelectElement).value);
    }

    if (state.domains.length > 0) {
      await switchDomain(state.domains[0].id);
    }
  } catch (err: any) {
    showToast('Could not connect to studio server: ' + err.message);
  }
}

export async function switchDomain(domainId: string): Promise<void> {
  state.currentDomain = state.domains.find(d => d.id === domainId) || null;
  if (!state.currentDomain) return;

  const sel = document.getElementById('domainSelect') as HTMLSelectElement | null;
  if (sel) sel.value = domainId;

  if (state.currentDomain.classes && state.currentDomain.classes.length > 0) {
    state.activeClassId = state.currentDomain.classes[0].id;
  }

  try {
    const res = await fetch(`/api/samples?domain=${domainId}`);
    const data = await res.json();
    state.samples = data.samples || [];

    renderLabelsTab();
    renderGallery();
    updateSampleSelect();

    if (state.samples.length > 0) {
      await selectSample(0);
    } else {
      state.image = null;
      redraw();
    }
    state.recordAudit('Domain Switch', `Switched to domain: ${state.currentDomain.label}`);
  } catch (err: any) {
    showToast('Failed loading domain samples: ' + err.message);
  }
}

export async function selectSample(idx: number): Promise<void> {
  if (idx < 0 || idx >= state.samples.length) return;
  state.currentSampleIdx = idx;
  state.currentSample = state.samples[idx];
  state.currentDraft = [];
  state.undoStack = [];
  state.redoStack = [];

  updateSampleSelect();
  updateGallerySelection();

  const img = new Image();
  img.src = state.currentSample.image_url;
  await new Promise(r => { img.onload = r; img.onerror = r; });
  state.image = img;

  state.polylines = {};
  state.polygons = {};
  state.bboxes = {};
  state.absentClasses = [];

  const truth = state.currentSample.truth_data || {};
  if (state.currentDomain?.classes) {
    for (const cls of state.currentDomain.classes) {
      let val = truth[cls.id] || truth[cls.id.replace('_line', '')];
      if (val && val.absent) {
        state.absentClasses.push(cls.id);
        continue;
      }
      if (Array.isArray(val) && val.length > 0) {
        if (Array.isArray(val[0]) && typeof val[0][0] === 'number') {
          state.polylines[cls.id] = [val];
        } else {
          state.polylines[cls.id] = val;
        }
      } else {
        state.polylines[cls.id] = [];
      }
    }
  }

  state.aiLines = state.currentSample.auto_lines || {};

  resetView();
  state.emit('state_changed');
  redraw();
}

function updateSampleSelect(): void {
  const sel = document.getElementById('sampleSelect') as HTMLSelectElement | null;
  if (!sel) return;
  sel.innerHTML = state.samples.map((s, idx) => `
    <option value="${idx}">[${s.split ? s.split.toUpperCase() : 'RAW'}] ${s.filename} ${s.has_truth ? '✓' : '—'}</option>
  `).join('');
  sel.value = state.currentSampleIdx.toString();
}

function renderGallery(): void {
  const gal = document.getElementById('bottomGallery');
  if (!gal) return;

  gal.innerHTML = state.samples.map((s, idx) => `
    <div class="thumb-card ${idx === state.currentSampleIdx ? 'active' : ''}" data-idx="${idx}">
      <span class="status-pill ${s.has_truth ? 'reviewed' : 'needs_label'}">
        ${s.has_truth ? 'Reviewed' : 'Needs Label'}
      </span>
      ${s.split && s.split !== 'unassigned' ? `<span class="split-pill ${s.split}">${s.split}</span>` : ''}
      <img src="${s.image_url}" alt="${s.stem}">
      <span class="thumb-name">${s.filename}</span>
    </div>
  `).join('');

  gal.querySelectorAll<HTMLElement>('.thumb-card').forEach(card => {
    card.onclick = () => selectSample(Number(card.dataset.idx));
  });
}

function updateGallerySelection(): void {
  document.querySelectorAll<HTMLElement>('.thumb-card').forEach(c => {
    c.classList.toggle('active', Number(c.dataset.idx) === state.currentSampleIdx);
  });
}

async function saveGroundTruth(): Promise<void> {
  if (!state.currentSample || !state.currentDomain) return;

  const data: Record<string, any> = {};
  for (const cls of state.currentDomain.classes) {
    if (state.absentClasses.includes(cls.id)) {
      data[cls.id] = { absent: true };
    } else {
      data[cls.id] = state.polylines[cls.id] || [];
    }
  }

  const payload = {
    domain: state.currentDomain.id,
    stem: state.currentSample.stem,
    target_file: state.currentSample.truth_file,
    data: data,
    auto_lines: state.aiLines,
  };

  try {
    const res = await fetch('/api/save_truth', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    const result = await res.json();
    if (result.ok) {
      state.currentSample.has_truth = true;
      state.currentSample.truth_data = data;
      if (result.metrics) {
        state.currentSample.metrics = result.metrics;
      }
      showToast(`Saved ground truth to ${result.saved}`);
      state.recordAudit('Save Ground Truth', `Committed ground truth for ${state.currentSample.filename}`);
      renderGallery();
      state.emit('state_changed');
    } else {
      showToast('Save failed: ' + (result.error || 'unknown'));
    }
  } catch (err: any) {
    showToast('Network error saving: ' + err.message);
  }
}

function setupTopControls(): void {
  const btnPrev = document.getElementById('btnPrevSample');
  const btnNext = document.getElementById('btnNextSample');
  const btnReset = document.getElementById('btnResetView');
  const btnSave = document.getElementById('btnSaveTruth');
  const selSample = document.getElementById('sampleSelect') as HTMLSelectElement | null;

  if (btnPrev) btnPrev.onclick = () => selectSample(state.currentSampleIdx - 1);
  if (btnNext) btnNext.onclick = () => selectSample(state.currentSampleIdx + 1);
  if (btnReset) btnReset.onclick = resetView;
  if (btnSave) btnSave.onclick = saveGroundTruth;
  if (selSample) selSample.onchange = e => selectSample(Number((e.target as HTMLSelectElement).value));
}

function setupKeyboardShortcuts(): void {
  window.addEventListener('keydown', (e: KeyboardEvent) => {
    const target = e.target as HTMLElement;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT')) {
      if (e.key === 'Escape') target.blur();
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      saveGroundTruth();
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      if (e.shiftKey) {
        state.redo();
      } else {
        state.undo();
      }
      redraw();
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'y') {
      e.preventDefault();
      state.redo();
      redraw();
      return;
    }

    if (e.key === '[') {
      selectSample(state.currentSampleIdx - 1);
      return;
    }
    if (e.key === ']') {
      selectSample(state.currentSampleIdx + 1);
      return;
    }

    // Tool hotkeys
    if (e.key.toLowerCase() === 'h') setTool('pan');
    else if (e.key.toLowerCase() === 'l') setTool('polyline');
    else if (e.key.toLowerCase() === 'p') setTool('polygon');
    else if (e.key.toLowerCase() === 'b') setTool('brush');
    else if (e.key.toLowerCase() === 'r') setTool('bbox');
    else if (e.key.toLowerCase() === 's' && !e.metaKey && !e.ctrlKey) setTool('wand');
    else if (e.key.toLowerCase() === 'a' && !e.metaKey && !e.ctrlKey) {
      document.getElementById('toolAbsent')?.click();
    }

    // Class selection (1..4)
    if (['1', '2', '3', '4'].includes(e.key)) {
      const idx = Number(e.key) - 1;
      if (state.currentDomain?.classes?.[idx]) {
        selectClass(state.currentDomain.classes[idx].id);
        renderLabelsTab();
      }
    }
  });
}

document.addEventListener('DOMContentLoaded', bootstrapStudio);
