(function pageSearchSetup(globalObject) {
  function normalizeSearchText(value) {
    return String(value || '')
      .toLowerCase()
      .replace(/\s+/g, ' ')
      .trim();
  }

  function matchesSearchQuery(value, query) {
    const normalizedQuery = normalizeSearchText(query);
    if (!normalizedQuery) {
      return true;
    }

    return normalizeSearchText(value).includes(normalizedQuery);
  }

  function filterGroupedItems(items, query) {
    const normalizedQuery = normalizeSearchText(query);
    if (!normalizedQuery) {
      return items.map(() => true);
    }

    const visibility = items.map(() => false);
    let currentHeaderIndex = -1;
    let sectionHeaderMatches = false;
    let sectionHasVisibleItems = false;
    let sectionItemIndices = [];

    function commitHeaderVisibility() {
      if (currentHeaderIndex >= 0) {
        visibility[currentHeaderIndex] = sectionHeaderMatches || sectionHasVisibleItems;

        if (sectionHeaderMatches) {
          sectionItemIndices.forEach((itemIndex) => {
            visibility[itemIndex] = true;
          });
        }
      }
    }

    items.forEach((item, index) => {
      if (item.kind === 'header') {
        commitHeaderVisibility();
        currentHeaderIndex = index;
        sectionHeaderMatches = matchesSearchQuery(item.text, normalizedQuery);
        sectionHasVisibleItems = false;
        sectionItemIndices = [];
        return;
      }

      const isVisible = sectionHeaderMatches || matchesSearchQuery(item.text, normalizedQuery);
      visibility[index] = isVisible;

      if (currentHeaderIndex >= 0) {
        sectionItemIndices.push(index);

        if (isVisible) {
          sectionHasVisibleItems = true;
        }
      }
    });

    commitHeaderVisibility();
    return visibility;
  }

  function getGroupedItemSectionNames(items) {
    let currentSectionName = '';

    return items.map((item) => {
      if (item.kind === 'header') {
        currentSectionName = item.text;
        return currentSectionName;
      }

      return currentSectionName;
    });
  }

  globalObject.pageSearch = {
    normalizeSearchText,
    matchesSearchQuery,
    filterGroupedItems,
    getGroupedItemSectionNames
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      normalizeSearchText,
      matchesSearchQuery,
      filterGroupedItems,
      getGroupedItemSectionNames
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
