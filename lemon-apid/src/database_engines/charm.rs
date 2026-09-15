use std::{
    borrow::Cow,
    cell::Cell,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::{Context, Result, anyhow, bail};
use axum::response::IntoResponse;
use regex::Regex;
use serde::Deserialize;

use crate::{
    common::{
        Breadcrumb, ImageType, SenderWriter, SiteBranding, aau_404, add_header_and_footer, aou_404,
        aup_404, breadcrumbs_to_api_breadcrumbs, breadcrumbs_to_title, breadcrumbs_to_topics,
        image_bytes_to_response, make_zip_static_files, manual_links_from_html,
    },
    database_engines::{DatabaseEngine, ResponseFormat},
    kv_store::{KVKey, KVStore, KVStoreCache},
    types::{DatabaseFileType, DatabaseMachineName, IndexJson, Make, ManualPageResponse, Year},
    uri_path::{
        AbsoluteUriPath, CanonicalUriPath, CarUriComponents, ConcretizeResult, ServerUriPath,
        UriComponent, UriPath, concretize_uri, make_breadcrumbs, parse_uri_path,
    },
    zipper::{
        AbsoluteAdjustedUri, AbsoluteOriginalUri, RelativeOriginalUri, ScopedZipper, ZipCore,
        Zipper,
    },
};

static ZIPPER_HYPERLINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##"((?:src|href)=['"])([^'"#]+)"##).unwrap());

// we do not support these, so remove them
static LONG_BUNDLE_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<li><a href='/bundle/long-names/.+?'>.*?</a>").unwrap());

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

#[derive(Debug, Clone, Deserialize)]
struct VehicleMeta {
    #[serde(alias = "databaseMake")]
    database_make: String,
}

pub struct Charm {
    text_database: KVStore,
    images_database: KVStore,
    vehicle_metas: HashMap<CarUriComponents, VehicleMeta>,
    zip_static_files: Vec<(ServerUriPath, Vec<u8>)>,
    site_branding: SiteBranding,
}

impl Charm {
    pub fn new(
        index_json: serde_json::Value,
        index_json_path: &Path,
        included_dir: &include_dir::Dir,
        site_branding: SiteBranding,
    ) -> Result<Self> {
        let index_json: IndexJson<IndexMeta, VehicleMeta> = serde_json::from_value(index_json)
            .context("CHARM index.json didn't have expected format")?;
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
                // BACKLOG this has absolute links to style and home in it, ideally would use a zip-specific 404 file
                ("404.html", "404.html"),
                ("icons/adjustments.svg", "icons/adjustments.svg"),
                (
                    "icons/description-and-operation.svg",
                    "icons/description-and-operation.svg",
                ),
                (
                    "icons/diagnostic-trouble-codes.svg",
                    "icons/diagnostic-trouble-codes.svg",
                ),
                ("icons/diagrams.svg", "icons/diagrams.svg"),
                ("icons/folder-open.svg", "icons/folder-open.svg"),
                ("icons/folder.svg", "icons/folder.svg"),
                ("icons/labor-times.svg", "icons/labor-times.svg"),
                ("icons/locations.svg", "icons/locations.svg"),
                ("icons/parts.svg", "icons/parts.svg"),
                (
                    "icons/service-and-repair.svg",
                    "icons/service-and-repair.svg",
                ),
                (
                    "icons/service-precautions.svg",
                    "icons/service-precautions.svg",
                ),
                ("icons/specifications.svg", "icons/specifications.svg"),
                (
                    "icons/technical-service-bulletins.svg",
                    "icons/technical-service-bulletins.svg",
                ),
                (
                    "icons/testing-and-inspection.svg",
                    "icons/testing-and-inspection.svg",
                ),
                (
                    "icons/tools-and-equipment.svg",
                    "icons/tools-and-equipment.svg",
                ),
                ("lemon-zip-README.txt", "README.txt"),
            ],
        )?;

        Ok(Charm {
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

    fn retrieve_image(
        &self,
        image_key: &KVKey,
        cache: Option<&KVStoreCache>,
    ) -> Result<Option<Vec<u8>>> {
        self.images_database.get(image_key, cache)
    }

    // useful on its own to check if a page exists
    fn retrieve_page_1(
        &self,
        cache: &KVStoreCache,
        vehicle: &VehicleMeta,
        path: &CanonicalUriPath,
    ) -> Result<Option<PageOffset>> {
        let mut uri_components_with_db_make = Vec::with_capacity(path.dirs().len());
        uri_components_with_db_make.push(UriComponent::from_encoded_str(&vehicle.database_make)?);
        uri_components_with_db_make.extend(path.dirs()[1..].iter().cloned());
        let db_path = CanonicalUriPath {
            dirs: uri_components_with_db_make,
        };
        let db_path_key = KVKey(db_path.stringify().to_string());
        Ok(self
            .text_database
            .get(&db_path_key, Some(cache))?
            .map(PageOffset))
    }

    // refreshingly easier than LEMON
    fn retrieve_page_2(&self, cache: &KVStoreCache, offset: PageOffset) -> Result<String> {
        let offset_key = KVKey(String::from_utf8(offset.0)?);
        let bytes = self.text_database.get(&offset_key, Some(cache))?
            .ok_or_else(|| anyhow!("retrieve_page_1 succeeded but retrieve_page_2 couldn't find page?? For offset {}", offset_key.0))?;
        // afaik all the pages are proper utf8 but just want to be safe
        let string = String::from_utf8_lossy(&bytes).into_owned();
        // only really need to do this on root pages, but it's fast enough to "fix it in post" and not have to thread through the path
        let removed_long_links_cow = LONG_BUNDLE_LINK_REGEX.replace_all(&string, "");
        Ok(match removed_long_links_cow {
            Cow::Borrowed(_) => string,
            Cow::Owned(replaced) => replaced,
        })
    }

    /// hyperlink_path shall include the leading /hyperlink/
    fn resolve_hyperlink(
        &self,
        cache: &KVStoreCache,
        hyperlink_path: &CanonicalUriPath,
    ) -> Result<Option<AbsoluteUriPath>> {
        assert!(
            hyperlink_path.dirs()[0].as_str() == "hyperlink",
            "Don't call resolve_hyperlink on non-hyperlink uri path"
        );
        let without_hyperlink_path = CanonicalUriPath {
            dirs: hyperlink_path.dirs()[1..].into(),
        };
        let without_hyperlink_path = canonicalize_make_in_uri_path(&without_hyperlink_path)?
            .unwrap_or(without_hyperlink_path);
        let car_uri_components = match without_hyperlink_path.extract_car_uri_components() {
            Some(c) => c,
            None => return Ok(None),
        };
        let vehicle_meta = match self.vehicle_metas.get(&car_uri_components) {
            Some(v) => v,
            None => return Ok(None),
        };
        concretize_uri(&without_hyperlink_path, &|dirs| {
            if self
                .retrieve_page_1(cache, vehicle_meta, &CanonicalUriPath { dirs: dirs.into() })?
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

    fn page_string_to_outer_html(
        &self,
        cache: &KVStoreCache,
        inner_html: &str,
        uri_path: &CanonicalUriPath,
        vehicle: &VehicleMeta,
    ) -> Result<String> {
        let breadcrumbs = self.page_breadcrumbs(cache, uri_path, vehicle)?;
        Ok(add_header_and_footer(
            &self.site_branding,
            inner_html,
            &breadcrumbs,
            breadcrumbs_need_more_context_predicate,
        ))
    }

    fn page_breadcrumbs(
        &self,
        cache: &KVStoreCache,
        uri_path: &CanonicalUriPath,
        vehicle: &VehicleMeta,
    ) -> Result<Vec<Breadcrumb>> {
        make_breadcrumbs(uri_path, &|uri_components| {
            Ok(self
                .retrieve_page_1(
                    cache,
                    vehicle,
                    &CanonicalUriPath {
                        dirs: uri_components.into(),
                    },
                )?
                .is_some())
        })
    }

    fn page_string_to_json(
        &self,
        cache: &KVStoreCache,
        inner_html: &str,
        uri_path: &CanonicalUriPath,
        vehicle: &VehicleMeta,
    ) -> Result<ManualPageResponse> {
        let breadcrumbs = self.page_breadcrumbs(cache, uri_path, vehicle)?;
        Ok(ManualPageResponse {
            title: breadcrumbs_to_title(&breadcrumbs, breadcrumbs_need_more_context_predicate),
            breadcrumbs: breadcrumbs_to_api_breadcrumbs(&breadcrumbs),
            topics: breadcrumbs_to_topics(&breadcrumbs),
            content: inner_html.to_string(),
            manuals: manual_links_from_html(uri_path, inner_html),
        })
    }

    fn determine_adjust_ctx(
        &self,
        aou: &AbsoluteOriginalUri,
        car_uri_components: &CarUriComponents,
    ) -> Option<AdjustCtx> {
        if let Some(image_kv_key) = image_request_uri_to_key(&aou.0) {
            Some(AdjustCtx::Image(image_kv_key))
        } else if aou
            .0
            .dirs()
            .first()
            .map(|f| f.as_str() == "hyperlink")
            .unwrap_or(false)
        {
            Some(AdjustCtx::Hyperlink)
        } else if aou.0.extract_car_uri_components().as_ref() == Some(car_uri_components) {
            Some(AdjustCtx::Page)
        } else {
            for (static_server_uri, _) in &self.zip_static_files {
                if &aou.0 == static_server_uri {
                    return Some(AdjustCtx::Static);
                }
            }
            None
        }
    }
}

struct PageOffset(Vec<u8>);

impl DatabaseEngine for Charm {
    fn machine_readable_name(&self) -> DatabaseMachineName {
        DatabaseMachineName("charm".to_string())
    }

    fn human_readable_name(&self) -> String {
        "CHARM".to_string()
    }

    fn priority_and_info(&self, make: &Make, year: &Year) -> (i32, String) {
        (year.0.parse::<u32>()).map(|year| {
            if (1997..=2003).contains(&year) && (make.0 == "Toyota" || make.0 == "Lexus") {
                (-3, "CHARM manuals were retrieved from a set of DVDs released in late 2013 and are generally our most detailed for older years.<br><br>Warning: CHARM manuals for Toyota and Lexus vehicles near the year 2000 are known to have many missing images.".to_string())
            } else if year >= 2011 {
                (0, "<b>Warning</b>: CHARM manuals were retrieved from a set of DVDs released in late 2013, so they often are not very detailed for the last couple of years before that. However, they are generally very good for years before 2011.".to_string())
            } else {
                (0, "CHARM manuals were retrieved from a set of DVDs released in late 2013. These manuals are usually the most detailed for the years in which they're available, but occasionally have missing or cut-off images.".to_string())
            }
        }).unwrap_or_else(|_| (-1, "Year was not an integer??".to_string()))
    }

    fn handle_car_request(
        &self,
        uri_path: CanonicalUriPath,
        response_format: ResponseFormat,
    ) -> Result<Option<axum::response::Response>> {
        if let Some(uri_path_with_canonical_make) = canonicalize_make_in_uri_path(&uri_path)? {
            return Ok(Some(
                axum::response::Redirect::permanent(
                    &uri_path_with_canonical_make.stringify().to_string(),
                )
                .into_response(),
            ));
        }
        // could argue we don't need a cache for single page request,
        // but on the other hand it can speed up breadcrumb resolving.
        let cache = KVStoreCache::new();
        let car_uri_components = &uri_path
            .extract_car_uri_components()
            .expect("Can't pass less than 3 URI components to handle_car_request (CHARM)");
        let vehicle = self
            .vehicle_metas
            .get(car_uri_components)
            .ok_or_else(|| anyhow!("Got bundle request for missing car"))?;
        if let Some(offset) = self.retrieve_page_1(&cache, vehicle, &uri_path)? {
            let inner_html = self.retrieve_page_2(&cache, offset)?;
            Ok(Some(match response_format {
                ResponseFormat::Html => axum::response::Html(self.page_string_to_outer_html(
                    &cache,
                    &inner_html,
                    &uri_path,
                    vehicle,
                )?)
                .into_response(),
                ResponseFormat::Json => {
                    axum::Json(self.page_string_to_json(&cache, &inner_html, &uri_path, vehicle)?)
                        .into_response()
                }
            }))
        } else {
            Ok(None)
        }
    }

    fn handle_bundle_request(
        &self,
        car_uri_components: &CarUriComponents,
        writer: SenderWriter,
    ) -> Result<()> {
        let cache = KVStoreCache::new();
        let vehicle = self
            .vehicle_metas
            .get(car_uri_components)
            .ok_or_else(|| anyhow!("Got bundle requset for missing car"))?;
        let zip_core = CharmZipCore {
            charm: self,
            cache: &cache,
            vehicle,
            car_uri_components,
            short_name_counter: Cell::new(0),
        };
        let mut zipper = Zipper::new(car_uri_components, zip_core, writer);
        zipper.add_static_files(&self.zip_static_files)?;
        let start_aou = AbsoluteOriginalUri(ServerUriPath {
            dirs: car_uri_components.into(),
            file: None,
        });
        zipper.recurse(start_aou, AdjustCtx::Page)?;
        zipper.finish()?;
        Ok(())
    }

    fn global_request_predicate(&self, uri_path: &CanonicalUriPath) -> bool {
        let dirs = uri_path.dirs();
        dirs.len() > 1 && (dirs[0].as_str() == "images" || dirs[0].as_str() == "hyperlink")
    }

    fn handle_global_request(
        &self,
        uri_path: &CanonicalUriPath,
    ) -> Result<Option<axum::response::Response>> {
        let dirs = uri_path.dirs();
        assert!(
            dirs.len() > 1,
            "global request not matching predicate (too short, charm)"
        );
        match dirs[0].as_str() {
            "images" => {
                // shouldn't be able to fail but let's be safe
                let image_key = image_request_uri_to_key(uri_path).ok_or_else(|| {
                    anyhow!("this shouldn't be possible: image uri wasn't a valid image uri")
                })?;
                // no purpose in caching a single request
                let image_bytes = self.retrieve_image(&image_key, None)?;
                Ok(image_bytes.map(image_bytes_to_response))
            }
            "hyperlink" => {
                let cache = KVStoreCache::new();
                Ok(self.resolve_hyperlink(&cache, uri_path)?.map(|r| {
                    axum::response::Redirect::permanent(&r.stringify().to_string()).into_response()
                }))
            }
            _ => panic!("global request not matching predicate (wrong first uri component, charm)"),
        }
    }
}

fn image_request_uri_to_key(image_request_uri: &impl UriPath) -> Option<KVKey> {
    let dirs = image_request_uri.dirs();
    (image_request_uri.is_absolute()
        && image_request_uri.fragment().is_none()
        && image_request_uri.file().is_none()
        && dirs.len() >= 2
        && dirs[0].as_str() == "images")
        .then(|| {
            KVKey(String::from(
                CanonicalUriPath {
                    dirs: dirs[1..].into(),
                }
                .stringify(),
            ))
        })
}

const TITLES_NEED_EXTRA_BREADCRUMBS: [&str; 20] = [
    "service precautions",
    "application and id",
    "description and operation",
    "adjustments",
    "testing and inspection",
    "diagrams",
    "locations",
    "specifications",
    "service and repair",
    "parts",
    "technical service bulletins",
    "tools and equipment",
    "labor times",
    "exploded diagram",
    "service intervals",
    "fundamentals and basics",
    "diagnostic trouble codes",
    "diagnosis and testing",
    "overview",
    "general procedures",
];

fn breadcrumbs_need_more_context_predicate(bcs: &[Breadcrumb]) -> bool {
    if bcs.is_empty() {
        return false;
    }
    let title_lowercase = bcs
        .last()
        .unwrap()
        .0
        .decode_uri_component()
        .0
        .to_lowercase();
    TITLES_NEED_EXTRA_BREADCRUMBS.contains(&title_lowercase.as_str())
}

const TRUCK_SUFFIX: &str = "%20Truck";

/// Return Some when the make is not canonical and a redirect is needed to correct it
fn canonicalize_make_in_uri_path(uri_path: &CanonicalUriPath) -> Result<Option<CanonicalUriPath>> {
    // would be more kosher to decode and encode here but idc
    let new_make = match uri_path.dirs()[0].as_str() {
        "Chevy%20Truck" => Some("Chevrolet"),
        "Dodge" => Some("Dodge%20and%20Ram"),
        "Dodge%20or%20Ram%20Truck" => Some("Dodge%20and%20Ram"),
        "Mitsubishi%20Fuso" => Some("Mitsubishi"),
        trucky
            if trucky.len() >= TRUCK_SUFFIX.len()
                && &trucky[trucky.len() - TRUCK_SUFFIX.len()..] == TRUCK_SUFFIX =>
        {
            Some(&trucky[0..trucky.len() - TRUCK_SUFFIX.len()])
        }
        _ => None,
    };
    if let Some(new_make) = new_make {
        // only reason to use result here is I'm not 100% sure the %20Truck slicing always maintains proper uri encoding
        let new_make_uri_component = UriComponent::from_encoded_str(new_make)?;
        let mut dirs = vec![new_make_uri_component];
        dirs.extend(uri_path.dirs()[1..].iter().cloned());
        Ok(Some(CanonicalUriPath { dirs }))
    } else {
        Ok(None)
    }
}

struct CharmZipCore<'a> {
    charm: &'a Charm,
    cache: &'a KVStoreCache,
    vehicle: &'a VehicleMeta,
    // to strip off the start of found links properly
    car_uri_components: &'a CarUriComponents,
    short_name_counter: Cell<u64>,
}

enum AdjustCtx {
    Page,
    Hyperlink,
    Image(KVKey),
    Static,
}

enum WriteCtx {
    Page(PageOffset, CanonicalUriPath),
    Image(Vec<u8>),
}

impl<'a> ZipCore for CharmZipCore<'a> {
    type AdjustCtx = AdjustCtx;
    type WriteCtx = WriteCtx;

    fn adjust_uri(
        &self,
        absolute_original_uri: &AbsoluteOriginalUri,
        ctx: AdjustCtx,
    ) -> Result<(AbsoluteAdjustedUri, Option<Self::WriteCtx>)> {
        match ctx {
            AdjustCtx::Page => {
                let (canonical_uri, changed) = absolute_original_uri.0.clone().canonicalize();
                if canonical_uri.dirs().is_empty() {
                    bail!("empty canonical dirs");
                }
                if canonical_uri.dirs()[0].as_str() == "hyperlink" {
                    bail!("hyperlink but had page adjust ctx");
                }
                if changed {
                    bail!(
                        "URI changed during canonicalization (charm zip): {absolute_original_uri:?} to {canonical_uri:?}"
                    );
                }
                let (aup, write_ctx) = self.adjust_page_uri(canonical_uri)?;
                Ok((AbsoluteAdjustedUri(aup), write_ctx))
            }
            AdjustCtx::Hyperlink => {
                let (canonical_aou, _) = absolute_original_uri.0.clone().canonicalize();
                // for some godforsaken reason, the hyperlinks are not
                // canonical, so it's ok for them to change here. Yes,
                // this means that every hyperlink request has to go
                // through a 308 redirect and an extra network round
                // trip the way we architected it. Shitfuck.
                let resolved: AbsoluteUriPath =
                    match self.charm.resolve_hyperlink(self.cache, &canonical_aou)? {
                        Some(r) => r,
                        None => return Ok((aau_404(), None)),
                    };
                // we need to extract and resolve the non-fragment portion of the URI
                if resolved.file().is_some() {
                    bail!("hyperlink pointed to a file: {canonical_aou:?} to {resolved:?}");
                }
                let resolved_dirs_canonical = CanonicalUriPath {
                    dirs: resolved.dirs().into(),
                };
                let (mut aup, _write_ctx) = self.adjust_page_uri(resolved_dirs_canonical)?;
                // do not actually write, this link will be found by other means and written there.
                // reattach the fragment
                aup.fragment = resolved.fragment;
                Ok((AbsoluteAdjustedUri(aup), None))
            }
            AdjustCtx::Image(image_key) => {
                let image_uri = &absolute_original_uri.0;
                if image_uri.dirs().len() < 2 || image_uri.dirs()[0].as_str() != "images" {
                    bail!(
                        "Something has gone horribly wrong. We've checked this invariant many times already"
                    );
                }
                let image_bytes = match self.charm.retrieve_image(&image_key, Some(self.cache))? {
                    Some(o) => o,
                    None => return Ok((aau_404(), None)),
                };
                let image_type = match ImageType::guess(&image_bytes) {
                    Some(i) => i,
                    // some images in CHARM are unknown type.
                    // BACKLOG would be better to have a special 404 image instead.
                    None => return Ok((aau_404(), None)),
                };
                let file_name = UriComponent::from_encoded_str(&format!(
                    "{}.{}",
                    image_uri.dirs().last().unwrap().as_str(),
                    image_type.file_extension()
                ))?;
                Ok((
                    AbsoluteAdjustedUri(AbsoluteUriPath {
                        dirs: image_uri.dirs()[0..image_uri.dirs().len() - 1].into(),
                        file: Some(file_name),
                        fragment: None,
                    }),
                    Some(WriteCtx::Image(image_bytes)),
                ))
            }
            AdjustCtx::Static => bail!(
                "Shouldn't ever actually have to adjust a Static page (should be inserted into adjustment mapping beforehand"
            ),
        }
    }

    fn write(
        &self,
        scoped_zipper: &mut ScopedZipper<Self>,
        ctx: WriteCtx,
    ) -> Result<(Vec<u8>, bool)> {
        match ctx {
            WriteCtx::Page(offset, canonical_aou) => {
                let content = self.charm.retrieve_page_2(self.cache, offset)?;
                let outer_html = self.charm.page_string_to_outer_html(
                    self.cache,
                    &content,
                    &canonical_aou,
                    self.vehicle,
                )?;
                let mut global_err = None;
                let outer_html =
                    ZIPPER_HYPERLINK_REGEX.replace_all(&outer_html, |caps: &regex::Captures| {
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
                                "Improperly encoded URI found in charm zip: {improper:?}"
                            ));
                            return String::new();
                        }
                        let proper_full = proper_server.into();
                        let rou = RelativeOriginalUri(proper_full);
                        let aou = scoped_zipper.rou_to_aou(rou);
                        let relative_adjusted_uri = match self
                            .charm
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
                    None => Ok((outer_html.into_owned().into_bytes(), true)),
                }
            }
            WriteCtx::Image(bytes) => Ok((bytes, false)),
        }
    }
}

impl<'a> CharmZipCore<'a> {
    // splitting this out so we can use it from both the page and hyperlink adjustment paths
    // checks for the page's existence nand returns the appropriate (aau, writectx) pair
    fn adjust_page_uri(
        &self,
        uri: CanonicalUriPath,
    ) -> Result<(AbsoluteUriPath, Option<WriteCtx>)> {
        let offset = self.charm.retrieve_page_1(self.cache, self.vehicle, &uri)?;
        Ok(offset
            .map(|offset| {
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
                (uri_path, Some(WriteCtx::Page(offset, uri)))
            })
            .unwrap_or_else(|| (aup_404(), None)))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn prop(s: &str) -> UriComponent {
        UriComponent::unsafe_from_encoded_str(s)
    }

    #[test]
    fn test_canonicalize_make_in_uri_path() {
        assert_eq!(
            canonicalize_make_in_uri_path(&CanonicalUriPath {
                dirs: vec![prop("Chevrolet"), prop("Caravan")],
            })
            .unwrap(),
            None
        );
        assert_eq!(
            canonicalize_make_in_uri_path(&CanonicalUriPath {
                dirs: vec![prop("Dodge"), prop("Terminator")],
            })
            .unwrap(),
            Some(CanonicalUriPath {
                dirs: vec![prop("Dodge%20and%20Ram"), prop("Terminator")],
            })
        );
    }
}
