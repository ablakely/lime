const test = require('node:test');
const assert = require('node:assert/strict');
const {
  isManualDirectoryPayload,
  parseManualApiResponse
} = require('../public/js/manual-response.js');

test('isManualDirectoryPayload detects manual directory JSON payloads', () => {
  assert.equal(isManualDirectoryPayload({
    title: 'Chevrolet: 2001: Astro Van Cargo, RWD',
    breadcrumbs: [{ label: 'Chevrolet', href: '/Chevrolet/' }],
    topics: [],
    manuals: [{ name: 'Repair and Diagnosis', uri: '/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/Repair%20and%20Diagnosis/' }]
  }), true);
});

test('parseManualApiResponse classifies directory payload JSON responses', async () => {
  const response = new Response(JSON.stringify({
    title: 'Chevrolet: 2001: Astro Van Cargo, RWD',
    breadcrumbs: [{ label: 'Chevrolet', href: '/Chevrolet/' }],
    topics: [],
    manuals: [{ name: 'Labor Times', uri: '/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/Labor%20Times/' }]
  }), {
    status: 200,
    headers: { 'content-type': 'application/json; charset=utf-8' }
  });

  const parsed = await parseManualApiResponse(response);

  assert.equal(parsed.kind, 'directory');
  assert.equal(parsed.data.manuals[0].name, 'Labor Times');
});

test('parseManualApiResponse classifies html manual responses', async () => {
  const response = new Response('<article><h1>Removal &amp; Installation</h1></article>', {
    status: 200,
    headers: { 'content-type': 'text/plain; charset=utf-8' }
  });

  const parsed = await parseManualApiResponse(response);

  assert.equal(parsed.kind, 'html');
  assert.match(parsed.data, /Removal/);
});
