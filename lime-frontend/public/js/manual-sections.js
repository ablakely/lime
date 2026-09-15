(function manualSectionsSetup(globalObject) {
  function getManualSectionName(uri) {
    const parts = String(uri || '').split('/').filter(Boolean);
    return decodeURIComponent(parts[parts.length - 2] || '');
  }

  function getSectionedManuals(manuals) {
    let currentSectionName = null;

    return (Array.isArray(manuals) ? manuals : []).reduce((entries, manual) => {
      const section = getManualSectionName(manual && manual.uri);

      if (section && section !== currentSectionName) {
        entries.push({ kind: 'header', text: section });
        currentSectionName = section;
      } else if (!section) {
        currentSectionName = null;
      }

      entries.push({
        kind: 'item',
        name: manual && manual.name,
        uri: manual && manual.uri,
        section
      });

      return entries;
    }, []);
  }

  globalObject.manualSections = {
    getManualSectionName,
    getSectionedManuals
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      getManualSectionName,
      getSectionedManuals
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
