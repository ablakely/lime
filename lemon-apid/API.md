# API

This server exposes JSON by default for navigation and manual discovery routes.

## Navigation JSON

### `GET /`

Returns all makes.

```json
{
  "makes": [
    { "name": "Buick", "uri": "/Buick/" }
  ]
}
```

### `GET /:make/`

Returns all years for a make.

```json
{
  "years": [
    { "year": "2012", "uri": "/Buick/2012/" }
  ]
}
```

### `GET /:make/:year/`

Returns all models for a make/year.

- `uri` is present when the model has exactly one direct manual root.
- `engines[*].uri` always contains the manual root URL to use next.

```json
{
  "models": [
    {
      "model": "LaCrosse",
      "uri": null,
      "engines": [
        {
          "name": "Leather, 3.6L Eng VIN 3",
          "uri": "/Buick/2012/LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203/"
        }
      ]
    }
  ]
}
```

## Manual JSON

### `GET /:make/:year/:model_or_engine/.../`

Returns JSON for a manual page/section.

- `manuals` contains descendant manual links listed from the current page, which is useful for drilling down from paths such as `/Buick/2012/LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203/Repair%20and%20Diagnosis/`.
- `manuals` includes navigation links discovered from both absolute and relative `<a href>` values, including LEMON split-tree navigation pages.

```json
{
  "title": "Repair and Diagnosis",
  "breadcrumbs": [
    { "label": "Buick", "href": "/Buick/" }
  ],
  "topics": ["Repair and Diagnosis"],
  "content": "<p>Manual page HTML content</p>",
  "manuals": [
    {
      "name": "Engine",
      "uri": "/Buick/2012/LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203/Repair%20and%20Diagnosis/Engine/"
    }
  ]
}
```

## Manual HTML

### `GET /:make/:year/:model_or_engine/.../index.html`

Returns the HTML version of the same manual page.

## Notes

- Static assets such as `/about.html`, `/style.css`, and images continue to work as before.
- Bundle download routes remain unchanged.
