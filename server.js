const express = require('express');
const fs = require('fs');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3000;
const INDEX_HTML = fs.readFileSync(path.join(__dirname, 'public/index.html'), 'utf8');
const LOCAL_LEMON_API_URL = 'http://localhost:8080';
const USER_FACING_CONFIG_MESSAGE = 'Set LEMON_API_URL to the LEMON backend base URL for this environment.';
const USER_FACING_CONNECTIVITY_MESSAGE = 'The manuals service is temporarily unavailable. Please try again later.';

app.use(express.static('public'));

function isLocalDevelopmentEnvironment(env = process.env) {
  return !env.CI && (env.NODE_ENV || 'development') !== 'production';
}

function normalizeBaseUrl(rawUrl) {
  const url = new URL(rawUrl);

  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new TypeError(`Unsupported LEMON API protocol: ${url.protocol}`);
  }

  url.hash = '';
  if (url.pathname !== '/') {
    url.pathname = url.pathname.replace(/\/+$/, '');
  }

  return url.toString();
}

function normalizeUpstreamPath(pathname) {
  const rawPath = String(pathname || '/');
  const hasTrailingSlash = rawPath.endsWith('/');
  const normalizedSegments = rawPath
    .split('/')
    .filter(Boolean)
    .map((segment) => {
      const decoded = decodeURIComponent(segment);

      if (decoded === '.' || decoded === '..') {
        throw new TypeError('Invalid LEMON API path segment.');
      }

      return encodeURIComponent(decoded);
    });

  if (normalizedSegments.length === 0) {
    return '/';
  }

  const normalizedPath = `/${normalizedSegments.join('/')}`;
  return hasTrailingSlash ? `${normalizedPath}/` : normalizedPath;
}

function resolveLemonApiConfig(env = process.env) {
  const lemonApiUrl = env.LEMON_API_URL && env.LEMON_API_URL.trim();
  const legacyApiBaseUrl = env.API_BASE_URL && env.API_BASE_URL.trim();
  const rawUrl = lemonApiUrl || legacyApiBaseUrl || (isLocalDevelopmentEnvironment(env) ? LOCAL_LEMON_API_URL : '');
  const source = lemonApiUrl ? 'LEMON_API_URL' : legacyApiBaseUrl ? 'API_BASE_URL' : rawUrl ? 'default' : 'missing';

  if (!rawUrl) {
    return {
      available: false,
      source,
      warning: `LEMON_API_URL is not set. Refusing to default to ${LOCAL_LEMON_API_URL} outside local development.`,
      userMessage: USER_FACING_CONFIG_MESSAGE
    };
  }

  try {
    const baseUrl = normalizeBaseUrl(rawUrl);
    let warning = null;

    if (!lemonApiUrl && legacyApiBaseUrl) {
      warning = 'API_BASE_URL is supported for compatibility, but LEMON_API_URL is the authoritative frontend setting.';
    } else if (source === 'default') {
      warning = `LEMON_API_URL is not set. Defaulting to ${LOCAL_LEMON_API_URL} for local development.`;
    }

    return {
      available: true,
      baseUrl,
      source,
      warning
    };
  } catch (error) {
    return {
      available: false,
      source,
      warning: `Invalid LEMON API URL from ${source}: ${error.message}`,
      userMessage: USER_FACING_CONFIG_MESSAGE
    };
  }
}

const LEMON_API_CONFIG = resolveLemonApiConfig();

function lemonUrl(pathname, config = LEMON_API_CONFIG) {
  if (!config.available || !config.baseUrl) {
    throw new Error('LEMON API base URL is not configured.');
  }

  const baseUrl = new URL(config.baseUrl.endsWith('/') ? config.baseUrl : `${config.baseUrl}/`);
  const normalizedPath = normalizeUpstreamPath(pathname);
  baseUrl.pathname = `${baseUrl.pathname.replace(/\/$/, '')}${normalizedPath}`;
  return baseUrl.toString();
}

function logProxyFailure(kind, pathname, config, error, targetUrl) {
  console.error(`[lime-frontend] ${kind}`, {
    pathname,
    targetUrl: targetUrl || null,
    baseUrl: config.baseUrl || null,
    source: config.source,
    error: error.message
  });
}

function sendConfigError(res) {
  return res.status(503).json({
    error: 'LEMON API is not configured',
    details: USER_FACING_CONFIG_MESSAGE
  });
}

function sendConnectivityError(res) {
  return res.status(502).json({
    error: 'Unable to reach the LEMON API',
    details: USER_FACING_CONNECTIVITY_MESSAGE
  });
}

async function proxyJson(res, pathname) {
  if (!LEMON_API_CONFIG.available) {
    return sendConfigError(res);
  }

  let targetUrl = null;

  try {
    targetUrl = lemonUrl(pathname);
    const response = await fetch(targetUrl, {
      headers: { Accept: 'application/json' }
    });

    if (!response.ok) {
      const text = await response.text();
      return res.status(response.status).json({
        error: `LEMON API request failed (${response.status})`,
        details: text || response.statusText
      });
    }

    const contentType = response.headers.get('content-type') || '';
    if (!contentType.includes('application/json')) {
      const text = await response.text();
      return res.status(502).json({
        error: 'LEMON API returned a non-JSON response',
        details: text.slice(0, 500)
      });
    }

    return res.json(await response.json());
  } catch (error) {
    logProxyFailure('LEMON API request failed', pathname, LEMON_API_CONFIG, error, targetUrl);
    return sendConnectivityError(res);
  }
}

async function proxyManual(res, pathname) {
  if (!LEMON_API_CONFIG.available) {
    return sendConfigError(res);
  }

  let targetUrl = null;

  try {
    targetUrl = lemonUrl(pathname);
    const response = await fetch(targetUrl, {
      headers: { Accept: 'text/html, application/json' }
    });
    const contentType = response.headers.get('content-type') || '';
    const bodyBuffer = Buffer.from(await response.arrayBuffer());
    const bodyText = bodyBuffer.toString('utf8');

    if (!response.ok) {
      if (contentType.includes('application/json')) {
        try {
          const data = JSON.parse(bodyText);
          return res.status(response.status).json({
            error: data.error || `LEMON API request failed (${response.status})`,
            details: data.details || response.statusText
          });
        } catch (_error) {
          // Fall through to text error payload.
        }
      }

      return res.status(response.status).json({
        error: `LEMON API request failed (${response.status})`,
        details: bodyText || response.statusText
      });
    }

    if (contentType.includes('application/json')) {
      return res.type('application/json').send(bodyText);
    }

    return res.type(contentType || 'text/html').send(bodyBuffer);
  } catch (error) {
    logProxyFailure('LEMON API request failed', pathname, LEMON_API_CONFIG, error, targetUrl);
    return sendConnectivityError(res);
  }
}

app.get('/api/makes', async (_req, res) => proxyJson(res, '/'));
app.get('/api/:make', async (req, res) => proxyJson(res, `/${encodeURIComponent(req.params.make)}/`));
app.get('/api/:make/:year', async (req, res) =>
  proxyJson(res, `/${encodeURIComponent(req.params.make)}/${encodeURIComponent(req.params.year)}/`)
);
app.get('/api/:make/:year/:model', async (req, res) =>
  proxyJson(
    res,
    `/${encodeURIComponent(req.params.make)}/${encodeURIComponent(req.params.year)}/${encodeURIComponent(req.params.model)}/`
  )
);
app.get('/api/manual/*path', async (req, res) => {
  const manualPrefix = '/api/manual/';
  const requestPath = req.originalUrl.split('?')[0];
  const manualPath = requestPath.startsWith(manualPrefix)
    ? requestPath.slice(manualPrefix.length)
    : (Array.isArray(req.params.path) ? req.params.path.join('/') : req.params.path);
  return proxyManual(res, `/${manualPath}`);
});


app.get('/images:num/*path', async (req, res) => {
   return proxyManual(res, `/images${req.params.num}/${ (Array.isArray(req.params.path)) ? req.params.path.join('/') : req.params.path }`);
});

app.get('/images/*path', async (req, res) => {
  return proxyManual(
    res,
    `/images/${(Array.isArray(req.params.path)) ? req.params.path.join('/') : req.params.path}`
  );
});

app.get(/^\/(?!api).*/, (_req, res) => {
  res.type('html').send(INDEX_HTML);
});

if (require.main === module) {
  app.listen(PORT, () => {
    console.log(`lime-frontend listening on http://localhost:${PORT}`);
    if (LEMON_API_CONFIG.warning) {
      console.warn(`[lime-frontend] ${LEMON_API_CONFIG.warning}`);
    }

    if (LEMON_API_CONFIG.available) {
      console.log(`Proxying LEMON API: ${LEMON_API_CONFIG.baseUrl}`);
    }
  });
}

module.exports = {
  app,
  lemonUrl,
  resolveLemonApiConfig,
  proxyJson,
  proxyManual
};
