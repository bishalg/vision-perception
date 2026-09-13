// apps/annotation_studio/tests/palm_guide.test.ts
// TDD tests for the palm crease hint guide mapping (pure logic, no DOM).

import assert from 'node:assert/strict';
import {
  PALM_GUIDE_LINES,
  PALM_GUIDE_FALLBACK_COLORS,
  CREASE_ZONE_MIN_Y,
  PALM_REGION_X,
  getGuideHighlight,
} from '../src/core/palmGuide.ts';

console.log('Running Palm Guide Mapping Tests...');

// Test 1: All four palm classes have a guide line with path + label + hint.
{
  const ids = PALM_GUIDE_LINES.map(l => l.id).sort();
  assert.deepEqual(ids, ['fate_line', 'head_line', 'heart_line', 'life_line']);
  for (const line of PALM_GUIDE_LINES) {
    assert.ok(line.path.length > 10, `${line.id} needs SVG path data`);
    assert.ok(line.label.length > 0, `${line.id} needs a label`);
    assert.ok(line.description.length > 0, `${line.id} needs a description`);
    assert.ok(PALM_GUIDE_FALLBACK_COLORS[line.id], `${line.id} needs a fallback color`);
  }
  console.log('  [PASS] Test 1: Guide covers all four crease classes');
}

// Test 2: Active class resolves to its own guide line.
{
  for (const id of ['heart_line', 'head_line', 'life_line', 'fate_line']) {
    const h = getGuideHighlight(id);
    assert.ok(h, `${id} should highlight`);
    assert.equal(h.line.id, id);
    assert.equal(h.isActive, true);
  }
  console.log('  [PASS] Test 2: Each class highlights its own crease');
}

// Test 3: Unknown classes / null render the neutral diagram (no highlight).
{
  assert.equal(getGuideHighlight(null), null);
  assert.equal(getGuideHighlight(''), null);
  assert.equal(getGuideHighlight('river_channel'), null);
  console.log('  [PASS] Test 3: Non-palm classes yield no highlight');
}

// Test 4: Crease geometry stays inside the palm zone by construction —
// no point may enter the finger band or leave the hand's x-range.
{
  for (const line of PALM_GUIDE_LINES) {
    const nums = line.path.match(/-?\d+\.?\d*/g)?.map(Number) ?? [];
    assert.ok(nums.length >= 4, `${line.id} has coordinate pairs`);
    for (let i = 0; i < nums.length; i += 2) {
      const x = nums[i];
      const y = nums[i + 1];
      assert.ok(
        x >= PALM_REGION_X[0] && x <= PALM_REGION_X[1],
        `${line.id} x=${x} leaves the palm region`,
      );
      assert.ok(y >= CREASE_ZONE_MIN_Y, `${line.id} y=${y} drifts into the finger zone`);
    }
  }
  console.log('  [PASS] Test 4: All crease points stay inside the palm zone');
}

// Test 5: Mirror helper — left hand flips x, right hand is identity.
{
  const { mirrorX, mirrorTransform } = await import('../src/components/Sidebar/Panels/guide/mirror.ts');
  assert.equal(mirrorX(0, 'left'), 220);
  assert.equal(mirrorX(220, 'left'), 0);
  assert.equal(mirrorX(110, 'left'), 110);
  assert.equal(mirrorX(49, 'right'), 49);
  assert.ok((mirrorTransform('left') ?? '').includes('scale(-1 1)'));
  assert.equal(mirrorTransform('right'), undefined);
  console.log('  [PASS] Test 5: Handedness mirror is exact');
}

// Test 6: Shared spec structure — what native apps consume.
{
  const { buildGuideSpec, GUIDE_SPEC_VERSION } = await import(
    '../src/components/Sidebar/Panels/guide/spec.ts'
  );
  const spec = buildGuideSpec() as Record<string, any>;
  assert.equal(spec.spec_version, GUIDE_SPEC_VERSION);
  assert.equal(spec.base_handedness, 'right');
  assert.equal(spec.creases.length, 4);
  assert.equal(spec.fingers.length, 4);
  assert.ok((spec.silhouette_path as string).length > 200);
  assert.ok((spec.silhouette_path as string).includes('199,108'), 'silhouette includes thumb tip');
  console.log('  [PASS] Test 6: Shared spec builds with full silhouette');
}

// Test 7: Filename-driven handedness — the guide follows the capture.
{
  const { inferHandednessFromFilename } = await import('../src/core/palmGuide.ts');
  assert.equal(inferHandednessFromFilename('01_left_bishal.jpg'), 'left');
  assert.equal(inferHandednessFromFilename('02_right_bishal.jpg'), 'right');
  assert.equal(inferHandednessFromFilename('RF_00001_palm.JPEG'), null);
  assert.equal(inferHandednessFromFilename(null), null);
  assert.equal(inferHandednessFromFilename('sample_L_04.png'), 'left');
  console.log('  [PASS] Test 7: Guide handedness follows the sample filename');
}
