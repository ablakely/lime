# lime

`lime` is a replacement frontend for LEMON/CHARM that adds quality-of-life features such as page filtering and a faster, easier UI/UX.

## Data sources

This project uses manual datasets from:

- **LEMON (Liberated Excellent Manuals ONline)**  
  The largest collection of free car repair manuals.  
  https://lemon-manuals.la

- **Operation CHARM**  
  The Collection of High-quality Auto Repair Manuals covers many makes and models from 1982 through 2013.  
  CHARM emphasizes the right to repair and publishes its data/code archive publicly.  
  https://charm.li

## Repository layout

- `lemon-apid` — modified LEMON Rust webserver used as a JSON REST API
- `lime-frontend` — frontend web app written in node.js

## lime-frontend

The frontend for this project lives in a separate repository and is included here as a git submodule.

- Repository: https://github.com/ablakely/lime-frontend
- Local path in this repo: `lime-frontend`

## Getting started

1. Clone the repository.
2. Initialize submodules:
   - `git submodule update --init --recursive`
3. Build and run the backend:
   - `cd lemon-apid`
   - `cargo run -- /path/to/index.json`

You can pass multiple `index.json` paths to enable multiple manual databases.

## Notes on manual navigation

Manual navigation pages may look repetitive: different URLs can point to the same content.  
On CHARM-style pages, blue overlays on images may be clickable links.
