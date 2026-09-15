async function fetchJson(path) {
  const response = await fetch(path);
  const data = await response.json().catch(() => ({}));

  if (!response.ok) {
    const message = data.error || `Request failed (${response.status})`;
    throw new Error(data.details ? `${message}: ${data.details}` : message);
  }

  return data;
}

async function fetchManual(path) {
  const response = await fetch(path);
  const parsedResponse = await window.manualResponse.parseManualApiResponse(response);

  if (
    parsedResponse.kind === 'directory' &&
    Array.isArray(parsedResponse.data.manuals) &&
    parsedResponse.data.manuals.length === 0
  ) {
    const indexPath = path.endsWith('/') ? `${path}index.html` : `${path}/index.html`;
    const indexResponse = await fetch(indexPath);

    if (indexResponse.ok) {
      return window.manualResponse.parseManualApiResponse(indexResponse);
    }
  }

  return parsedResponse;
}

window.lemonApi = {
  getHtml: (uri) => fetchManual(uri),
  getMakes: () => fetchJson('/api/makes'),
  getYears: (make) => fetchJson(`/api/${encodeURIComponent(make)}`),
  getModels: (make, year) => fetchJson(`/api/${encodeURIComponent(make)}/${encodeURIComponent(year)}`),
  getManual: (make, year, model) =>
    fetchJson(`/api/${encodeURIComponent(make)}/${encodeURIComponent(year)}/${encodeURIComponent(model)}`),
  getManualPath: (segments) => fetchManual(`/api/manual/${segments.map((segment) => encodeURIComponent(segment)).join('/')}`),
  getManualPathFromRawSegments: (rawSegments) => fetchManual(`/api/manual/${rawSegments.join('/')}`)
};
