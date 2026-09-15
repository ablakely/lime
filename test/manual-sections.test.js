const test = require('node:test');
const assert = require('node:assert/strict');
const {
  getManualSectionName,
  getSectionedManuals
} = require('../public/js/manual-sections.js');

test('getManualSectionName returns the decoded parent section from a manual uri', () => {
  assert.equal(
    getManualSectionName('/manual/Ford/2008/F-150/Specifications/Engine.html'),
    'Specifications'
  );
});

test('getSectionedManuals keeps consecutive manuals in one rendered section', () => {
  const entries = getSectionedManuals([
    { name: 'Dimensions', uri: '/manual/Ford/2008/F-150/Specifications/Dimensions.html' },
    { name: 'Capacities', uri: '/manual/Ford/2008/F-150/Specifications/Capacities.html' }
  ]);

  assert.deepEqual(entries, [
    { kind: 'header', text: 'Specifications' },
    {
      kind: 'item',
      name: 'Dimensions',
      uri: '/manual/Ford/2008/F-150/Specifications/Dimensions.html',
      section: 'Specifications'
    },
    {
      kind: 'item',
      name: 'Capacities',
      uri: '/manual/Ford/2008/F-150/Specifications/Capacities.html',
      section: 'Specifications'
    }
  ]);
});

test('getSectionedManuals restores a header when the same section reappears later', () => {
  const entries = getSectionedManuals([
    { name: 'Overview', uri: '/manual/Ford/2008/F-150/Specifications/Overview.html' },
    { name: 'Overview', uri: '/manual/Ford/2008/F-150/Repair/Overview.html' },
    { name: 'Torque', uri: '/manual/Ford/2008/F-150/Specifications/Torque.html' }
  ]);

  assert.deepEqual(entries, [
    { kind: 'header', text: 'Specifications' },
    {
      kind: 'item',
      name: 'Overview',
      uri: '/manual/Ford/2008/F-150/Specifications/Overview.html',
      section: 'Specifications'
    },
    { kind: 'header', text: 'Repair' },
    {
      kind: 'item',
      name: 'Overview',
      uri: '/manual/Ford/2008/F-150/Repair/Overview.html',
      section: 'Repair'
    },
    { kind: 'header', text: 'Specifications' },
    {
      kind: 'item',
      name: 'Torque',
      uri: '/manual/Ford/2008/F-150/Specifications/Torque.html',
      section: 'Specifications'
    }
  ]);
});
