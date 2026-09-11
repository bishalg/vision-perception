// apps/annotation_studio/tests/drawing_tools.test.ts
// TDD Test Suite verifying polyline drawing, class isolation, auto-commit, and undo/redo.

import assert from 'node:assert/strict';
import {
  studioStore,
  selectClass,
  setTool,
  performUndo,
  performRedo,
  initialStudioState,
} from '../src/core/store.ts';
import {
  handlePointerDown,
  handlePointerMove,
  handlePointerUp,
  finishDraft,
  cancelDraft,
} from '../src/core/tools.ts';
import type { DomainConfig } from '../src/core/types.ts';

const mockPalmDomain: DomainConfig = {
  id: 'palm',
  label: 'Palm Creases',
  coord_space: 'norm_512',
  classes: [
    { id: 'heart_line', label: 'Heart Line', color: '#F43F5E' },
    { id: 'head_line', label: 'Head Line', color: '#06B6D4' },
    { id: 'life_line', label: 'Life Line', color: '#10B981' },
    { id: 'fate_line', label: 'Fate Line', color: '#8B5CF6' },
  ],
};

function setupTestState() {
  studioStore.setState(() => ({
    ...initialStudioState,
    currentDomain: mockPalmDomain,
    activeTool: 'polyline',
    activeClassId: 'heart_line',
    polylines: {
      heart_line: [],
      head_line: [],
      life_line: [],
      fate_line: [],
    },
    polygons: {},
    bboxes: {},
    brushStrokes: [],
    undoStack: [],
    redoStack: [],
  }));
}

console.log('Running Annotation Studio Drawing & Class Isolation Tests...');

// Test 1: Class Isolation & Auto-Commit on Class Switch
{
  setupTestState();
  assert.equal(studioStore.state.activeClassId, 'heart_line');

  // Draw 2 points for heart_line
  handlePointerDown([100, 100]);
  handlePointerDown([150, 120]);
  assert.equal(studioStore.state.currentDraft.length, 2);

  // Switch to head_line
  selectClass('head_line');

  // Verification:
  // 1. heart_line must have committed the 2 points!
  assert.equal(studioStore.state.polylines['heart_line']?.length, 1, 'heart_line should have 1 committed polyline');
  assert.deepEqual(studioStore.state.polylines['heart_line'][0], [[100, 100], [150, 120]]);

  // 2. currentDraft must be cleared
  assert.equal(studioStore.state.currentDraft.length, 0, 'currentDraft should be empty after commit');

  // 3. activeClassId is now head_line
  assert.equal(studioStore.state.activeClassId, 'head_line');

  // 4. head_line currently has 0 polylines
  assert.equal(studioStore.state.polylines['head_line']?.length, 0);

  // Now draw for head_line
  handlePointerDown([200, 200]);
  handlePointerDown([250, 220]);
  finishDraft();

  // Verification:
  assert.equal(studioStore.state.polylines['head_line']?.length, 1, 'head_line should have 1 committed polyline');
  assert.deepEqual(studioStore.state.polylines['head_line'][0], [[200, 200], [250, 220]]);

  // Verify heart_line was NOT altered
  assert.equal(studioStore.state.polylines['heart_line']?.length, 1);
  assert.deepEqual(studioStore.state.polylines['heart_line'][0], [[100, 100], [150, 120]]);

  console.log('  [PASS] Test 1: Class Isolation & Auto-Commit on Class Switch');
}

// Test 2: Freehand Drag-to-Draw Polyline Auto-Commits on Mouse Up
{
  setupTestState();
  selectClass('life_line');

  // Press down
  handlePointerDown([50, 50]);
  assert.equal(studioStore.state.currentDraft.length, 1);

  // Drag stroke (> 8px distance)
  handlePointerMove([55, 55], true);
  handlePointerMove([60, 65], true);
  handlePointerMove([70, 80], true);

  assert.ok(studioStore.state.currentDraft.length >= 3, 'Draft should contain dragged points');

  // Release mouse
  handlePointerUp([70, 80]);

  // Verification: Freehand stroke automatically committed to life_line!
  assert.equal(studioStore.state.polylines['life_line']?.length, 1, 'life_line should have 1 committed polyline');
  assert.equal(studioStore.state.currentDraft.length, 0, 'currentDraft should be reset after freehand mouseup');

  console.log('  [PASS] Test 2: Freehand Drag-to-Draw Polyline Auto-Commits on Mouse Up');
}

// Test 3: Click-to-Click Multi-Segment Polyline (Mouse Up does NOT commit single click)
{
  setupTestState();
  selectClass('fate_line');

  // Click 1 (down and up with 0 drag distance)
  handlePointerDown([300, 100]);
  handlePointerUp([300, 100]);
  assert.equal(studioStore.state.currentDraft.length, 1, 'Point 1 should stay in draft');
  assert.equal(studioStore.state.polylines['fate_line']?.length, 0, 'Should not commit after 1 click');

  // Click 2
  handlePointerDown([300, 200]);
  handlePointerUp([300, 200]);
  assert.equal(studioStore.state.currentDraft.length, 2, 'Point 2 should be added');
  assert.equal(studioStore.state.polylines['fate_line']?.length, 0, 'Should not auto-commit multi-click yet');

  // Click 3
  handlePointerDown([300, 300]);
  handlePointerUp([300, 300]);
  assert.equal(studioStore.state.currentDraft.length, 3);

  // Explicit finish (double-click, right-click, or Enter)
  finishDraft();
  assert.equal(studioStore.state.polylines['fate_line']?.length, 1);
  assert.equal(studioStore.state.polylines['fate_line'][0].length, 3);
  assert.equal(studioStore.state.currentDraft.length, 0);

  console.log('  [PASS] Test 3: Click-to-Click Multi-Segment Polyline');
}

// Test 4: Switching Tools Auto-Commits Pending Draft
{
  setupTestState();
  selectClass('heart_line');

  handlePointerDown([10, 10]);
  handlePointerDown([20, 20]);
  assert.equal(studioStore.state.currentDraft.length, 2);

  // Switch tool to pan
  setTool('pan');

  // Draft should be committed to heart_line
  assert.equal(studioStore.state.polylines['heart_line']?.length, 1);
  assert.equal(studioStore.state.activeTool, 'pan');
  assert.equal(studioStore.state.currentDraft.length, 0);

  console.log('  [PASS] Test 4: Switching Tools Auto-Commits Pending Draft');
}

// Test 5: Cancel Draft on Escape
{
  setupTestState();
  handlePointerDown([10, 10]);
  handlePointerDown([20, 20]);
  assert.equal(studioStore.state.currentDraft.length, 2);

  cancelDraft();
  assert.equal(studioStore.state.currentDraft.length, 0);
  assert.equal(studioStore.state.polylines['heart_line']?.length, 0);

  console.log('  [PASS] Test 5: Cancel Draft on Escape');
}

// Test 6: Undo & Redo Integration
{
  setupTestState();
  selectClass('head_line');

  // Draw and commit
  handlePointerDown([10, 10]);
  handlePointerDown([20, 20]);
  finishDraft();
  assert.equal(studioStore.state.polylines['head_line']?.length, 1);

  // Undo
  performUndo();
  assert.equal(studioStore.state.polylines['head_line']?.length, 0, 'Polyline should be undone');

  // Redo
  performRedo();
  assert.equal(studioStore.state.polylines['head_line']?.length, 1, 'Polyline should be restored');

  console.log('  [PASS] Test 6: Undo & Redo Integration');
}

console.log('\nALL DRAWING & CLASS ISOLATION TESTS PASSED!\n');
