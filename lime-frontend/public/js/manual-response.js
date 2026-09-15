(function manualResponseSetup(globalObject) {
  function parseJsonBody(body) {
    try {
      return JSON.parse(body);
    } catch (_error) {
      return null;
    }
  }

  function isManualDirectoryPayload(data) {
    return Boolean(
      data &&
      typeof data === 'object' &&
      !Array.isArray(data) &&
      typeof data.title === 'string' &&
      Array.isArray(data.breadcrumbs) &&
      Array.isArray(data.topics) &&
      Array.isArray(data.manuals)
    );
  }

  function looksLikeHtml(body) {
    return /^\s*<!doctype html\b/i.test(body) || /^\s*<html\b/i.test(body) || /^\s*<(article|div|h1|p|section|table)\b/i.test(body);
  }

  async function parseManualApiResponse(response) {
    const contentType = response.headers.get('content-type') || '';
    const body = await response.text();

    if (!response.ok) {
      if (contentType.includes('application/json')) {
        const data = parseJsonBody(body);
        if (data && typeof data === 'object') {
          const message = data.error || `Request failed (${response.status})`;
          throw new Error(data.details ? `${message}: ${data.details}` : message);
        }
      }

      throw new Error(body ? `Request failed (${response.status}): ${body}` : `Request failed (${response.status})`);
    }

    if (contentType.includes('application/json')) {
      const data = parseJsonBody(body);

      if (!data) {
        throw new Error('Manual response returned invalid JSON.');
      }

      return {
        kind: isManualDirectoryPayload(data) ? 'directory' : 'json',
        data
      };
    }

    if (contentType.includes('text/html') || looksLikeHtml(body)) {
      return { kind: 'html', data: body };
    }

    return { kind: 'text', data: body };
  }

  globalObject.manualResponse = {
    isManualDirectoryPayload,
    parseManualApiResponse
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      isManualDirectoryPayload,
      parseManualApiResponse
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
