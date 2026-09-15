const test = require('node:test');
const assert = require('node:assert/strict');
const { getManualProxyBasePath, toManualProxyUrl } = require('../public/js/manual-url.js');

test('getManualProxyBasePath keeps directory paths trailing slash', () => {
  assert.equal(
    getManualProxyBasePath(['Chevrolet', '2001', 'Astro%20Van%20Cargo%2C%20RWD', 'Labor%20Times']),
    '/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/Labor%20Times/'
  );
});

test('getManualProxyBasePath trims filename segment to containing directory', () => {
  assert.equal(
    getManualProxyBasePath(['Chevrolet', '2001', 'Astro%20Van%20Cargo%2C%20RWD', 'index.html']),
    '/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/'
  );
});

test('toManualProxyUrl resolves legacy imagesNN paths from root', () => {
  assert.equal(
    toManualProxyUrl('images25/VA145687/', ['Chevrolet', '2001', 'Astro%20Van%20Cargo%2C%20RWD', 'Labor%20Times']),
    '/api/manual/images25/VA145687/'
  );
});

test('toManualProxyUrl keeps ordinary relative asset paths relative to manual page', () => {
  assert.equal(
    toManualProxyUrl('diagram.png', ['Chevrolet', '2001', 'Astro%20Van%20Cargo%2C%20RWD', 'Labor%20Times']),
    '/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/Labor%20Times/diagram.png'
  );
});
