(function routeUtilsSetup(globalObject) {
  function parsePathname(pathname) {
    const rawParts = pathname.split('/').filter(Boolean);
    const decodedParts = rawParts.map((part) => decodeURIComponent(part));

    return {
      rawParts,
      decodedParts,
      make: decodeURIComponent(rawParts[0] || ''),
      year: decodeURIComponent(rawParts[1] || ''),
      model: rawParts.slice(2).map((part) => decodeURIComponent(part)).join('/')
    };
  }

  function shouldShowQuickNavByDefault(parts) {
    return Array.isArray(parts) && parts.length === 0;
  }

  globalObject.routeUtils = { parsePathname, shouldShowQuickNavByDefault };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = { parsePathname, shouldShowQuickNavByDefault };
  }
})(typeof window !== 'undefined' ? window : globalThis);
