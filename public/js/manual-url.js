(function manualUrlSetup(globalObject) {
  function getManualProxyBasePath(rawParts) {
    const proxyPath = `/api/manual/${rawParts.join('/')}`;
    if (proxyPath.endsWith('/')) {
      return proxyPath;
    }

    const lastSegment = rawParts.length > 0 ? decodeURIComponent(rawParts[rawParts.length - 1]) : '';
    if (/\.[a-z0-9]+$/i.test(lastSegment)) {
      return proxyPath.slice(0, proxyPath.lastIndexOf('/') + 1);
    }

    return `${proxyPath}/`;
  }

  function normalizeLegacyAssetPath(value) {
    if (typeof value !== 'string' || value.startsWith('/') || value.startsWith('./') || value.startsWith('../')) {
      return value;
    }

    if (/^images\d+\//i.test(value)) {
      return `/${value}`;
    }

    return value;
  }

  function toManualProxyUrl(value, rawParts) {
    if (!value || value.startsWith('data:') || value.startsWith('blob:') || value.startsWith('#')) {
      return value;
    }

    if (/^[a-z][a-z0-9+.-]*:/i.test(value) || value.startsWith('//') || value.startsWith('/api/manual/')) {
      return value;
    }

    const basePath = getManualProxyBasePath(rawParts);
    const manualOrigin = 'https://manual.local';
    const normalizedValue = normalizeLegacyAssetPath(value);
    const resolved = new URL(normalizedValue, `${manualOrigin}${basePath}`);
    const proxiedPathname = resolved.pathname.startsWith('/api/manual/')
      ? resolved.pathname
      : `/api/manual${resolved.pathname}`;
    return `${proxiedPathname}${resolved.search}${resolved.hash}`;
  }

  globalObject.manualUrl = {
    getManualProxyBasePath,
    toManualProxyUrl
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      getManualProxyBasePath,
      toManualProxyUrl
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
