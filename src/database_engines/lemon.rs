use std::borrow::Cow;
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Error, Result, anyhow, bail};
use axum::response::IntoResponse;
use elsa::FrozenMap;
use plait::html;
use regex::Regex;
use serde::Deserialize;

use crate::common::{
    Breadcrumb, ImageType, SenderWriter, SiteBranding, aau_404, add_header_and_footer, aou_404,
    breadcrumbs_to_api_breadcrumbs, breadcrumbs_to_title, breadcrumbs_to_topics, english_list,
    get_or_compute, image_bytes_to_response, make_zip_static_files, manual_links_from_html,
};
use crate::database_engines::{DatabaseEngine, ResponseFormat};
use crate::kv_store::{KVKey, KVStore, KVStoreCache};
use crate::types::{
    DatabaseFileType, DatabaseMachineName, IndexJson, Make, ManualPageResponse, Year,
};
use crate::uri_path::{
    AbsoluteUriPath, CanonicalUriPath, CarUriComponents, ConcretizeResult, FullUriPath,
    ServerUriPath, UriComponent, UriPath, concretize_uri, make_breadcrumbs, parse_uri_path,
};
use crate::zipper::{
    AbsoluteAdjustedUri, AbsoluteOriginalUri, RelativeOriginalUri, ScopedZipper, ZipCore, Zipper,
};

static HYPERLINK25_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"hyperlink25\(([^)]*)\)").unwrap());

static ZIPPER_HYPERLINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##"((?:src|href)=['"])([^'"#]+)"##).unwrap());

const TITLES_NEED_EXTRA_BREADCRUMBS: [&str; 11] = [
    "general description",
    "wiring diagram",
    "trouble symptom",
    "dtc detecting condition",
    "preparation tool",
    "other inspections",
    "remove & replace",
    "remove, install, and overhaul",
    "reverse clean & flush",
    "other variant",
    "electrical component location",
];

#[derive(Debug, Clone, Deserialize)]
struct VehicleMeta {
    #[serde(alias = "rootUriTable")]
    root_uri_table: KVKey,
    #[serde(alias = "rootLinkTable")]
    root_link_table: KVKey,
}

#[derive(Debug, Clone, Deserialize)]
struct IndexMeta {
    #[serde(alias = "databasePath")]
    database_path: PathBuf,
    #[serde(alias = "databaseType")]
    database_type: DatabaseFileType,
    #[serde(alias = "imagesDatabasePath")]
    images_database_path: PathBuf,
    #[serde(alias = "imagesDatabaseType")]
    images_database_type: DatabaseFileType,
}

pub struct Lemon {
    text_database: KVStore,
    images_database: KVStore,
    vehicle_metas: HashMap<CarUriComponents, VehicleMeta>,
    zip_static_files: Vec<(ServerUriPath, Vec<u8>)>,
    site_branding: SiteBranding,
}

impl Lemon {
    pub fn new(
        index_json: serde_json::Value,
        index_json_path: &Path,
        included_dir: &include_dir::Dir,
        site_branding: SiteBranding,
    ) -> Result<Self> {
        let index_json: IndexJson<IndexMeta, VehicleMeta> = serde_json::from_value(index_json)
            .context("LEMON index.json didn't have expected format")?;

        let vehicle_metas = HashMap::from_iter(
            index_json
                .vehicles
                .into_iter()
                .map(|vehicle| (vehicle.common.uri_path, vehicle.db_specific_metadata)),
        );

        let index_json_dir = index_json_path.parent().ok_or_else(|| {
            anyhow!(
                "Could not find directory for index JSON path {}",
                index_json_path.to_string_lossy()
            )
        })?;
        let text_db_path = index_json_dir
            // idk if there's a better way to get a path from a str
            .join(AsRef::<Path>::as_ref(
                &index_json.meta.db_specific.database_path,
            ));
        let images_db_path = index_json_dir.join(AsRef::<Path>::as_ref(
            &index_json.meta.db_specific.images_database_path,
        ));

        let zip_static_files = make_zip_static_files(
            included_dir,
            &[
                ("script.js", "script.js"),
                ("style.css", "style.css"),
                ("about.html", "about.html"),
                ("404.html", "404.html"),
                ("lemon-external-car.html", "external-car.html"),
                ("lemon-zip-README.txt", "README.txt"),
                ("icons/folder.svg", "icons/folder.svg"),
                ("icons/folder-open.svg", "icons/folder-open.svg"),
                ("icons/quick-lookups.svg", "icons/quick-lookups.svg"),
                (
                    "icons/diagnostic-trouble-codes.svg",
                    "icons/diagnostic-trouble-codes.svg",
                ),
                ("icons/diagrams.svg", "icons/diagrams.svg"),
                ("icons/specifications.svg", "icons/specifications.svg"),
                (
                    "icons/technical-service-bulletins.svg",
                    "icons/technical-service-bulletins.svg",
                ),
                (
                    "icons/tools-and-equipment.svg",
                    "icons/tools-and-equipment.svg",
                ),
                ("icons/labor-times.svg", "icons/labor-times.svg"),
                ("icons/download.svg", "icons/download.svg"),
                ("icons/wrench.svg", "icons/wrench.svg"),
                ("icons/locations.svg", "icons/locations.svg"),
                ("icons/tire.svg", "icons/tire.svg"),
                ("icons/fluids.svg", "icons/fluids.svg"),
            ],
        )?;

        Ok(Lemon {
            text_database: KVStore::new(&text_db_path, index_json.meta.db_specific.database_type)?,
            images_database: KVStore::new(
                &images_db_path,
                index_json.meta.db_specific.images_database_type,
            )?,
            vehicle_metas,
            zip_static_files,
            site_branding,
        })
    }
}

impl DatabaseEngine for Lemon {
    fn machine_readable_name(&self) -> DatabaseMachineName {
        DatabaseMachineName("lemon".to_string())
    }

    fn human_readable_name(&self) -> String {
        "LEMON".to_string()
    }

    fn priority_and_info(&self, _make: &Make, year: &Year) -> (i32, String) {
        let year = year.0.parse::<u32>();
        year.map(|year| {
            if year >= 2022 {
                (1, "<b>Warning</b>: LEMON manuals were retrieved in late 2025, so manuals for the last couple years before that may not be very detailed.".to_string())
            } else if year >= 2011 {
                (1, "LEMON manuals are what our site is known for; they were retrieved in late 2025 and are best for newer vehicles.".to_string())
            } else if year < 1985 {
                (-1, "LEMON manuals were retrieved in late 2025. For very old vehicles, they are not very detailed.".to_string())
            } else {
                (-1, "LEMON manuals were retrieved in late 2025, so they have more up to date technical bulletins for example. However, they tend to be less detailed for older vehicles.".to_string())
            }
        }).unwrap_or_else(|_| (-1, "Year was not an integer??".to_string()))
    }

    fn global_request_predicate(&self, uri_path: &CanonicalUriPath) -> bool {
        matches!(uri_path.dirs(), [images25, _] if images25.as_str() == "images25")
    }

    fn handle_global_request(
        &self,
        uri_path: &CanonicalUriPath,
    ) -> Result<Option<axum::response::Response>> {
        match uri_path.dirs() {
            [images25, image_id] if images25.as_str() == "images25" => {
                let image_bytes = self
                    .images_database
                    // although images are compressed, no need to cache during single image dl
                    .get(&image_id_to_kv_key(image_id.as_str()), None)?;
                Ok(image_bytes.map(image_bytes_to_response))
            }
            _ => panic!("global request not matching predicate (lemon)"),
        }
    }

    fn handle_car_request(
        &self,
        uri_path: CanonicalUriPath,
        response_format: ResponseFormat,
    ) -> Result<Option<axum::response::Response>> {
        let tables_cache = TablesCache::new(&self.text_database);

        let vehicle = self
            .vehicle_metas
            .get(
                &uri_path
                    .extract_car_uri_components()
                    .expect("Can't pass less than 3 URI components to handle_car_request (LEMON)"),
            )
            // arguably should be an assertion / unwrap instead:
            .ok_or_else(|| {
                anyhow!("LEMON handler called with unknown car uri components: {uri_path:?}")
            })?;

        let page = match self.retrieve_page(&tables_cache, vehicle, uri_path)? {
            Some(page) => page,
            None => return Ok(None),
        };
        let page_db_bytes = match self
            .text_database
            .get(&page.key, Some(&tables_cache.kv_cache))?
        {
            Some(page) => page,
            None => return Ok(None),
        };
        Ok(Some(match response_format {
            ResponseFormat::Html => axum::response::Html(self.page_db_bytes_to_outer_html(
                &tables_cache,
                vehicle,
                &page,
                &page_db_bytes,
            )?)
            .into_response(),
            ResponseFormat::Json => axum::Json(self.page_db_bytes_to_json(
                &tables_cache,
                vehicle,
                &page,
                &page_db_bytes,
            )?)
            .into_response(),
        }))
    }

    fn handle_bundle_request(
        &self,
        car_uri_components: &CarUriComponents,
        writer: SenderWriter,
    ) -> Result<()> {
        let tables_cache = TablesCache::new(&self.text_database);
        let vehicle_meta = self
            .vehicle_metas
            .get(car_uri_components)
            .ok_or_else(|| anyhow!("Got bundle request for missing car"))?;
        let tables_cache: &TablesCache = &tables_cache;
        let zip_core = LemonZipCore {
            lemon: self,
            tables_cache,
            vehicle: vehicle_meta,
            car_uri_components,
            short_name_counter: Cell::new(0),
        };
        let mut zipper = Zipper::new(car_uri_components, zip_core, writer);
        zipper.add_static_files(&self.zip_static_files)?;
        let start_aau = AbsoluteOriginalUri(ServerUriPath {
            dirs: car_uri_components.into(),
            file: None,
        });
        zipper.recurse(start_aau, AdjustCtx::Page)?;
        zipper.finish()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum PageInfo {
    OtherVariants {
        #[serde(alias = "otherVariants")]
        other_variants: Vec<String>,
    },
    OtherCars {
        #[serde(alias = "otherCars")]
        other_cars: Vec<String>,
        #[serde(alias = "numOtherCars")]
        num_other_cars: usize,
    },
}

#[derive(Debug)]
struct Page {
    /// Key to the actual page content
    key: KVKey,
    info: Option<PageInfo>,
    // includes the car breadcrumbs
    breadcrumbs: Vec<Breadcrumb>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UriTablePrefix {
    KeyOnly(KVKey),
    KeyAndInfo {
        #[serde(alias = "key")]
        next_uri_table_key: KVKey,
        #[serde(flatten)]
        info: PageInfo,
    },
}

impl UriTablePrefix {
    fn next_uri_table_key(&self) -> &KVKey {
        match self {
            UriTablePrefix::KeyOnly(key) => key,
            UriTablePrefix::KeyAndInfo {
                next_uri_table_key,
                info: _,
            } => next_uri_table_key,
        }
    }

    fn page_info(&self) -> Option<&PageInfo> {
        match self {
            UriTablePrefix::KeyOnly(_) => None,
            UriTablePrefix::KeyAndInfo {
                info,
                next_uri_table_key: _,
            } => Some(info),
        }
    }
}

#[derive(Debug, Deserialize)]
struct UriTable {
    prefix: HashMap<String, UriTablePrefix>,
    #[serde(alias = "final")]
    final_: HashMap<String, KVKey>,
}

#[derive(Debug, Deserialize)]
struct LinkTablePrefix {
    #[serde(alias = "linkTable")]
    next_link_table_key: KVKey,
    path: String,
}

#[derive(Debug, Deserialize)]
struct LinkTable {
    prefix: HashMap<String, LinkTablePrefix>,
    /// values are the final piece of the path
    #[serde(alias = "final")]
    final_: HashMap<String, String>,
}

impl Lemon {
    #[allow(clippy::only_used_in_recursion)]
    fn retrieve_page_in_uri_table(
        &self,
        tables_cache: &TablesCache,
        remaining_path: &str,
        uri_table_key: &KVKey,
        page_info_so_far: Option<PageInfo>,
        recursion_depth: u32,
    ) -> Result<Option<(KVKey, Option<PageInfo>)>> {
        if recursion_depth > 10 {
            bail!("Too much recursion resolving URI table");
        }

        let uri_table = match tables_cache.uri_table(uri_table_key)? {
            Some(ut) => ut,
            None => return Ok(None),
        };
        if let Some(final_key) = uri_table.final_.get(remaining_path) {
            return Ok(Some((
                // can theoretically avoid clone here by calling
                // `.remove` from the hashmap, but not sure if it's
                // faster and then it'd be mut etc etc
                final_key.clone(),
                page_info_so_far,
            )));
        }

        for (prefix, prefix_value) in &uri_table.prefix {
            if remaining_path.starts_with(prefix) {
                let new_page_info = match prefix_value.page_info() {
                    Some(new_info) => {
                        if page_info_so_far.is_some() {
                            bail!(
                                "Multiple uri table prefixes had nonempty page info: {page_info_so_far:?} vs {new_info:?}"
                            );
                        }
                        Some(new_info.clone())
                    }
                    None => page_info_so_far,
                };
                return self.retrieve_page_in_uri_table(
                    tables_cache,
                    &remaining_path[prefix.len()..],
                    prefix_value.next_uri_table_key(),
                    new_page_info,
                    recursion_depth + 1,
                );
            }
        }
        Ok(None)
    }

    fn retrieve_page_key_and_info(
        &self,
        tables_cache: &TablesCache,
        vehicle: &VehicleMeta,
        uri_path: &CanonicalUriPath,
    ) -> Result<Option<(KVKey, Option<PageInfo>)>> {
        let remaining_path = String::from(
            FullUriPath {
                dirs: uri_path.dirs()[3..].into(),
                file: None,
                fragment: None,
                is_absolute: false,
            }
            .stringify(),
        );
        self.retrieve_page_in_uri_table(
            tables_cache,
            &remaining_path,
            &vehicle.root_uri_table,
            None,
            0,
        )
    }

    /// uri_components should include the first three / car components
    fn retrieve_page(
        &self,
        tables_cache: &TablesCache,
        vehicle: &VehicleMeta,
        uri_path: CanonicalUriPath,
    ) -> Result<Option<Page>> {
        // uri components logic is a bit fucked
        match self.retrieve_page_key_and_info(tables_cache, vehicle, &uri_path)? {
            Some((db_key, page_info)) => {
                let breadcrumbs = make_breadcrumbs(&uri_path, &|uri_components| {
                    Ok(self
                        .retrieve_page_key_and_info(
                            tables_cache,
                            vehicle,
                            &CanonicalUriPath {
                                dirs: uri_components.into(),
                            },
                        )?
                        .is_some())
                })?;
                Ok(Some(Page {
                    key: db_key,
                    info: page_info,
                    breadcrumbs,
                }))
            }
            None => Ok(None),
        }
    }

    /// Error here means something real mean, the whole page should fail.
    #[allow(clippy::only_used_in_recursion)]
    fn resolve_link_code_in_link_table(
        &self,
        tables_cache: &TablesCache,
        remaining_link_code: &str,
        link_table_key: &KVKey,
        result_so_far: &str,
        recursion_depth: u32,
    ) -> Result<Option<String>> {
        if recursion_depth > 10 {
            bail!("Too much recursion in link table");
        }
        let link_table = match tables_cache.link_table(link_table_key)? {
            Some(lt) => lt,
            None => return Ok(None),
        };
        if let Some(final_segment) = link_table.final_.get(remaining_link_code) {
            return Ok(Some(result_so_far.to_string() + final_segment));
        }
        for (prefix, prefix_value) in &link_table.prefix {
            if remaining_link_code.starts_with(prefix) {
                return self.resolve_link_code_in_link_table(
                    tables_cache,
                    &remaining_link_code[prefix.len()..],
                    &prefix_value.next_link_table_key,
                    &(result_so_far.to_string() + &prefix_value.path),
                    recursion_depth + 1,
                );
            }
        }
        Ok(None)
    }

    fn resolve_link_code(
        &self,
        tables_cache: &TablesCache,
        vehicle: &VehicleMeta,
        link_code: &str,
    ) -> Result<Option<AbsoluteUriPath>> {
        let raw_resolved_link = self.resolve_link_code_in_link_table(
            tables_cache,
            link_code,
            &vehicle.root_link_table,
            "",
            0,
        )?;
        match raw_resolved_link {
            Some(raw_resolved_link) => {
                let canonical_resolved_link = ServerUriPath::try_from(
                    parse_uri_path(&raw_resolved_link)?.reencode_properly()?.0,
                )?
                .canonicalize()
                .0;
                concretize_uri(&canonical_resolved_link, &|dirs| {
                    if self
                        .retrieve_page_key_and_info(
                            tables_cache,
                            vehicle,
                            &CanonicalUriPath { dirs: dirs.into() },
                        )?
                        .is_some()
                    {
                        return Ok(ConcretizeResult::Found);
                    }
                    if dirs.len() > 4 {
                        Ok(ConcretizeResult::NotFoundYet)
                    } else {
                        Ok(ConcretizeResult::NotFoundEver)
                    }
                })
            }
            None => Ok(None),
        }
    }

    fn replace_links<'a>(
        &self,
        tables_cache: &TablesCache,
        vehicle: &VehicleMeta,
        content: &'a str,
    ) -> Result<Cow<'a, str>> {
        let mut global_err: Option<Error> = None;
        let replaced = HYPERLINK25_REGEX.replace_all(content, |caps: &regex::Captures| {
            match self.resolve_link_code(tables_cache, vehicle, &caps[1]) {
                Err(err) => {
                    global_err = Some(err);
                    log::error!("too much recursion in a link???");
                    // will never be displayed, as we'll return error:
                    "".to_string()
                }
                Ok(None) => {
                    // BACKLOG determine whether this ever happens for known_missing pages? in which case just replace with known_missing
                    log::error!("Link missing from link table: {}", &caps[1]);
                    "".to_string()
                }
                Ok(Some(rep)) => String::from(rep.stringify()),
            }
        });
        match global_err {
            Some(err) => Err(err),
            None => Ok(replaced),
        }
    }

    /// "inner html" means without header/footer
    fn page_db_bytes_to_outer_html(
        &self,
        tables_cache: &TablesCache,
        vehicle: &VehicleMeta,
        page: &Page,
        page_db_bytes: &[u8],
    ) -> Result<String> {
        let page_db_string = String::from_utf8_lossy(page_db_bytes);
        let page_string = self.replace_links(tables_cache, vehicle, &page_db_string)?;
        let page_string = page
            .info
            .as_ref()
            .map(page_info_to_warning_html)
            .unwrap_or_default()
            + page_string.as_ref();
        let page_string = add_header_and_footer(
            &self.site_branding,
            &page_string,
            &page.breadcrumbs,
            breadcrumbs_need_more_context_predicate,
        );
        Ok(page_string)
    }

    fn page_db_bytes_to_json(
        &self,
        tables_cache: &TablesCache,
        vehicle: &VehicleMeta,
        page: &Page,
        page_db_bytes: &[u8],
    ) -> Result<ManualPageResponse> {
        let page_db_string = String::from_utf8_lossy(page_db_bytes);
        let replaced_links_html = self.replace_links(
            tables_cache,
            vehicle,
            &page_db_string,
        )?;
        Ok(ManualPageResponse {
            title: breadcrumbs_to_title(&page.breadcrumbs, breadcrumbs_need_more_context_predicate),
            content: self.page_db_bytes_to_outer_html(
                tables_cache,
                vehicle,
                page,
                page_db_bytes,
            )?,
            breadcrumbs: breadcrumbs_to_api_breadcrumbs(&page.breadcrumbs),
            topics: breadcrumbs_to_topics(&page.breadcrumbs),
            manuals: manual_links_from_html(
                &CanonicalUriPath {
                    dirs: page
                        .breadcrumbs
                        .iter()
                        .map(|(label, _)| label.clone())
                        .collect(),
                },
                replaced_links_html.as_ref(),
            ),
        })
    }

    fn determine_adjust_ctx(
        &self,
        aou: &AbsoluteOriginalUri,
        car_uri_components: &CarUriComponents,
    ) -> Option<AdjustCtx> {
        let uri_path = &aou.0;
        let dirs = uri_path.dirs();
        if dirs.len() == 2 && dirs[0].as_str() == "images25" && uri_path.file.is_none() {
            Some(AdjustCtx::Image(dirs[1].as_str().to_string()))
        } else if uri_path.extract_car_uri_components().as_ref() == Some(car_uri_components) {
            Some(AdjustCtx::Page)
        } else {
            for (static_server_uri, _) in &self.zip_static_files {
                if uri_path == static_server_uri {
                    return Some(AdjustCtx::Static);
                }
            }
            None
        }
    }
}

struct LemonZipCore<'a> {
    lemon: &'a Lemon,
    tables_cache: &'a TablesCache<'a>,
    vehicle: &'a VehicleMeta, // doesn't really need to be a reference but it's free at this point
    car_uri_components: &'a CarUriComponents,
    short_name_counter: Cell<u64>,
}

enum AdjustCtx {
    Page,
    Image(String),
    Static,
}

enum WriteCtx {
    Page(Page),
    // image has to be read during adjustment to know the correct extension
    Image(Vec<u8>),
}

impl<'a> ZipCore for LemonZipCore<'a> {
    type AdjustCtx = AdjustCtx;
    type WriteCtx = WriteCtx;

    fn adjust_uri(
        &self,
        absolute_original_uri: &AbsoluteOriginalUri,
        ctx: Self::AdjustCtx,
    ) -> Result<(AbsoluteAdjustedUri, Option<Self::WriteCtx>)> {
        match ctx {
            AdjustCtx::Page => {
                let (canonical_uri, changed) = absolute_original_uri.0.clone().canonicalize();
                if changed {
                    bail!(
                        "URI changed during canonicalization (lemon zip): {absolute_original_uri:?} to {canonical_uri:?}"
                    );
                }
                let page =
                    self.lemon
                        .retrieve_page(self.tables_cache, self.vehicle, canonical_uri)?;
                Ok(page
                    .map(|p| match p.info {
                        Some(PageInfo::OtherCars { .. }) => (
                            AbsoluteAdjustedUri(AbsoluteUriPath {
                                dirs: vec![],
                                file: Some(UriComponent::unsafe_from_encoded_str(
                                    "external-car.html",
                                )),
                                fragment: None,
                            }),
                            None,
                        ),
                        _ => {
                            let cur_counter = self.short_name_counter.get();
                            self.short_name_counter.set(cur_counter + 1);
                            let uri_path = if cur_counter == 0 {
                                AbsoluteUriPath {
                                    dirs: vec![],
                                    file: Some(UriComponent::unsafe_from_encoded_str("index.html")),
                                    fragment: None,
                                }
                            } else {
                                AbsoluteUriPath {
                                    dirs: vec![UriComponent::unsafe_from_encoded_str("pages")],
                                    file: Some(UriComponent::unsafe_from_encoded_str(&format!(
                                        "{}.html",
                                        cur_counter
                                    ))),
                                    fragment: None,
                                }
                            };
                            (AbsoluteAdjustedUri(uri_path), Some(WriteCtx::Page(p)))
                        }
                    })
                    .unwrap_or_else(|| (aau_404(), None)))
            }
            AdjustCtx::Image(image_id) => {
                let image = self.lemon.images_database.get(
                    &image_id_to_kv_key(&image_id),
                    Some(&self.tables_cache.kv_cache),
                )?;
                if let Some(bytes) = image {
                    let image_type = ImageType::guess(&bytes);
                    if let Some(image_type) = image_type {
                        // I believe these are all url-encoded already but let's be sure and encode
                        // only fails on empty, ok to unwrap
                        let file_name = UriComponent::from_decoded_str(&format!(
                            "{image_id}.{}",
                            image_type.file_extension()
                        ))
                        .unwrap();
                        return Ok((
                            AbsoluteAdjustedUri(AbsoluteUriPath {
                                dirs: vec![UriComponent::unsafe_from_encoded_str("images")],
                                file: Some(file_name),
                                fragment: None,
                            }),
                            Some(WriteCtx::Image(bytes)),
                        ));
                    }
                }
                Ok((
                    AbsoluteAdjustedUri(AbsoluteUriPath {
                        dirs: vec![],
                        // BACKLOG include in static zip
                        file: Some(UriComponent::unsafe_from_encoded_str("404.png")),
                        fragment: None,
                    }),
                    None,
                ))
            }
            // should probably be an assert :|
            AdjustCtx::Static => bail!(
                "Shouldn't have to ever actually adjust a static (should be in the already done hashmap)"
            ),
        }
    }

    fn write(
        &self,
        scoped_zipper: &mut ScopedZipper<Self>,
        ctx: Self::WriteCtx,
    ) -> Result<(Vec<u8>, bool)> {
        match ctx {
            WriteCtx::Page(page) => {
                let content = match self
                    .lemon
                    .text_database
                    .get(&page.key, Some(&self.tables_cache.kv_cache))? {
                        Some(c) => c,
                        // sometimes this happens for legit pages, like missing bulletins at least.
                        None => return Ok((b"This page is a \"known-missing\" page; we know it should be here but we are missing it. Sorry".into(), true)),
                    };
                let page_html = self.lemon.page_db_bytes_to_outer_html(
                    self.tables_cache,
                    self.vehicle,
                    &page,
                    &content,
                )?;
                let mut global_err = None;
                let page_html =
                    ZIPPER_HYPERLINK_REGEX.replace_all(&page_html, |caps: &regex::Captures| {
                        // if there's a protocol in the uri for any reason
                        // (eg we put javascript alert links on error, or
                        // http global links), skip it
                        if caps[2].contains(':') {
                            return format!("{}{}", &caps[1], &caps[2]);
                        }
                        let improper = match parse_uri_path(&caps[2]) {
                            Ok(i) => i,
                            Err(e) => {
                                global_err = Some(e);
                                return String::new();
                            }
                        };
                        let (proper_server, changed) = match improper.reencode_properly() {
                            Ok(o) => o,
                            Err(e) => {
                                global_err = Some(e);
                                return String::new();
                            }
                        };
                        if changed {
                            global_err = Some(anyhow!(
                                "Improperly encoded URI found in lemon zip: {improper:?}"
                            ));
                            return String::new();
                        }
                        let proper_full: FullUriPath = proper_server.into();
                        let rou = RelativeOriginalUri(proper_full);
                        let aou = scoped_zipper.rou_to_aou(rou);
                        let relative_adjusted_uri = match self
                            .lemon
                            .determine_adjust_ctx(&aou, self.car_uri_components)
                        {
                            Some(adjust_ctx) => scoped_zipper.recurse(aou, adjust_ctx),
                            None => scoped_zipper.recurse(aou_404(), AdjustCtx::Static),
                        };
                        format!(
                            "{}{}",
                            &caps[1],
                            String::from(relative_adjusted_uri.0.stringify())
                        )
                    });
                match global_err {
                    Some(err) => Err(err),
                    None => Ok((page_html.into_owned().into_bytes(), true)),
                }
            }
            WriteCtx::Image(bytes) => Ok((bytes, false)),
        }
    }
}

// not really great ways to return "multiple possible implementers of
// HtmlDisplay". We could return a Box<dyn ...> and avoid stringifying
// here, but we'll still need to stringify it later because Box<dyn
// Trait> doesn't actually implement Trait (it forwards the relevant
// calls or sth)
fn page_info_to_warning_html(page_info: &PageInfo) -> String {
    match page_info {
        PageInfo::OtherVariants { other_variants } => match other_variants.as_slice() {
            [] => html! {
                div(class: "other-warning other-variant") {
                    b { "WARNING:" }
                    " This page is about a different variant/trim than selected."
                }
            }.to_string(),
            [other_variant] => html! {
                div(class: "other-warning other-variant") {
                    b { "WARNING:" }
                    " This page is about the " (other_variant) ", which is a different variant/trim than selected."
                }
            }.to_string(),
            _ => panic!("Cannot have more than 1 other variant"),
        },
        PageInfo::OtherCars {
            other_cars,
            num_other_cars,
        } => {
            if other_cars.len() == *num_other_cars {
                html! {
                    div(class: "other-warning other-car") {
                        b { "WARNING:" }
                        " This page is about a different car, the " (english_list(other_cars)) ". However, it is still accessible from the selected car via links, so may be relevant."
                    }
                }.to_string()
            } else {
                html! {
                    div(class: "other-warning other-car") {
                        b { "WARNING:" }
                        " This page does not describe the selected car, but rather " (num_other_cars) " other vehicles, including the " (english_list(other_cars)) ". However, it is still accessible from the selected car via links, so may be relevant."
                    }
                }.to_string()
            }
        }
    }
}

fn breadcrumbs_need_more_context_predicate(bcs: &[Breadcrumb]) -> bool {
    if bcs.len() <= 4 {
        return false;
    }
    let title_lowercase = bcs
        .last()
        .unwrap()
        .0
        .decode_uri_component()
        .0
        .to_lowercase();
    !title_lowercase.contains(' ')
        || TITLES_NEED_EXTRA_BREADCRUMBS.contains(&title_lowercase.as_str())
}

fn image_id_to_kv_key(image_id: &str) -> KVKey {
    KVKey("image_".to_string() + image_id)
}

struct TablesCache<'a> {
    db: &'a KVStore,
    link_tables: FrozenMap<KVKey, Box<Option<LinkTable>>>,
    uri_tables: FrozenMap<KVKey, Box<Option<UriTable>>>,
    kv_cache: KVStoreCache,
}

impl<'a> TablesCache<'a> {
    fn new(db: &'a KVStore) -> Self {
        Self {
            db,
            link_tables: FrozenMap::new(),
            uri_tables: FrozenMap::new(),
            kv_cache: KVStoreCache::new(),
        }
    }

    fn link_table(&self, key: &KVKey) -> Result<Option<&LinkTable>> {
        get_or_compute(&self.link_tables, key, || {
            Ok(Box::new(
                self.db
                    .get(key, Some(&self.kv_cache))?
                    .map(|v| serde_json::from_slice(&v))
                    .transpose()?,
            ))
        })
        .map(|x| x.as_ref())
    }

    fn uri_table(&self, key: &KVKey) -> Result<Option<&UriTable>> {
        get_or_compute(&self.uri_tables, key, || {
            Ok(Box::new(
                self.db
                    .get(key, Some(&self.kv_cache))?
                    .map(|v| serde_json::from_slice(&v))
                    .transpose()?,
            ))
        })
        .map(|x| x.as_ref())
    }
}
