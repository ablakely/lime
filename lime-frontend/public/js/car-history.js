(function carHistorySetup(globalObject) {
  const STORAGE_KEY = 'lime.quickNavHistory';
  const MAX_HISTORY_ITEMS = 5;

  function normalizeSelection(selection) {
    if (!selection || typeof selection !== 'object') {
      return null;
    }

    const make = String(selection.make || '').trim();
    const year = String(selection.year || '').trim();
    const model = String(selection.model || '').trim();

    if (!make) {
      return null;
    }

    return { make, year, model };
  }

  function selectionsMatch(left, right) {
    return left.make === right.make && left.year === right.year && left.model === right.model;
  }

  function toHistoryLabel(selection) {
    return [selection.make, selection.year, selection.model].filter(Boolean).join(' / ');
  }

  function sanitizeHistory(history) {
    if (!Array.isArray(history)) {
      return [];
    }

    const sanitized = [];
    history.forEach((entry) => {
      const normalized = normalizeSelection(entry);
      if (!normalized || sanitized.some((existing) => selectionsMatch(existing, normalized))) {
        return;
      }

      sanitized.push(normalized);
    });

    return sanitized.slice(0, MAX_HISTORY_ITEMS);
  }

  function loadHistory(storage) {
    if (!storage || typeof storage.getItem !== 'function') {
      return [];
    }

    try {
      return sanitizeHistory(JSON.parse(storage.getItem(STORAGE_KEY) || '[]'));
    } catch (_error) {
      return [];
    }
  }

  function saveSelection(storage, selection) {
    const normalized = normalizeSelection(selection);
    if (!normalized) {
      return [];
    }

    const nextHistory = [
      normalized,
      ...loadHistory(storage).filter((entry) => !selectionsMatch(entry, normalized))
    ].slice(0, MAX_HISTORY_ITEMS);

    if (storage && typeof storage.setItem === 'function') {
      try {
        storage.setItem(STORAGE_KEY, JSON.stringify(nextHistory));
      } catch (_error) {
        return nextHistory;
      }
    }

    return nextHistory;
  }

  function clearHistory(storage) {
    if (!storage) {
      return [];
    }

    if (typeof storage.removeItem === 'function') {
      try {
        storage.removeItem(STORAGE_KEY);
        return [];
      } catch (_error) {
      }
    }

    if (typeof storage.setItem === 'function') {
      try {
        storage.setItem(STORAGE_KEY, JSON.stringify([]));
      } catch (_error) {
      }
    }

    return [];
  }

  globalObject.carHistory = {
    clearHistory,
    loadHistory,
    saveSelection,
    toHistoryLabel
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      clearHistory,
      loadHistory,
      saveSelection,
      toHistoryLabel
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
