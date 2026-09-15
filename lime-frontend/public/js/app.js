const content = document.getElementById('content');
const loading = document.getElementById('loading');
const message = document.getElementById('message');
const searchEmptyState = document.getElementById('search-empty-state');
const breadcrumbs = document.getElementById('breadcrumbs');
const pageSearchInput = document.getElementById('page-search');
const quickNavForm = document.getElementById('quick-nav-form');
const quickNavSection = document.getElementById('quick-nav-section');
const toggleQuickNavButton = document.getElementById('toggle-quick-nav');
const quickHistory = document.getElementById('quick-history');
const clearQuickHistoryButton = document.getElementById('clear-quick-history');
const quickMake = document.getElementById('quick-make');
const quickYear = document.getElementById('quick-year');
const quickModel = document.getElementById('quick-model');
const quickMakeMenu = document.getElementById('quick-make-menu');
const quickYearMenu = document.getElementById('quick-year-menu');
const quickModelMenu = document.getElementById('quick-model-menu');

let cachedMakes = [];
let makesHydrationPromise = null;
let quickNavHistoryEntries = [];
let cachedYears = [];
let cachedModels = [];

const quickNavAutocompleteFields = {
  make: { input: quickMake, menu: quickMakeMenu, getOptions: () => cachedMakes },
  year: { input: quickYear, menu: quickYearMenu, getOptions: () => cachedYears },
  model: { input: quickModel, menu: quickModelMenu, getOptions: () => cachedModels }
};

function updateQuickNavToggleState(isHidden) {
  toggleQuickNavButton.setAttribute('aria-pressed', String(isHidden));
  toggleQuickNavButton.setAttribute('aria-label', isHidden ? 'Show manual finder' : 'Hide manual finder');
  toggleQuickNavButton.setAttribute('title', isHidden ? 'Show manual finder' : 'Hide manual finder');
}

function syncQuickNavForRoute(parts) {
  if (!toggleQuickNavButton || !quickNavSection) {
    return;
  }

  const showQuickNavByDefault = window.routeUtils.shouldShowQuickNavByDefault(parts);
  quickNavSection.classList.toggle('d-none', !showQuickNavByDefault);
  toggleQuickNavButton.classList.remove('d-none');
  updateQuickNavToggleState(!showQuickNavByDefault);
  resetQuickNavForm();
}

function showLoading(show) {
  loading.classList.toggle('d-none', !show);
}

function showMessage(text, level = 'danger') {
  if (!text) {
    message.className = 'alert d-none';
    message.textContent = '';
    return;
  }

  message.className = `alert alert-${level}`;
  message.textContent = text;
}

function decodeHtmlEntities(text) {
  if (typeof text !== 'string' || !text.includes('&')) {
    return text || '';
  }

  const textarea = document.createElement('textarea');
  textarea.innerHTML = text;
  return textarea.value;
}

function setBreadcrumbs(parts) {
  breadcrumbs.innerHTML = '';
  const crumbs = [{ label: 'Home', path: '/' }, ...parts];

  crumbs.forEach((crumb, index) => {
    const item = document.createElement('li');
    item.className = 'breadcrumb-item';

    const isLast = index === crumbs.length - 1;
    if (isLast) {
      item.classList.add('active');
      item.setAttribute('aria-current', 'page');
      item.textContent = crumb.label;
    } else {
      const link = document.createElement('a');
      link.href = crumb.path;
      link.textContent = crumb.label;
      item.appendChild(link);
    }

    breadcrumbs.appendChild(item);
  });
}

function cardList(items) {
  const row = document.createElement('div');
  row.className = 'row g-3 card-list';

  items.forEach(({ title, href, subtitle, badge }) => {
    const col = document.createElement('div');
    col.className = 'col-12 col-md-6 col-lg-4';
    const link = document.createElement('a');
    link.href = href;

    const card = document.createElement('div');
    card.className = 'card h-100 shadow-sm';

    const body = document.createElement('div');
    body.className = 'card-body';

    const heading = document.createElement('h2');
    heading.className = 'h5 card-title mb-1';
    heading.textContent = title;

    if (badge) {
        const b = document.createElement('span');
        if (badge == 'lemon') {
            b.className = 'badge rounded-pill bg-warning';
        } else {
            b.className = 'badge rounded-pill bg-info';
        }
        b.textContent = badge;
        heading.appendChild(b)
    }
    body.appendChild(heading);


    if (subtitle) {
      const note = document.createElement('p');
      note.className = 'text-muted small mb-0';
      note.textContent = subtitle;
      body.appendChild(note);
    }

    card.appendChild(body);
    link.appendChild(card);
    col.appendChild(link);
    row.appendChild(col);
  });

  return row;
}

function renderEmptyState(text) {
  const notice = document.createElement('div');
  notice.className = 'alert alert-light border mb-0';
  notice.textContent = text;
  return notice;
}

function setSearchEmptyState(text) {
  if (!searchEmptyState) {
    return;
  }

  searchEmptyState.textContent = text || '';
  searchEmptyState.classList.toggle('d-none', !text);
}

function toggleElementVisibility(element, isVisible) {
  element.classList.toggle('d-none', !isVisible);
}

function setGroupedItemSectionContext(element, sectionName, showSectionName) {
  if (!(element instanceof HTMLElement) || element.tagName !== 'A') {
    return;
  }

  const itemLabel = element.dataset.itemLabel || element.textContent || '';
  element.dataset.itemLabel = itemLabel;

  if (!showSectionName || !sectionName) {
    element.textContent = itemLabel;
    return;
  }

  element.replaceChildren();

  const title = document.createElement('span');
  title.textContent = itemLabel;
  element.appendChild(title);

  const context = document.createElement('span');
  context.className = 'd-block text-muted small';
  context.textContent = sectionName;
  element.appendChild(context);
}

function applyCardListSearch(query) {
  let totalItems = 0;
  let visibleItems = 0;

  content.querySelectorAll('.card-list').forEach((list) => {
    Array.from(list.children).forEach((item) => {
      const isVisible = window.pageSearch.matchesSearchQuery(item.textContent, query);
      toggleElementVisibility(item, isVisible);
      totalItems += 1;
      if (isVisible) {
        visibleItems += 1;
      }
    });
  });

  return { totalItems, visibleItems };
}

function applyGroupedListSearch(query) {
  let totalItems = 0;
  let visibleItems = 0;

  content.querySelectorAll('.list-group').forEach((list) => {
    const children = Array.from(list.children);
    const definitions = children.map((child) => ({
      kind: child.tagName === 'LI' && child.classList.contains('list-group-item-dark') ? 'header' : 'item',
      text: child.dataset.itemLabel || child.textContent
    }));
    const visibility = window.pageSearch.filterGroupedItems(definitions, query);
    const sectionNames = window.pageSearch.getGroupedItemSectionNames(definitions);
    const hasQuery = window.pageSearch.normalizeSearchText(query).length > 0;

    children.forEach((child, index) => {
      const isHeader = definitions[index].kind === 'header';
      const isVisible = visibility[index];
      toggleElementVisibility(child, isVisible);

      if (!isHeader) {
        setGroupedItemSectionContext(child, sectionNames[index], hasQuery && isVisible);
      }

      if (!isHeader) {
        totalItems += 1;
        if (isVisible) {
          visibleItems += 1;
        }
      }
    });
  });

  return { totalItems, visibleItems };
}

function applyPageSearch() {
  if (!pageSearchInput || !window.pageSearch) {
    return;
  }

  const query = pageSearchInput.value;
  const results = [applyCardListSearch(query), applyGroupedListSearch(query)];
  const totalItems = results.reduce((sum, result) => sum + result.totalItems, 0);
  const visibleItems = results.reduce((sum, result) => sum + result.visibleItems, 0);
  const hasQuery = window.pageSearch.normalizeSearchText(query).length > 0;

  if (hasQuery && totalItems > 0 && visibleItems === 0) {
    setSearchEmptyState(`No items on this page match "${query.trim()}".`);
    return;
  }

  setSearchEmptyState('');
}

function renderManualListing(data, rawParts) {
  const section = document.createElement('section');
  section.className = 'card shadow-sm';

  const body = document.createElement('div');
  body.className = 'card-body';

  const heading = document.createElement('h1');
  heading.className = 'h3 mb-3';
  heading.textContent = decodeHtmlEntities(data.title || 'Manuals');
  body.appendChild(heading);

  if (Array.isArray(data.topics) && data.topics.length > 0) {
    const topics = document.createElement('div');
    topics.className = 'd-flex flex-wrap gap-2 mb-3';

    data.topics.forEach((topic) => {
      const badge = document.createElement('span');
      badge.className = 'badge text-bg-light border';
      badge.textContent = decodeHtmlEntities(topic);
      topics.appendChild(badge);
    });

    body.appendChild(topics);
  }

  if (Array.isArray(data.manuals) && data.manuals.length > 0) {
    const list = document.createElement('div');
    list.className = 'list-group';

    const sectionedManuals = window.manualSections
      ? window.manualSections.getSectionedManuals(data.manuals)
      : [];

    sectionedManuals.forEach((entry) => {
      if (entry.kind === 'header') {
        const header = document.createElement('li');
        header.className = 'list-group-item list-group-item-dark';
        header.textContent = entry.text;
        list.appendChild(header);
        return;
      }

      const link = document.createElement('a');
      link.className = 'list-group-item list-group-item-action';
      link.href = entry.uri;
      link.dataset.itemLabel = decodeHtmlEntities(entry.name || entry.uri);
      setGroupedItemSectionContext(link, entry.section, false);
      list.appendChild(link);
    });

    body.appendChild(list);
  } else {
    if (data.content) {
        body.appendChild(renderHtml(data.content, rawParts));
    } else {
        body.appendChild(renderEmptyState('No manuals were found for this path.'));
    }
  }

  section.appendChild(body);
  return section;
}

function renderiFrame(uri) {
    const iframe = document.createElement('iframe');
    iframe.src = uri;
    return iframe;
}

function renderHtml(html, rawParts) {
    const article = document.createElement('article');

    const $html = $(html);

    $html.find('table').addClass('table table-striped');
    $html.find('img').each(function() {
        const $img = $(this);
        const currentSrc = $img.attr('src');

        if (currentSrc) {
            $img.attr('src', currentSrc.replace('/api/manual/', ''));
        }
    });

    article.className = 'manual-content';
    article.innerHTML = $html.html();

    return article;
}

function renderManualHtml(html, rawParts) {
  const article = document.createElement('article');
  const $page = $('<div>').html(html).find('.main');

  article.className = 'manual-content';
  $page.find('table').addClass('table table-striped');
 
  $page.find('img').each(function() {
    const $img = $(this);
    const currentSrc = $img.attr('src');
    
    if (currentSrc) {
      $img.attr('src', currentSrc.replace('/api/manual/', ''));
    }
  });

  article.innerHTML = $page.html();
  return article;
}

function normalizeArrayPayload(data) {
  if (!data || typeof data !== 'object') {
    return [];
  }

  if (Array.isArray(data)) {
    return data;
  }

  const candidates = ['makes', 'years', 'models', 'items', 'results', 'data'];
  for (const key of candidates) {
    if (Array.isArray(data[key])) {
      return data[key];
    }
  }

  return [];
}

function clearDataList(element) {
  if (element === 'years') {
    cachedYears = [];
    renderAutocompleteSuggestions('year');
    return;
  }

  if (element === 'models') {
    cachedModels = [];
    renderAutocompleteSuggestions('model');
  }
}

function renderQuickNavHistory() {
  if (!quickHistory) {
    return;
  }

  quickHistory.innerHTML = '';

  const placeholder = document.createElement('option');
  placeholder.value = '';
  placeholder.textContent = quickNavHistoryEntries.length > 0 ? 'Choose a recent car' : 'No recent cars yet';
  quickHistory.appendChild(placeholder);

  quickNavHistoryEntries.forEach((entry, index) => {
    const option = document.createElement('option');
    option.value = String(index);
    option.textContent = window.carHistory.toHistoryLabel(entry);
    quickHistory.appendChild(option);
  });

  quickHistory.disabled = quickNavHistoryEntries.length === 0;
  quickHistory.value = '';
  if (clearQuickHistoryButton) {
    clearQuickHistoryButton.disabled = quickNavHistoryEntries.length === 0;
  }
}

function loadQuickNavHistory() {
  if (!window.carHistory) {
    return;
  }

  quickNavHistoryEntries = window.carHistory.loadHistory(window.localStorage);
  renderQuickNavHistory();
}

function saveQuickNavHistory(selection) {
  if (!window.carHistory) {
    return;
  }

  quickNavHistoryEntries = window.carHistory.saveSelection(window.localStorage, selection);
  renderQuickNavHistory();
}

function clearQuickNavHistory() {
  if (!window.carHistory) {
    return;
  }

  quickNavHistoryEntries = window.carHistory.clearHistory(window.localStorage);
  renderQuickNavHistory();
}

function extractMakes(data) {
  return normalizeArrayPayload(data)
    .map((make) => {
      if (typeof make === 'string') {
        return make;
      }

      if (!make || typeof make !== 'object') {
        return '';
      }

      return make.make || make.name || make.label || make.value || '';
    })
    .filter(Boolean);
}

function extractYears(data) {
  return normalizeArrayPayload(data)
    .map((year) => {
      if (typeof year === 'number' || typeof year === 'string') {
        return String(year);
      }
      return year.year != null ? String(year.year) : '';
    })
    .filter(Boolean);
}

function extractModels(data) {
  const models = [];
  normalizeArrayPayload(data).forEach((entry) => {
    if (typeof entry === 'string') {
      models.push(entry);
      return;
    }

    if (entry.model && Array.isArray(entry.engines) && entry.engines.length > 0) {
      entry.engines.forEach((engine) => {
        const engineName = String((engine && engine.name) || '').trim();
        const modelName = String(entry.model).trim();
        models.push(engineName ? `${modelName} ${engineName}`.trim() : modelName);
      });
      return;
    }

    if (entry.model) {
      models.push(entry.model);
    }
  });
  return models.filter(Boolean);
}

async function hydrateMakesIfNeeded() {
  if (cachedMakes.length > 0) {
    return cachedMakes;
  }

  if (!makesHydrationPromise) {
    makesHydrationPromise = (async () => {
      try {
        cachedMakes = window.quickNavAutocomplete.uniqueOptions(extractMakes(await window.lemonApi.getMakes()));
        return cachedMakes;
      } catch (error) {
        cachedMakes = [];
        throw new Error(`Unable to load makes for autocomplete. ${error.message}`);
      } finally {
        makesHydrationPromise = null;
      }
    })();
  }

  return makesHydrationPromise;
}

async function refreshYearOptions() {
  const make = quickMake.value.trim();
  clearDataList('years');
  clearDataList('models');
  quickYear.value = '';
  quickModel.value = '';
  if (!make) {
    return;
  }
  const years = extractYears(await window.lemonApi.getYears(make));
  cachedYears = window.quickNavAutocomplete.uniqueOptions(years);
  renderAutocompleteSuggestions('year');
}

async function refreshModelOptions() {
  const make = quickMake.value.trim();
  const year = quickYear.value.trim();
  clearDataList('models');
  quickModel.value = '';
  if (!make || !year) {
    return;
  }
  const models = extractModels(await window.lemonApi.getModels(make, year));
  cachedModels = window.quickNavAutocomplete.uniqueOptions(models);
  renderAutocompleteSuggestions('model');
}

function buildQuickNavPath(selection) {
  const make = String(selection.make || '').trim();
  const year = String(selection.year || '').trim();
  const model = String(selection.model || '').trim();

  if (model) {
    return `/${encodeURIComponent(make)}/${encodeURIComponent(year)}/${encodeURIComponent(model)}`;
  }

  if (year) {
    return `/${encodeURIComponent(make)}/${encodeURIComponent(year)}`;
  }

  return `/${encodeURIComponent(make)}`;
}

function hideAutocompleteMenu(menu) {
  if (!menu) {
    return;
  }

  menu.classList.add('d-none');
  menu.innerHTML = '';
}

function hideAllAutocompleteMenus() {
  Object.values(quickNavAutocompleteFields).forEach(({ menu }) => hideAutocompleteMenu(menu));
}

function renderAutocompleteSuggestions(fieldName) {
  const field = quickNavAutocompleteFields[fieldName];
  if (!field || !field.menu || !window.quickNavAutocomplete) {
    return;
  }

  const matches = window.quickNavAutocomplete.filterOptions(field.getOptions(), field.input.value);
  hideAutocompleteMenu(field.menu);

  if (matches.length === 0) {
    return;
  }

  matches.forEach((match) => {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'list-group-item list-group-item-action';
    button.dataset.value = match;
    button.textContent = match;
    field.menu.appendChild(button);
  });

  field.menu.classList.remove('d-none');
}

function resetQuickNavForm() {
  showMessage('');
  hideAllAutocompleteMenus();
  quickHistory.value = '';
  quickMake.value = '';
  quickYear.value = '';
  quickModel.value = '';
  clearDataList('years');
  clearDataList('models');
}

function navigateQuickNav(selection) {
  const make = String(selection.make || '').trim();
  const year = String(selection.year || '').trim();
  const model = String(selection.model || '').trim();

  if (!make) {
    showMessage('Select a make before navigating.');
    return;
  }

  if (model && !year) {
    showMessage('Select a year before selecting a model.');
    return;
  }

  saveQuickNavHistory({ make, year, model });
  const path = buildQuickNavPath({ make, year, model });
  resetQuickNavForm();
  window.history.pushState({}, '', path);
  renderRoute();
}

function selectAutocompleteSuggestion(fieldName, value) {
  const field = quickNavAutocompleteFields[fieldName];
  if (!field) {
    return;
  }

  field.input.value = value;
  hideAutocompleteMenu(field.menu);

  if (fieldName === 'make') {
    refreshYearOptions().catch((error) => showMessage(error.message));
    return;
  }

  if (fieldName === 'year') {
    refreshModelOptions().catch((error) => showMessage(error.message));
  }
}

function registerAutocompleteField(fieldName) {
  const field = quickNavAutocompleteFields[fieldName];
  if (!field || !field.input || !field.menu) {
    return;
  }

  field.input.addEventListener('focus', () => renderAutocompleteSuggestions(fieldName));
  field.input.addEventListener('input', () => renderAutocompleteSuggestions(fieldName));
  field.input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      hideAutocompleteMenu(field.menu);
    }
  });

  field.menu.addEventListener('mousedown', (event) => {
    event.preventDefault();
  });
  field.menu.addEventListener('click', (event) => {
    const suggestion = event.target.closest('button[data-value]');
    if (!suggestion) {
      return;
    }

    selectAutocompleteSuggestion(fieldName, suggestion.dataset.value || '');
  });
}

async function renderRoute() {
  const { rawParts, decodedParts: parts } = window.routeUtils.parsePathname(window.location.pathname);
  syncQuickNavForRoute(parts);
  showMessage('');
  showLoading(true);
  content.innerHTML = '';
  $("#page-search").val('');
  applyPageSearch();

  try {
    if (parts.length === 0) {
      setBreadcrumbs([]);
      const makes = (await hydrateMakesIfNeeded()).map((make) => ({ make }));
      
      content.appendChild(
        cardList(
          makes.map(({ make }) => ({
            title: make,
            href: `/${encodeURIComponent(make)}`
          }))
        )
      );
      return;
    }

    if (parts.length === 1) {
      const [make] = parts;
      setBreadcrumbs([{ label: make, path: `/${encodeURIComponent(make)}` }]);
      const data = await window.lemonApi.getYears(make);
      const years = extractYears(data).map((year) => ({ year })).reverse();
      content.appendChild(
        cardList(
          years.map(({ year }) => ({
            title: String(year),
            href: `/${encodeURIComponent(make)}/${encodeURIComponent(year)}`
          }))
        )
      );
      return;
    }

    if (parts.length === 2) {
      const [make, year] = parts;
      setBreadcrumbs([
        { label: make, path: `/${encodeURIComponent(make)}` },
        { label: year, path: `/${encodeURIComponent(make)}/${encodeURIComponent(year)}` }
      ]);
      const data = await window.lemonApi.getModels(make, year);
      const models = normalizeArrayPayload(data);
      const modelCards = [];

      models.forEach((entry) => {
        if (typeof entry === 'string') {
          modelCards.push({
            title: entry,
            href: `/${encodeURIComponent(make)}/${encodeURIComponent(year)}/${encodeURIComponent(entry)}`
          });
          return;
        }

        if (entry.model && Array.isArray(entry.engines) && entry.engines.length > 0) {
          entry.engines.forEach((engine) => {
            const subtitle = engine.name || entry.model;
            const title = entry.model !== subtitle ? entry.model : ''; 

            modelCards.push({
              subtitle: subtitle,
              badge: engine.database,
              title: title,
              href: `/${encodeURIComponent(make)}/${encodeURIComponent(year)}/${encodeURIComponent(title + ' ' + subtitle)}`
            });
          });
          return;
        }

        if (entry.model) {
          modelCards.push({
            title: entry.model,
            href: `/${encodeURIComponent(make)}/${encodeURIComponent(year)}/${encodeURIComponent(entry.model)}`
          });
        }
      });

      content.appendChild(cardList(modelCards));
      return;
    }

    const manualBreadcrumbs = parts.map((label, index) => ({
      label,
      path: `/${rawParts.slice(0, index + 1).join('/')}`
    }));

    const data = await window.lemonApi.getManualPathFromRawSegments(rawParts);

    if (data.kind === 'directory') {
      const apiBreadcrumbs = Array.isArray(data.data.breadcrumbs) ? data.data.breadcrumbs : [];
      setBreadcrumbs(
        apiBreadcrumbs.map((crumb) => ({
          label: decodeHtmlEntities(crumb.label),
          path: crumb.href
        }))
      );
      content.appendChild(renderManualListing(data.data, rawParts));
      return;
    }

    setBreadcrumbs(manualBreadcrumbs);

    if (data.kind === 'html') {
      content.appendChild(renderManualHtml(data.data, rawParts));
      return;
    }

    const pre = document.createElement('pre');
    pre.className = 'manual-content';
    pre.textContent = data.kind === 'json' ? JSON.stringify(data.data, null, 2) : data.data;
    content.appendChild(pre);
  } catch (error) {
    showMessage(error.message);
  } finally {
    showLoading(false);
    applyPageSearch();
  }
}

window.addEventListener('click', (event) => {
  const image = event.target.closest('.manual-content img[src]');
  if (image) {
    const imageUrl = window.manualImages.getImageOpenUrl(image);
    if (imageUrl) {
      event.preventDefault();
      window.open(imageUrl, '_blank', 'noopener');
    }
    return;
  }

  const link = event.target.closest('a[href^="/"]');
  if (!link || link.target === '_blank' || link.hasAttribute('download')) {
    return;
  }

  if (event.target.closest('.card-list')) {
    const destinationPath = new URL(link.href, window.location.origin).pathname;
    const { decodedParts } = window.routeUtils.parsePathname(destinationPath);
    if (decodedParts.length === 3) {
      const [make = '', year = '', model = ''] = decodedParts;
      if (model.trim()) {
        saveQuickNavHistory({ make, year, model });
      }
    }
  }

  event.preventDefault();
  window.history.pushState({}, '', link.getAttribute('href'));
  renderRoute();
});

window.addEventListener('popstate', renderRoute);

function debounce(fn, delayMs = 200) {
  let debounceTimeoutId = null;
  return (...args) => {
    window.clearTimeout(debounceTimeoutId);
    debounceTimeoutId = window.setTimeout(() => fn(...args), delayMs);
  };
}

const refreshYearOptionsOnInput = debounce(() => {
  const value = quickMake.value.trim();
  if (!cachedMakes.includes(value)) {
    clearDataList('years');
    clearDataList('models');
    quickYear.value = '';
    quickModel.value = '';
    return;
  }
  refreshYearOptions().catch((error) => showMessage(error.message));
});

const refreshModelOptionsOnInput = debounce(() => {
  const selectedYear = quickYear.value.trim();
  if (!cachedYears.includes(selectedYear)) {
    clearDataList('models');
    quickModel.value = '';
    return;
  }
  refreshModelOptions().catch((error) => showMessage(error.message));
});

quickMake.addEventListener('change', () => {
  refreshYearOptions().catch((error) => showMessage(error.message));
});
quickMake.addEventListener('input', refreshYearOptionsOnInput);
quickYear.addEventListener('change', () => {
  refreshModelOptions().catch((error) => showMessage(error.message));
});
quickYear.addEventListener('input', refreshModelOptionsOnInput);
quickNavForm.addEventListener('submit', (event) => {
  event.preventDefault();
  navigateQuickNav({
    make: quickMake.value,
    year: quickYear.value,
    model: quickModel.value
  });
});

if (quickHistory) {
  quickHistory.addEventListener('change', () => {
    if (!quickHistory.value) {
      return;
    }

    const entry = quickNavHistoryEntries[Number.parseInt(quickHistory.value, 10)];
    if (entry) {
      navigateQuickNav(entry);
    }
  });
}

if (toggleQuickNavButton && quickNavSection) {
  toggleQuickNavButton.addEventListener('click', () => {
    const isHidden = quickNavSection.classList.toggle('d-none');
    updateQuickNavToggleState(isHidden);
    resetQuickNavForm();
  });
}

if (clearQuickHistoryButton) {
  clearQuickHistoryButton.addEventListener('click', () => {
    clearQuickNavHistory();
    resetQuickNavForm();
  });
}

if (pageSearchInput) {
  pageSearchInput.addEventListener('input', applyPageSearch);
}

Object.keys(quickNavAutocompleteFields).forEach(registerAutocompleteField);

window.addEventListener('click', (event) => {
  if (event.target.closest('.quick-nav-autocomplete')) {
    return;
  }

  hideAllAutocompleteMenus();
});

async function initializeApp() {
  if (typeof feather !== 'undefined') {
    feather.replace();
  }

  loadQuickNavHistory();

  let hydrationErrorMessage = '';
  try {
    await hydrateMakesIfNeeded();
  } catch (error) {
    hydrationErrorMessage = error.message;
  }

  await renderRoute();

  if (hydrationErrorMessage && !message.textContent) {
    showMessage(hydrationErrorMessage, 'warning');
  }
}

initializeApp();
