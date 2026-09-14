# LEMON - Car Manuals API Server

LEMON is a fast, efficient API server for browsing and accessing car manuals. Built with Rust using Axum web framework, it provides a clean JSON API for retrieving car make/model/year information and accessing technical documentation.

## 🏗️ Architecture

LEMON consists of two main components:

### Backend (This Repository)
- **Language**: Rust (85%)
- **Framework**: Axum web framework
- **Database**: LEMON/CHARM database engines
- **API**: RESTful JSON endpoints for navigation and manual retrieval

### Frontend
- **Repository**: [lime-frontend](https://github.com/ablakely/lime-frontend)
- **Language**: Node.js/JavaScript with Bootstrap UI
- **Features**: Interactive browsing, responsive design, manual viewer

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+
- LEMON/CHARM index.json files

### Running the Server

```bash
cargo build --release
./target/release/lime /path/to/index.json
```

The server starts on `http://localhost:8080`

### Configuration

```bash
# Listen on specific address
./lime --listen-address 0.0.0.0:3000

# Add custom site branding
./lime --site-name "My Car Manuals" --slogan "All manuals, all cars"

# Production mode (enable rate limiting)
./lime --production

# Multiple databases
./lime /path/to/lemon/index.json /path/to/charm/index.json

# Disable bundle downloads
./lime --disable-bundles /path/to/index.json
```

## 📡 API Endpoints

### Navigation (JSON API)
- `GET /` - List all car makes
- `GET /:make` - List years for a make
- `GET /:make/:year` - List models and engines for make/year
- `GET /:make/:year/:model/` - Manual page metadata (title, breadcrumbs)
- `GET /:make/:year/:model/index.html` - Full HTML manual page

### Downloads
- `GET /bundle/:make/:year/:model` - Download manual as ZIP
- `POST /bundle/:make/:year/:model` - Requires captcha verification

### Global Handlers
Database-specific endpoints (e.g., bundle listings, search)

## 📚 JSON Response Examples

### Get All Makes
```json
{
  "makes": ["Toyota", "Honda", "Ford", ...]
}
```

### Get Years for Make
```json
{
  "make": "Toyota",
  "years": ["2020", "2021", "2022", ...]
}
```

### Get Models for Make/Year
```json
{
  "make": "Toyota",
  "year": "2022",
  "models": [
    {
      "model": "Camry",
      "engines": [
        { "name": "2.5L", "uri": "/Toyota/2022/Camry/" },
        { "name": "Hybrid", "uri": "/Toyota/2022/Camry%20Hybrid/" }
      ]
    }
  ]
}
```

### Get Manual Page
```json
{
  "success": true,
  "data": {
    "title": "2022 Toyota Camry - Repair and Diagnosis",
    "breadcrumbs": [
      { "name": "Toyota", "path": "/Toyota" },
      { "name": "2022", "path": "/Toyota/2022" },
      { "name": "Camry", "path": "/Toyota/2022/Camry/" }
    ]
  }
}
```

## ⚙️ Rate Limiting

By default (non-production mode), rate limiting is disabled. In production:

- **General requests**: 1200 per IP per minute
- **Bundle downloads**: 20 per IP per hour (burst per hour)
- **Global bundle limit**: 50 concurrent downloads

Configure with CLI flags:
```bash
./lime \
  --production \
  --rate-limit-all-requests-per-ip-per-minute 1200 \
  --rate-limit-bundles-per-ip-per-hour 20 \
  --rate-limit-global-inflight-bundles 50 \
  /path/to/index.json
```

## 🔧 Features

- ✅ Multi-database support (LEMON, CHARM)
- ✅ JSON-first API for modern frontends
- ✅ Automatic bundle generation (.zip downloads)
- ✅ Rate limiting and DDoS protection
- ✅ Breadcrumb navigation
- ✅ Unix socket and TCP support
- ✅ Custom branding/theming
- ✅ Interactive mode on Windows

## 🌐 Frontend

The official frontend is available at [lime-frontend](https://github.com/ablakely/lime-frontend).

To use the frontend with this API:

```bash
# Terminal 1: Start the API server
./lime /path/to/index.json

# Terminal 2: Start the frontend
cd ../lime-frontend
npm install
npm start
```

Then visit `http://localhost:3000` in your browser.

## 📦 Project Structure

```
lime/
├── src/
│   ├── main.rs                 # Server entry point
│   ├── database_engines/       # Lemon & Charm engines
│   ├── indexing.rs            # Navigation tree builder
│   ├── json_responses.rs       # API response types
│   ├── uri_path.rs            # URI parsing & encoding
│   ├── types.rs               # Core types
│   ├── zipper.rs              # Bundle creation
│   └── ...
├── Cargo.toml
├── Cargo.lock
└── README.md
```

## 🛠️ Building from Source

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check code
cargo clippy
```

## 📋 CLI Arguments

```
--listen-address <HOST:PORT>
    Default: 0.0.0.0:8080
    Unix socket: unix:/path/to/socket

--site-name <NAME>
    Default: "LEMON Manuals"

--slogan <TEXT>
    Default: "Even more car manuals for everyone"

--production
    Enable rate limiting, X-Real-IP header support

--disable-bundles
    Disable .zip file downloads

--rate-limit-*
    Configure rate limiting thresholds

<INDEX_PATHS>
    Paths to index.json files from LEMON/CHARM databases
```

## 🔐 Security

- Rate limiting to prevent abuse
- Captcha on bundle downloads
- Automatic cleanup of temporary files
- Worker-based architecture for isolation
- Support for reverse proxy setup

## 🤝 Contributing

Contributions welcome! Please ensure:
- Code passes `cargo clippy`
- Tests pass with `cargo test`
- Follow existing code style
- Document new features

## 📄 License

See LICENSE file in repository

## 🐛 Issues & Support

For bug reports and feature requests, use the GitHub issues tracker.

## 🚀 Deployment

### Docker

```dockerfile
FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/lime /usr/local/bin/
EXPOSE 8080
CMD ["lime", "/data/index.json"]
```

### Systemd Service

```ini
[Unit]
Description=LEMON Manuals API Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/lime --production /var/lib/lemon/index.json
Restart=on-failure
User=lemon
Group=lemon

[Install]
WantedBy=multi-user.target
```

## 📞 Support

- **API Issues**: Check `/API.md` for detailed endpoint documentation
- **Frontend Issues**: See [lime-frontend](https://github.com/ablakely/lime-frontend) repository
- **General Questions**: Open an issue or discussion

---

Built with ❤️ using Rust and Axum
