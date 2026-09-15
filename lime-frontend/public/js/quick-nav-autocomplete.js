(function quickNavAutocompleteSetup(globalObject) {
  const DEFAULT_LIMIT = 8;

  function normalizeOption(value) {
    return String(value || '').trim();
  }

  function uniqueOptions(options) {
    const seen = new Set();
    const unique = [];

    (Array.isArray(options) ? options : []).forEach((option) => {
      const normalized = normalizeOption(option);
      const key = normalized.toLowerCase();
      if (!normalized || seen.has(key)) {
        return;
      }

      seen.add(key);
      unique.push(normalized);
    });

    return unique;
  }

  function filterOptions(options, query, limit = DEFAULT_LIMIT) {
    const normalizedQuery = normalizeOption(query).toLowerCase();
    const unique = uniqueOptions(options);

    if (!normalizedQuery) {
      return unique.slice(0, limit);
    }

    const startsWith = [];
    const contains = [];

    unique.forEach((option) => {
      const value = option.toLowerCase();
      if (value.startsWith(normalizedQuery)) {
        startsWith.push(option);
      } else if (value.includes(normalizedQuery)) {
        contains.push(option);
      }
    });

    return [...startsWith, ...contains].slice(0, limit);
  }

  globalObject.quickNavAutocomplete = {
    filterOptions,
    uniqueOptions
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      filterOptions,
      uniqueOptions
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
