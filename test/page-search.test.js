const test = require('node:test');
const assert = require('node:assert/strict');
const {
  normalizeSearchText,
  matchesSearchQuery,
  filterGroupedItems,
  getGroupedItemSectionNames
} = require('../public/js/page-search.js');

test('normalizeSearchText lowercases and collapses whitespace', () => {
  assert.equal(normalizeSearchText('  Astro   Van  '), 'astro van');
});

test('matchesSearchQuery supports partial case-insensitive matches', () => {
  assert.equal(matchesSearchQuery('Astro Van Cargo', 'van'), true);
  assert.equal(matchesSearchQuery('Astro Van Cargo', 'SAVANA'), false);
});

test('filterGroupedItems hides section headers without matching items', () => {
  const visibility = filterGroupedItems([
    { kind: 'header', text: 'Repair' },
    { kind: 'item', text: 'Brakes' },
    { kind: 'item', text: 'Engine' },
    { kind: 'header', text: 'Specifications' },
    { kind: 'item', text: 'Dimensions' }
  ], 'engine');

  assert.deepEqual(visibility, [true, false, true, false, false]);
});

test('filterGroupedItems shows a full section when its header matches the query', () => {
  const visibility = filterGroupedItems([
    { kind: 'header', text: 'Repair' },
    { kind: 'item', text: 'Brakes' },
    { kind: 'item', text: 'Engine' },
    { kind: 'header', text: 'Specifications' },
    { kind: 'item', text: 'Dimensions' },
    { kind: 'item', text: 'Capacities' }
  ], 'spec');

  assert.deepEqual(visibility, [false, false, false, true, true, true]);
});

test('filterGroupedItems keeps each matching duplicate item under its section header', () => {
  const visibility = filterGroupedItems([
    { kind: 'header', text: 'Specifications' },
    { kind: 'item', text: 'Overview' },
    { kind: 'header', text: 'Repair' },
    { kind: 'item', text: 'Overview' }
  ], 'overview');

  assert.deepEqual(visibility, [true, true, true, true]);
});

test('filterGroupedItems shows every entry when query is empty', () => {
  const visibility = filterGroupedItems([
    { kind: 'header', text: 'Repair' },
    { kind: 'item', text: 'Brakes' }
  ], '');

  assert.deepEqual(visibility, [true, true]);
});

test('getGroupedItemSectionNames maps items to their current section header', () => {
  const sectionNames = getGroupedItemSectionNames([
    { kind: 'header', text: 'Repair' },
    { kind: 'item', text: 'Brakes' },
    { kind: 'item', text: 'Engine' },
    { kind: 'header', text: 'Specifications' },
    { kind: 'item', text: 'Dimensions' }
  ]);

  assert.deepEqual(sectionNames, ['Repair', 'Repair', 'Repair', 'Specifications', 'Specifications']);
});
