# lime-frontend

Node.js frontend for the LEMON manuals API with a Bootstrap 5 UI.

## Setup

```bash
npm install
```

## Run

```bash
LEMON_API_URL=http://localhost:8080 npm start
```

App runs at `http://localhost:3000` by default.

## LEMON API configuration

- `LEMON_API_URL` is the authoritative frontend setting for the backend base URL.
- `API_BASE_URL` is still accepted as a compatibility fallback, but prefer `LEMON_API_URL`.
- In local development, the server defaults to `http://localhost:8080` when no API URL is set.
- In CI, preview, and production-style environments, set `LEMON_API_URL` explicitly so the frontend does not guess a localhost backend.

Examples:

```bash
# local development
LEMON_API_URL=http://localhost:8080 npm start

# deployed/preview environment
LEMON_API_URL=https://lemon.example.com npm start
```

## Routes

- `/` make list
- `/:make` year list
- `/:make/:year` model/engine list
- `/:make/:year/:model` manual listing or manual content view

## API proxy routes

- `/api/makes`
- `/api/:make`
- `/api/:make/:year`
- `/api/:make/:year/:model`
- `/api/manual/*` (manual path passthrough)
