(function manualImagesSetup(globalObject) {

  function getImageOpenUrl(image) {
    if (!image) {
      return '';
    }

    const attributeSource = typeof image.getAttribute === 'function' ? image.getAttribute('src') : '';
    return image.currentSrc || image.src || attributeSource || '';
  }

  globalObject.manualImages = {
    getImageOpenUrl
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      getImageOpenUrl
    };
  }
})(typeof window !== 'undefined' ? window : globalThis);
