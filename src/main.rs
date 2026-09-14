mod common;
mod database_engines;
mod indexing;
mod json_responses;
mod kv_store;
mod not_found_layer_adapter;
mod types;
mod uri_path;
mod zipper;

use std::fmt::Debug;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{io::Read, path::PathBuf};

use anyhow::Result;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use clap::Parser;
use governor::clock::Clock;
use tokio::net::TcpListener;

use crate::common::{
    SiteBranding, car_uri_components_to_human_readable_file_name, make_writer_to_bytes_stream,
};
use crate::database_engines::DatabaseEngine;
use crate::database_engines::charm::Charm;
use crate::database_engines::lemon::Lemon;
use crate::indexing::Indices;
use crate::json_responses::{ApiResponse, ErrorResponse, MakeResponse, RootResponse};
use crate::not_found_layer_adapter::NotFoundLayerAdapter;
use crate::types::{IndexJsonCommon, Make, Year};
use crate::uri_path::{CanonicalUriPath, ServerUriPath, UriPath, parse_uri_path};

#[derive(Parser, Debug)]
struct CliArgs {
    /// What IP address and port number to listen for incoming
    /// connections on. If it starts with unix:, then the remainder is
    /// interpreted as a path to a unix socket.
    #[arg(
        short,
        long,
        value_name = "listen_host:listen_port",
        default_value = "0.0.0.0:8080"
    )]
    listen_address: String,

    /// Title to display. This won't fully change the site name
    /// everywhere but will be enough to make the site look a bit more
    /// like your own.
    #[arg(long, default_value = "LEMON Manuals")]
    site_name: String,

    #[arg(long, default_value = "Even more car manuals for everyone")]
    slogan: String,

    #[arg(long, default_value = "scientia non olet")]
    latin_phrase: String,

    /// When provided, display a loud obnoxious banner with your custom message. HTML is allowed.
    #[arg(long)]
    announcement_html: Option<String>,

    /// Disable .zip downloads (bundles). This might be needed if you
    /// are using a mechanical hard disk.
    #[arg(long, action)]
    disable_bundles: bool,

    /// All HTTP requests are subject to this rate limit. The rate
    /// limit is allowed to burst up to the given value too; ie, a
    /// user can make 1000 requests in one second without error, then
    /// be banned for the next 59 seconds. Keep in mind that some
    /// pages have many images (upwards of 100); make sure the value
    /// is high enough to support these.
    #[arg(long, default_value = "1200")]
    rate_limit_all_requests_per_ip_per_minute: u32,

    /// Only .zip downloads (bundles) are subject to this rate
    /// limit. The bursting is also per-hour, so if this is set to
    /// 100, someone can download 100 bundles immediately then must
    /// wait one hour.
    #[arg(long, default_value = "20")]
    rate_limit_bundles_per_ip_per_hour: u32,

    /// Applies globally, NOT per-IP! When many users are downloading
    /// bundles slowly, it can consume up to about 128MiB of RAM per
    /// bundle. Set this low enough so you don't run out of RAM. It
    /// can also help prevent bundle downloads from using up all your
    /// storage IOPS and starving the non-bundle users.
    #[arg(long, default_value = "50")]
    rate_limit_global_inflight_bundles: usize,

    /// When set: Read IP from X-Real-IP header, enable rate limiting,
    /// and set cache control header. You should only enable this when
    /// LEMON is behind a reverse proxy that properly sets the
    /// X-Real-IP header; otherwise, everyone's IP will be grouped
    /// under the same "unspecified" address, causing the per-IP
    /// ratelimits to apply globally, crippling the website (LEMON
    /// does not attempt to read the connecting IP from the TCP socket).
    #[arg(long, action)]
    production: bool,

    /// Provide paths to `index.json` files from the downloaded
    /// torrent. Each one indicates a database you would like to
    /// enable. If none are provided, and you're on Windows, it enters
    /// a special "interactive" mode where you select the files you
    /// want via a graphical file picker. Rate limiting is implicitly
    /// disabled in interactive mode.
    #[arg(value_name = "/path/to/index.json")]
    index_paths: Vec<PathBuf>,
}

impl CliArgs {
    fn is_interactive(&self) -> bool {
        self.index_paths.is_empty()
    }

    fn is_rate_limiting_enabled(&self) -> bool {
        self.production
    }

    fn is_production(&self) -> bool {
        self.production
    }
}

const HTML_DIR: include_dir::Dir<'static> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/html");

fn response_404() -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        axum::Json(ErrorResponse::not_found()),
    )
        .into_response()
}

fn response_400_bad_uri() -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        axum::Json(ErrorResponse::bad_request("invalid URI, probably bad percent encoding")),
    )
        .into_response()
}

fn response_500(e: impl Debug) -> axum::response::Response {
    log::error!(target: "500", "Serving up a 500 error due to: {e:?}");
    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(ErrorResponse::internal_error())).into_response()
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = CliArgs::parse();

    let index_paths: Vec<PathBuf> = if args.is_interactive() {
        #[cfg(windows)]
        {
            log::info!("No paths passed on command line, entering interactive mode");
            let mut index_paths = vec![];
            'index_path_interactive_loop: loop {
                index_paths.extend_from_slice(
                    &native_dialog::DialogBuilder::file()
                        .set_title("Select index.json files")
                        .add_filter("index.json file", ["json"])
                        .open_multiple_file()
                        .show()
                        .expect("Error showing file picker dialog"),
                );
                if !native_dialog::DialogBuilder::message()
                    .set_title("Add more index.json files?")
                    .set_text(format!("({} so far)", index_paths.len()))
                    .confirm()
                    .show()
                    .unwrap()
                {
                    break 'index_path_interactive_loop;
                }
            }
            index_paths
        }
        #[cfg(not(windows))]
        {
            log::warn!("No index.json paths passed on command line, there will be no content!");
            Vec::new()
        }
    } else {
        args.index_paths.clone()
    };

    let branding = SiteBranding {
        name: args.site_name.clone(),
        slogan: args.slogan.clone(),
        latin_phrase: args.latin_phrase.clone(),
        announcement: args.announcement_html.clone(),
    };

    let mut indices_mut = Indices::new();

    for index_path in index_paths {
        let index_path_string = index_path.to_string_lossy().into_owned();
        log::info!("Loading {index_path_string}");
        let mut bytes = Vec::new();
        std::fs::File::open(&index_path)
            .expect("Failed to open {index_path_string}")
            .read_to_end(&mut bytes)
            .expect("Failed to read {index_path_string}");

        let index_value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("index.json had invalid JSON!");
        let common_parsed: IndexJsonCommon = serde_json::from_value(index_value.clone())
            .expect("index.json was missing common properties!");

        let database: Arc<dyn DatabaseEngine> = match common_parsed.meta.database.as_str() {
            "lemon" => {
                log::info!("Loading LEMON database from {index_path_string}");
                let lemon_db = Lemon::new(index_value, &index_path, &HTML_DIR, branding.clone())
                    .expect("Failed to create LEMON database");
                Arc::new(lemon_db)
            }
            "charm" => {
                log::info!("Loading CHARM database from {index_path_string}");
                let charm_db = Charm::new(index_value, &index_path, &HTML_DIR, branding.clone())
                    .expect("Failed to create CHARM database");
                Arc::new(charm_db)
            }
            unknown_db_name => {
                panic!("Unknown database type in {index_path_string}: \"{unknown_db_name}\"")
            }
        };
        indices_mut.add_database(database.clone()).unwrap();
        for vehicle in common_parsed.vehicles {
            indices_mut.add_vehicle(
                vehicle.make,
                &vehicle.years,
                vehicle.model,
                vehicle.engine,
                vehicle.uri_path,
                database.clone(),
            );
        }
    }
    let indices = Arc::new(indices_mut);
    let serve_dir_service = tower_serve_static::ServeDir::new(&HTML_DIR);
    let serve_dir_layer = NotFoundLayerAdapter::new(serve_dir_service);
    let mut default_headers = axum::http::HeaderMap::new();
    if args.is_production() {
        default_headers.insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("max-age=86400"),
        );
    }
    let default_headers_layer = tower_default_headers::DefaultHeadersLayer::new(default_headers);

    let is_rate_limiting_enabled = args.is_rate_limiting_enabled();
    let all_requests_governor = Arc::new(governor::DefaultKeyedRateLimiter::<IpAddr>::keyed(governor::Quota::per_minute(args.rate_limit_all_requests_per_ip_per_minute.try_into().expect("Must provide non-zero rate limit"))));
    let bundle_requests_governor =  Arc::new(governor::DefaultKeyedRateLimiter::<IpAddr>::keyed(governor::Quota::per_hour(args.rate_limit_bundles_per_ip_per_hour.try_into().expect("Must provide non-zero bundle rate limit"))));
    let semaphore_limit = if is_rate_limiting_enabled {
        args.rate_limit_global_inflight_bundles
    } else {
        tokio::sync::Semaphore::MAX_PERMITS
    };
    let bundle_semaphore = Arc::new(tokio::sync::Semaphore::new(semaphore_limit));

    let rate_limit_layer = axum::middleware::from_fn(
        move |ip: real::RealIp, request: axum::extract::Request, next: axum::middleware::Next| {
            let all_requests_governor = all_requests_governor.clone();
            async move {
                if is_rate_limiting_enabled {
                    match all_requests_governor.check_key(&ip.ip()) {
                        Ok(_) => next.run(request).await,
                        Err(_) => (StatusCode::TOO_MANY_REQUESTS, "You've had one too many. Please slow down how many pages you're browsing.").into_response(),
                    }
                } else {
                    next.run(request).await
                }
            }
        },
    );

    let real_ip_layer = real::RealIpLayer::with_extractor(
        real::IpExtractor::new().with_headers(vec!["X-Real-IP".to_string()]),
    );

    let app = axum::Router::new()
        .fallback(async move |ip: real::RealIp, uri: axum::http::Uri, method: axum::http::Method, raw_form: axum::extract::RawForm| -> axum::response::Response {
            let parsed_uri_path = match parse_uri_path(uri.path()) {
                Ok(p) => p,
                Err(_) => return response_400_bad_uri(),
            };
            let mut needs_canonical_redirect = false;
            let fragmentless_uri_path = match parsed_uri_path.reencode_properly() {
                Err(_) => return response_400_bad_uri(),
                Ok((p, ncr)) => {
                    needs_canonical_redirect |= ncr;
                    p
                }
            };
            let server_uri_path = match ServerUriPath::try_from(fragmentless_uri_path) {
                Err(_) => {
                    return (StatusCode::BAD_REQUEST, axum::Json(ErrorResponse::bad_request("invalid URI")))
                        .into_response();
                }
                Ok(u) => u,
            };
            let (canonical_uri_path, ncr) = server_uri_path.canonicalize();
            needs_canonical_redirect |= ncr;
            if needs_canonical_redirect {
                return axum::response::Redirect::permanent(&String::from(
                    canonical_uri_path.stringify(),
                ))
                    .into_response();
            }

            for database in indices.databases() {
                if database.global_request_predicate(&canonical_uri_path) {
                    let database = database.clone();
                    return match tokio::task::spawn_blocking(move || {
                        database.handle_global_request(&canonical_uri_path)
                    })
                        .await
                        .unwrap()
                    {
                        Ok(Some(global_response)) => global_response,
                        Ok(None) => response_404(),
                        Err(err) => response_500(err.context(format!("Global handler error at {}", uri.path()))),
                    };
                }
            }

            match &canonical_uri_path.dirs()[..] {
                [bundle, _, _, _] if bundle.as_str() == "bundle" => {
                    let chopped_uri_path = CanonicalUriPath {
                        dirs: canonical_uri_path.dirs()[1..].into(),
                    };
                    let car_uri_components = chopped_uri_path
                        .extract_car_uri_components()
                        .expect("bundle guaranteed to have 3 parts for car uri component");
                    let matched_database =
                        match indices.database_engine_for_car(&car_uri_components) {
                            Some(matched) => matched,
                            None => return response_404(),
                        };
                    if args.disable_bundles {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            axum::Json(ErrorResponse::bad_request(".zip downloads are disabled right now"))
                        ).into_response();
                    }

                    match method {
                        axum::http::Method::GET => {
                            let make = car_uri_components[0].decode_uri_component().0.to_string();
                            let year = car_uri_components[1].decode_uri_component().0.to_string();
                            let model = car_uri_components[2].decode_uri_component().0.to_string();
                            let car_name = format!("{} {} {}", year, make, model);
                            
                            axum::Json(ApiResponse::ok(serde_json::json!({
                                "car_name": car_name,
                                "make": make,
                                "year": year,
                                "model": model,
                                "message": "POST with captcha=human to download",
                            }))).into_response()
                        }
                        axum::http::Method::POST => {
                            if raw_form.0.as_ref() != b"captcha=human" {
                                return (StatusCode::FORBIDDEN, axum::Json(ErrorResponse::bad_request("You did not type human in the box, bozo. Go back and try again."))).into_response();
                            }

                            let semaphore_permit = match bundle_semaphore.try_acquire_owned() {
                                Ok(p) => p,
                                Err(tokio::sync::TryAcquireError::NoPermits) => return (StatusCode::SERVICE_UNAVAILABLE, axum::Json(ErrorResponse::bad_request("Too many people are downloading .zips simultaneously right now"))).into_response(),
                                Err(tokio::sync::TryAcquireError::Closed) => return response_500("TryAcquireError::Closed acquiring bundling semaphore permit"),
                            };
                            if let Err(e) = bundle_requests_governor.check_key(&ip.ip()) {
                                let remaining_time = e.wait_time_from(bundle_requests_governor.clock().now());
                                let remaining_string = if remaining_time > Duration::from_secs(120) {
                                    format!("{} minutes", remaining_time.as_secs() / 60)
                                } else {
                                    format!("{} seconds", remaining_time.as_secs())
                                };
                                return (StatusCode::TOO_MANY_REQUESTS, axum::Json(ErrorResponse::bad_request(&format!("You've hit the hourly .zip download rate limit. You can download again in {}", remaining_string)))).into_response();
                            }

                            let filename = format!("LEMON {}.zip", car_uri_components_to_human_readable_file_name(&car_uri_components));
                            let (sender, stream) = make_writer_to_bytes_stream();
                            tokio::task::spawn_blocking(move || {
                                let _semaphore_permit = semaphore_permit;
                                if let Err(e) =
                                    matched_database.handle_bundle_request(&car_uri_components, sender)
                                {
                                    log::error!(
                                        "Fatal error during bundling {car_uri_components:?}: {e}"
                                    );
                                }
                            });
                           
                            axum_extra::response::Attachment::new(axum::body::Body::from_stream(stream))
                                .filename(&filename)
                                .content_type("application/zip")
                                .into_response()
                        }
                        _ => (StatusCode::METHOD_NOT_ALLOWED, axum::Json(ErrorResponse::bad_request("Method not allowed"))).into_response()
                    }
                }
                [] => {
                    // Root endpoint - list all makes
                    let makes = indices.get_all_makes();
                    axum::Json(ApiResponse::ok(RootResponse { makes })).into_response()
                }
                [make_str] => {
                    // Make endpoint - list all years for a make
                    let make = Make::new(make_str.decode_uri_component().0.to_string());
                    match indices.get_years_for_make(&make) {
                        Some(years) => {
                            axum::Json(ApiResponse::ok(MakeResponse {
                                make: make.0.clone(),
                                years,
                            })).into_response()
                        }
                        None => response_404(),
                    }
                }
                [make_str, year_str] => {
                    // Make/Year endpoint - list models for a make/year
                    let make = Make::new(make_str.decode_uri_component().0.to_string());
                    let year = Year::new(year_str.decode_uri_component().0.to_string());
                    match indices.get_models_for_make_year(&make, &year) {
                        Some(response) => {
                            axum::Json(ApiResponse::ok(response)).into_response()
                        }
                        None => response_404(),
                    }
                }
                _ => {
                    // Manual content pages - return JSON with HTML content
                    let car_uri_components = &canonical_uri_path
                        .extract_car_uri_components()
                        .expect("Guaranteed to have at least three parts in this match branch");
                    let matched_database =
                        match indices.database_engine_for_car(car_uri_components) {
                            Some(matched) => matched,
                            None => return response_404(),
                        };
                    tokio::task::spawn_blocking(move || {
                        match matched_database.handle_car_request_json(canonical_uri_path) {
                            Ok(Some(page_data)) => {
                                axum::Json(ApiResponse::ok(page_data)).into_response()
                            }
                            Ok(None) => response_404(),
                            Err(e) => response_500(e.context("Handle car request error")),
                        }
                    })
                        .await
                        .unwrap()
                }
            }
        }
        )
        .layer(serve_dir_layer)
        .layer(default_headers_layer)
        .layer(rate_limit_layer)
        .layer(real_ip_layer);

    if args.listen_address.starts_with("unix:") {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            use tokio::net::UnixListener;

            let path = Path::new(&args.listen_address["unix:".len()..]);
            log::info!("Server starting on unix socket {}", path.to_string_lossy());

            if path.exists() {
                std::fs::remove_file(path).expect("Unix socket already existed, and we failed to delete it (likely already in use).");
            }
            let listener = UnixListener::bind(path).expect("Failed to listen on unix socket");
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o770))
                .expect("Failed to set permissions on unix socket");
            axum::serve(listener, app).await?;
        }
        #[cfg(not(unix))]
        panic!("Cannot listen on unix socket on non-unix platform such as Windows");
    } else {
        log::info!("Server starting at {}", args.listen_address);
        let listener = TcpListener::bind(&args.listen_address)
            .await
            .expect("Failed to listen on TCP socket");
        #[cfg(windows)]
        if args.is_interactive() {
            let listen_address_to_display = args.listen_address.replace("0.0.0.0", "127.0.0.1");
            native_dialog::DialogBuilder::message()
                .set_title("Server Information")
                .set_text(format!(
                    "After clicking OK, LEMON will be available by typing {} into your browser.",
                    &listen_address_to_display
                ))
                .alert()
                .show()
                .expect("Failed to show server listen info dialog");
        }
        axum::serve(listener, app).await?;
    }

    Ok(())
}
