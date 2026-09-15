const test = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { once } = require('node:events');

const realFetch = global.fetch;
const ENV_KEYS = ['LEMON_API_URL', 'API_BASE_URL', 'CI', 'NODE_ENV'];

function withEnv(overrides, callback) {
  const previous = new Map();

  ENV_KEYS.forEach((key) => {
    previous.set(key, process.env[key]);

    if (Object.prototype.hasOwnProperty.call(overrides, key)) {
      const value = overrides[key];
      if (value === undefined || value === null || value === '') {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
      return;
    }

    delete process.env[key];
  });

  try {
    return callback();
  } finally {
    ENV_KEYS.forEach((key) => {
      const value = previous.get(key);
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    });
  }
}

function loadServer(overrides = {}) {
  return withEnv(overrides, () => {
    delete require.cache[require.resolve('../server.js')];
    return require('../server.js');
  });
}

async function startServer(overrides = {}) {
  delete require.cache[require.resolve('../server.js')];
  const { app } = loadServer(overrides);
  const server = http.createServer(app);
  server.listen(0);
  await once(server, 'listening');
  return server;
}

test('manual api route proxies html responses without converting them to json', async (t) => {
  global.fetch = async (url, options) => {
    assert.equal(url, 'http://localhost:8080/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/');
    assert.equal(options.headers.Accept, 'text/html, application/json');

    return new Response('<section><h1>Repair and Diagnosis</h1></section>', {
      status: 200,
      headers: { 'content-type': 'text/html; charset=utf-8' }
    });
  };

  const server = await startServer({ LEMON_API_URL: 'http://localhost:8080' });
  t.after(() => {
    global.fetch = realFetch;
    server.close();
  });

  const response = await realFetch(`http://127.0.0.1:${server.address().port}/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/`);

  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'text/html; charset=utf-8');
  assert.match(await response.text(), /Repair and Diagnosis/);
});

test('manual api route preserves directory json payloads', async (t) => {
  global.fetch = async () => new Response(JSON.stringify({
    title: 'Chevrolet: 2001: Astro Van Cargo, RWD',
    breadcrumbs: [{ label: 'Chevrolet', href: '/Chevrolet/' }],
    topics: [],
    manuals: [{ name: 'Labor Times', uri: '/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/Labor%20Times/' }]
  }), {
    status: 200,
    headers: { 'content-type': 'application/json; charset=utf-8' }
  });

  const server = await startServer({ LEMON_API_URL: 'http://localhost:8080' });
  t.after(() => {
    global.fetch = realFetch;
    server.close();
  });

  const response = await realFetch(`http://127.0.0.1:${server.address().port}/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/`);

  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'application/json; charset=utf-8');
  assert.deepEqual(await response.json(), {
    title: 'Chevrolet: 2001: Astro Van Cargo, RWD',
    breadcrumbs: [{ label: 'Chevrolet', href: '/Chevrolet/' }],
    topics: [],
    manuals: [{ name: 'Labor Times', uri: '/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/Labor%20Times/' }]
  });
});

test('manual api route preserves binary image responses', async (t) => {
  const imageBytes = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10, 0, 255, 17, 34]);

  global.fetch = async (url, options) => {
    assert.equal(url, 'http://localhost:8080/images25/assets/example.png');
    assert.equal(options.headers.Accept, 'text/html, application/json');

    return new Response(imageBytes, {
      status: 200,
      headers: { 'content-type': 'image/png' }
    });
  };

  const server = await startServer({ LEMON_API_URL: 'http://localhost:8080' });
  t.after(() => {
    global.fetch = realFetch;
    server.close();
  });

  const response = await realFetch(`http://127.0.0.1:${server.address().port}/api/manual/images25/assets/example.png`);

  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'image/png');
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), imageBytes);
});

test('images route preserves binary image responses', async (t) => {
  const imageBytes = Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10, 0, 255, 17, 34]);

  global.fetch = async (url, options) => {
    assert.equal(url, 'http://localhost:8080/images/DM02Q313/chry60/515649253/');
    assert.equal(options.headers.Accept, 'text/html, application/json');

    return new Response(imageBytes, {
      status: 200,
      headers: { 'content-type': 'image/png' }
    });
  };

  const server = await startServer({ LEMON_API_URL: 'http://localhost:8080' });
  t.after(() => {
    global.fetch = realFetch;
    server.close();
  });

  const response = await realFetch(`http://127.0.0.1:${server.address().port}/images/DM02Q313/chry60/515649253/`);

  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'image/png');
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()), imageBytes);
});

test('resolveLemonApiConfig prefers LEMON_API_URL and joins nested paths safely', () => {
  const { lemonUrl, resolveLemonApiConfig } = loadServer({
    LEMON_API_URL: 'https://lemon.example/manuals/',
    API_BASE_URL: 'https://ignored.example/api/'
  });

  const config = resolveLemonApiConfig({
    LEMON_API_URL: 'https://lemon.example/manuals/',
    API_BASE_URL: 'https://ignored.example/api/'
  });

  assert.equal(config.baseUrl, 'https://lemon.example/manuals');
  assert.equal(config.source, 'LEMON_API_URL');
  assert.equal(
    lemonUrl('/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/'),
    'https://lemon.example/manuals/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/'
  );
});

test('resolveLemonApiConfig falls back to API_BASE_URL for compatibility', () => {
  const { lemonUrl, resolveLemonApiConfig } = loadServer({
    API_BASE_URL: 'https://legacy.example/api/'
  });

  const config = resolveLemonApiConfig({
    API_BASE_URL: 'https://legacy.example/api/'
  });

  assert.equal(config.baseUrl, 'https://legacy.example/api');
  assert.equal(config.source, 'API_BASE_URL');
  assert.match(config.warning, /authoritative frontend setting/);
  assert.equal(lemonUrl('/makes'), 'https://legacy.example/api/makes');
});

test('lemonUrl treats user input as path segments instead of allowing absolute URL overrides', () => {
  const { lemonUrl } = loadServer({
    LEMON_API_URL: 'https://lemon.example/manuals/'
  });

  assert.equal(
    lemonUrl('https://evil.example/steal'),
    'https://lemon.example/manuals/https%3A/evil.example/steal'
  );
});

test('manual api route returns a friendly error and logs diagnostics when upstream fetch fails', async (t) => {
  const errorLogs = [];
  const originalConsoleError = console.error;

  console.error = (...args) => {
    errorLogs.push(args);
  };

  global.fetch = async (url) => {
    if (url.startsWith('http://127.0.0.1:')) {
      return realFetch(url);
    }

    throw new TypeError('fetch failed');
  };

  const server = await startServer({ LEMON_API_URL: 'https://lemon.example/manuals/' });
  t.after(() => {
    console.error = originalConsoleError;
    global.fetch = realFetch;
    server.close();
  });

  const response = await realFetch(`http://127.0.0.1:${server.address().port}/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/`);

  assert.equal(response.status, 502);
  assert.deepEqual(await response.json(), {
    error: 'Unable to reach the LEMON API',
    details: 'The manuals service is temporarily unavailable. Please try again later.'
  });
  assert.equal(errorLogs.length, 1);
  assert.equal(errorLogs[0][0], '[lime-frontend] LEMON API request failed');
  assert.equal(errorLogs[0][1].pathname, '/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/');
  assert.equal(errorLogs[0][1].targetUrl, 'https://lemon.example/manuals/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/');
  assert.equal(errorLogs[0][1].source, 'LEMON_API_URL');
  assert.equal(errorLogs[0][1].error, 'fetch failed');
});

test('manual api route fails clearly when no api url is configured outside local development', async (t) => {
  let upstreamCalled = false;

  global.fetch = async (url) => {
    if (url.startsWith('http://127.0.0.1:')) {
      return realFetch(url);
    }

    upstreamCalled = true;
    throw new Error('unexpected upstream call');
  };

  const server = await startServer({ NODE_ENV: 'production', CI: 'true' });
  t.after(() => {
    global.fetch = realFetch;
    server.close();
  });

  const response = await realFetch(`http://127.0.0.1:${server.address().port}/api/manual/Chevrolet/2001/Astro%20Van%20Cargo%2C%20RWD/`);

  assert.equal(response.status, 503);
  assert.equal(upstreamCalled, false);
  assert.deepEqual(await response.json(), {
    error: 'LEMON API is not configured',
    details: 'Set LEMON_API_URL to the LEMON backend base URL for this environment.'
  });
});
