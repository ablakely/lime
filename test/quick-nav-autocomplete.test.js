const test = require('node:test');
const assert = require('node:assert/strict');
const { filterOptions, uniqueOptions } = require('../public/js/quick-nav-autocomplete.js');

test('uniqueOptions trims values and removes case-insensitive duplicates', () => {
  assert.deepEqual(uniqueOptions([' Ford ', 'ford', 'GM', '', null, 'GM ']), ['Ford', 'GM']);
});

test('filterOptions prefers prefix matches before contains matches', () => {
  assert.deepEqual(
    filterOptions(['Focus', 'Escape', 'Mustang', 'Explorer'], 'es'),
    ['Escape']
  );
  assert.deepEqual(
    filterOptions(['Silverado', 'Sierra', 'Transit Connect', 'Connect Van'], 'connect'),
    ['Connect Van', 'Transit Connect']
  );
});

test('filterOptions returns a capped list when the query is empty', () => {
  const options = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];
  assert.equal(filterOptions(options, '').length, 8);
});
