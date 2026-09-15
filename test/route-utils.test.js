const test = require('node:test');
const assert = require('node:assert/strict');
const { parsePathname, shouldShowQuickNavByDefault } = require('../public/js/route-utils.js');

test('parsePathname handles home route', () => {
  const parsed = parsePathname('/');
  assert.deepEqual(parsed.rawParts, []);
  assert.deepEqual(parsed.decodedParts, []);
  assert.equal(parsed.make, '');
  assert.equal(parsed.year, '');
  assert.equal(parsed.model, '');
});

test('parsePathname handles make/year/model route', () => {
  const parsed = parsePathname('/Buick/2012/LaCrosse%20Leather');
  assert.deepEqual(parsed.decodedParts, ['Buick', '2012', 'LaCrosse Leather']);
  assert.equal(parsed.make, 'Buick');
  assert.equal(parsed.year, '2012');
  assert.equal(parsed.model, 'LaCrosse Leather');
});

test('parsePathname keeps encoded slash inside model segment', () => {
  const parsed = parsePathname('/Ford/2020/F-150%2FLariat');
  assert.deepEqual(parsed.rawParts, ['Ford', '2020', 'F-150%2FLariat']);
  assert.equal(parsed.model, 'F-150/Lariat');
});

test('parsePathname supports deeper manual paths', () => {
  const parsed = parsePathname('/Buick/2012/LaCrosse/Repair%20and%20Diagnosis/');
  assert.equal(parsed.make, 'Buick');
  assert.equal(parsed.year, '2012');
  assert.equal(parsed.model, 'LaCrosse/Repair and Diagnosis');
});

test('parsePathname preserves deep encoded segments for manual navigation', () => {
  const parsed = parsePathname(
    '/GMC/1998/Cab%20%26%20Chassis%20C3500%2C%202D%20Pickup%2C%206.5%20F%2C%20Automatic/Repair%20and%20Diagnosis/'
  );
  assert.deepEqual(parsed.rawParts, [
    'GMC',
    '1998',
    'Cab%20%26%20Chassis%20C3500%2C%202D%20Pickup%2C%206.5%20F%2C%20Automatic',
    'Repair%20and%20Diagnosis'
  ]);
  assert.deepEqual(parsed.decodedParts, [
    'GMC',
    '1998',
    'Cab & Chassis C3500, 2D Pickup, 6.5 F, Automatic',
    'Repair and Diagnosis'
  ]);
});

test('shouldShowQuickNavByDefault only enables the selector on the home route', () => {
  assert.equal(shouldShowQuickNavByDefault([]), true);
  assert.equal(shouldShowQuickNavByDefault(['Buick']), false);
  assert.equal(shouldShowQuickNavByDefault(['Buick', '2012', 'LaCrosse']), false);
});
