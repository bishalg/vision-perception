// apps/annotation_studio/src/main.ts — CurvStudio Roboflow Clone Main Entry (TypeScript)
import { state } from './state';
import { initCanvas, resetView, redraw, zoomIn, zoomOut } from './canvas';
import { initTools, setTool, showToast, performUndo, performRedo } from './tools';
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

    renderProjectTabs();

    if (state.domains.length > 0) {
      await switchDomain(state.domains[0].id);
    }
  } catch (err: any) {
    showToast('Could not connect to studio server: ' + err.message);
  }
}

function renderProjectTabs(): void {
  const tabsContainer = document.getElementById('projectTabs');
  if (!tabsContainer) return;
  tabsContainer.innerHTML = state.domains.map(d => {
    const isActive = d.id === state.currentDomain?.id;
    const icon = d.id === 'flood' ? '🌊' : '🌿';
    return `
      <button class="project-tab ${isActive ? 'active' : ''}" data-domain="${d.id}" title="${d.label}">
        <span>${icon}</span>
        <span>${d.label}</span>
      </button>
    `;
  }).join('');

  tabsContainer.querySelectorAll<HTMLElement>('.project-tab').forEach(tab => {
    tab.onclick = () => {
      const domId = tab.dataset.domain!;
      if (domId !== state.currentDomain?.id) {
        switchDomain(domId);
      }
    };
  });
}

export async function switchDomain(domainId: string): Promise<void> {
  state.currentDomain = state.domains.find(d => d.id === domainId) || null;
  if (!state.currentDomain) return;

  const sel = document.getElementById('domainSelect') as HTMLSelectElement | null;
  if (sel) sel.value = domainId;
  renderProjectTabs();

  if (state.currentDomain.classes && state.currentDomain.classes.length > 0) {
    state.activeClassId = state.currentDomain.classes[0].id;
  }

  try {
    const res = await fetch(`/api/samples?domain=${domainId}`);
    const data = await res.json();
    state.samples = data.samples || [];

    renderLabelsTab();
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

  const pagerLbl = document.getElementById('samplePagerLabel');
  if (pagerLbl) {
    pagerLbl.textContent = `${state.currentSampleIdx + 1} / ${state.samples.length}`;
  }

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

  // Bottom floating controls bar
  const btnZoomIn = document.getElementById('btnZoomIn');
  const btnZoomOut = document.getElementById('btnZoomOut');
  if (btnZoomIn) btnZoomIn.onclick = () => zoomIn();
  if (btnZoomOut) btnZoomOut.onclick = () => zoomOut();

  const btnTheme = document.getElementById('btnToggleTheme');
  if (btnTheme) {
    let contrastMode = false;
    btnTheme.onclick = () => {
      contrastMode = !contrastMode;
      const cvs = document.getElementById('studioCanvas');
      if (cvs) cvs.style.filter = contrastMode ? 'contrast(1.35) brightness(1.2)' : 'none';
      btnTheme.textContent = contrastMode ? '🌙' : '☀️';
      showToast(contrastMode ? 'High-contrast canvas enabled' : 'Normal canvas view');
    };
  }

  const btnShortcuts = document.getElementById('btnOpenShortcuts');
  const modalShortcuts = document.getElementById('shortcutsModal');
  const btnCloseShortcuts = document.getElementById('btnCloseShortcuts');
  if (btnShortcuts && modalShortcuts) {
    btnShortcuts.onclick = () => modalShortcuts.classList.add('show');
  }
  if (btnCloseShortcuts && modalShortcuts) {
    btnCloseShortcuts.onclick = () => modalShortcuts.classList.remove('show');
  }
  if (modalShortcuts) {
    modalShortcuts.onclick = (e) => {
      if (e.target === modalShortcuts) modalShortcuts.classList.remove('show');
    };
  }
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

    // Undo: Ctrl+Z or Cmd+Z
    if ((e.metaKey || e.ctrlKey) && !e.shiftKey && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      performUndo();
      return;
    }

    // Redo: Ctrl+Shift+Z, Cmd+Shift+Z, Ctrl+Y, Cmd+Y
    if (((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === 'z') ||
        ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'y')) {
      e.preventDefault();
      performRedo();
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

    if (e.key === '?' || (e.shiftKey && e.key === '/')) {
      document.getElementById('shortcutsModal')?.classList.toggle('show');
      return;
    }
    if (e.key === '0') {
      resetView();
      return;
    }
    if (e.key === '=' || e.key === '+') {
      zoomIn();
      return;
    }
    if (e.key === '-' || e.key === '_') {
      zoomOut();
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
