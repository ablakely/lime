const test = require('node:test');
const assert = require('node:assert/strict');
const { getImageOpenUrl } = require('../public/js/manual-images.js');

test('getImageOpenUrl prefers the current resolved image source', () => {
  assert.equal(getImageOpenUrl({
    currentSrc: '/api/manual/current.png',
    src: '/api/manual/fallback.png',
    getAttribute() {
      return '/api/manual/attribute.png';
    }
  }), '/api/manual/current.png');
});
