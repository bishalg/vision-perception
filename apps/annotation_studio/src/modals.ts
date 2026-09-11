// apps/annotation_studio/src/modals.ts — Dataset Splitter & Live Model Test Bench (TypeScript)
import { state } from './state';
import { showToast } from './tools';
import type { DatasetSplitResponse, InferenceResponse } from './types';

export function initModals(): void {
  setupSplitterModal();
  setupTestBenchModal();
}

function setupSplitterModal(): void {
  const btnOpen = document.getElementById('btnOpenSplitter');
  const modal = document.getElementById('datasetSplitModal');
  const btnClose = document.getElementById('btnCloseSplitter');
  const btnCancel = document.getElementById('btnCancelSplitter');
  const btnCommit = document.getElementById('btnCommitSplit') as HTMLButtonElement | null;

  const inpTrain = document.getElementById('splitTrainPct') as HTMLInputElement | null;
  const inpValid = document.getElementById('splitValidPct') as HTMLInputElement | null;
  const inpTest = document.getElementById('splitTestPct') as HTMLInputElement | null;
  const chkLabeledOnly = document.getElementById('chkSplitLabeledOnly') as HTMLInputElement | null;

  if (btnOpen && modal) btnOpen.onclick = openSplitterModal;
  if (btnClose && modal) btnClose.onclick = () => modal.classList.remove('show');
  if (btnCancel && modal) btnCancel.onclick = () => modal.classList.remove('show');

  function updateSplitVisuals(): void {
    if (!inpTrain || !inpValid || !inpTest) return;

    let tr = parseInt(inpTrain.value || '70', 10);
    let va = parseInt(inpValid.value || '20', 10);
    if (tr + va > 100) va = 100 - tr;
    let te = 100 - tr - va;
    if (te < 0) te = 0;

    inpTrain.value = tr.toString();
    inpValid.value = va.toString();
    inpTest.value = te.toString();

    const segTr = document.getElementById('segTrain');
    const segVa = document.getElementById('segValid');
    const segTe = document.getElementById('segTest');

    if (segTr) { segTr.style.width = `${tr}%`; segTr.textContent = `${tr}% Train`; }
    if (segVa) { segVa.style.width = `${va}%`; segVa.textContent = `${va}% Valid`; }
    if (segTe) { segTe.style.width = `${te}%`; segTe.textContent = `${te}% Test`; }

    const labeledOnly = chkLabeledOnly?.checked;
    const targetSamples = labeledOnly ? state.samples.filter(s => s.has_truth) : state.samples;
    const n = targetSamples.length;

    const cTr = Math.round((tr / 100) * n);
    const cVa = Math.round((va / 100) * n);
    const cTe = Math.max(0, n - cTr - cVa);

    const cntTr = document.getElementById('countTrain');
    const cntVa = document.getElementById('countValid');
    const cntTe = document.getElementById('countTest');
    const cntTotal = document.getElementById('countSplitTotal');

    if (cntTr) cntTr.textContent = `${cTr} images`;
    if (cntVa) cntVa.textContent = `${cVa} images`;
    if (cntTe) cntTe.textContent = `${cTe} images`;
    if (cntTotal) cntTotal.textContent = `${n} total images`;
  }

  if (inpTrain) inpTrain.oninput = updateSplitVisuals;
  if (inpValid) inpValid.oninput = updateSplitVisuals;
  if (chkLabeledOnly) chkLabeledOnly.onchange = updateSplitVisuals;

  if (btnCommit) {
    btnCommit.onclick = async () => {
      if (!state.currentDomain || !inpTrain || !inpValid || !inpTest) return;
      btnCommit.disabled = true;
      btnCommit.textContent = 'Partitioning...';
      try {
        const payload = {
          domain: state.currentDomain.id,
          split: {
            train: parseInt(inpTrain.value, 10),
            valid: parseInt(inpValid.value, 10),
            test: parseInt(inpTest.value, 10),
          },
          labeled_only: chkLabeledOnly?.checked || false,
        };

        const res = await fetch('/api/dataset_split', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
        const result: DatasetSplitResponse = await res.json();
        if (result.ok) {
          showToast(`Partition saved to ${result.manifest_file}`);
          for (const s of result.samples) {
            const found = state.samples.find(x => x.stem === s.stem);
            if (found) found.split = s.split;
          }
          if (modal) modal.classList.remove('show');
          state.emit('samples_updated');
          state.recordAudit('Dataset Split', `Committed split: ${payload.split.train}/${payload.split.valid}/${payload.split.test}`);
        } else {
          showToast('Split failed');
        }
      } catch (err: any) {
        showToast('Error: ' + err.message);
      } finally {
        btnCommit.disabled = false;
        btnCommit.textContent = 'Commit Partition to manifest.json';
      }
    };
  }
}

function openSplitterModal(): void {
  const modal = document.getElementById('datasetSplitModal');
  if (!modal) return;
  modal.classList.add('show');
  const inpTrain = document.getElementById('splitTrainPct');
  if (inpTrain) inpTrain.dispatchEvent(new Event('input'));
}

function setupTestBenchModal(): void {
  const btnOpen = document.getElementById('btnOpenTestBench');
  const modal = document.getElementById('testBenchModal');
  const btnClose = document.getElementById('btnCloseTestBench');
  const btnRun = document.getElementById('btnRunInference') as HTMLButtonElement | null;
  const selSource = document.getElementById('inferSourceSelect') as HTMLSelectElement | null;
  const fileInput = document.getElementById('inferFileInput') as HTMLInputElement | null;
  const rngConf = document.getElementById('rngInferConfidence') as HTMLInputElement | null;
  const lblConf = document.getElementById('lblInferConfidence');

  if (btnOpen && modal) btnOpen.onclick = openTestBenchModal;
  if (btnClose && modal) btnClose.onclick = () => modal.classList.remove('show');

  if (rngConf && lblConf) {
    rngConf.oninput = e => {
      lblConf.textContent = `${Math.round(parseFloat((e.target as HTMLInputElement).value) * 100)}%`;
    };
  }

  let uploadedBase64: string | null = null;
  if (fileInput) {
    fileInput.onchange = e => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        const reader = new FileReader();
        reader.onload = ev => {
          uploadedBase64 = ev.target?.result as string;
          showToast(`Loaded ${file.name} for inference test`);
        };
        reader.readAsDataURL(file);
      }
    };
  }

  if (btnRun) {
    btnRun.onclick = async () => {
      btnRun.disabled = true;
      btnRun.textContent = 'Running Pipeline...';
      const statusBox = document.getElementById('inferStatusBox');
      const resultsContainer = document.getElementById('inferResultsList');
      if (statusBox) statusBox.textContent = 'Executing C++ CurvEngine pipeline...';

      try {
        const payload: Record<string, any> = {
          domain: state.currentDomain?.id || 'palm',
          confidence_threshold: parseFloat(rngConf?.value || '0.5'),
        };

        if (selSource?.value === 'upload' && uploadedBase64) {
          payload.image_base64 = uploadedBase64;
        } else {
          payload.image_url = state.currentSample?.image_url;
        }

        const res = await fetch('/api/run_inference', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
        const result: InferenceResponse = await res.json();

        if (result.ok) {
          if (statusBox) statusBox.textContent = `Completed in ${result.duration_ms} ms (${result.predictions.length} detections)`;

          if (resultsContainer) {
            resultsContainer.innerHTML = result.predictions.map(p => `
              <div style="display:flex;align-items:center;justify-content:space-between;padding:8px 10px;background:var(--surface-2);border:1px solid var(--border);border-radius:6px">
                <span style="font-weight:600;font-size:12.5px">${p.class_id}</span>
                <span style="font-size:11px;background:rgba(16,185,129,0.15);color:var(--green);border:1px solid var(--green);padding:2px 8px;border-radius:999px;font-weight:700">
                  ${Math.round(p.confidence * 100)}% Confidence
                </span>
              </div>
            `).join('') || '<div style="color:var(--txt-muted);font-size:12px">No features exceeded confidence threshold.</div>';
          }

          const overlayImg = document.getElementById('inferOverlayPreview') as HTMLImageElement | null;
          if (overlayImg && result.overlay_url) {
            overlayImg.src = `${result.overlay_url}?t=${Date.now()}`;
            overlayImg.style.display = 'block';
          }

          state.recordAudit('Model Inference', `Evaluated ${state.currentSample?.stem} (${result.duration_ms} ms)`);
        } else {
          if (statusBox) statusBox.textContent = 'Error: ' + (result.error || 'Execution failed');
        }
      } catch (err: any) {
        if (statusBox) statusBox.textContent = 'Network error: ' + err.message;
      } finally {
        btnRun.disabled = false;
        btnRun.textContent = '▶ Run CurvEngine Inference';
      }
    };
  }
}

function openTestBenchModal(): void {
  const modal = document.getElementById('testBenchModal');
  if (!modal) return;
  modal.classList.add('show');
}
