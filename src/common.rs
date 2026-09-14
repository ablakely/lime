use std::{hash::Hash, io::Write, sync::LazyLock};

use anyhow::{Result, anyhow, bail};
use axum::{http::StatusCode, response::IntoResponse};
use elsa::FrozenMap;
use plait::{HtmlDisplay, html};
use regex::Regex;
use serde::Deserialize;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    types::{ApiBreadcrumb, NamedUri, Year},
    uri_path::{
        AbsoluteUriPath, CanonicalUriPath, CarUriComponents, ServerUriPath, UriComponent, UriPath,
        car_uri_path_string_to_car_uri_components, parse_uri_path,
    },
    zipper::{AbsoluteAdjustedUri, AbsoluteOriginalUri},
};

static A_HREF_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"(?is)<a[^>]*href=['"]([^'"#]+)(?:#[^'"]*)?['"][^>]*>(.*?)</a>"##).unwrap()
});
static STRIP_HTML_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<[^>]+>").unwrap());

pub fn deserialize_years<'de, D>(deserializer: D) -> Result<Vec<Year>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let years: Vec<String> = Deserialize::deserialize(deserializer)?;
    if years.is_empty() {
        return Err(serde::de::Error::custom("years array cannot be empty"));
    }
    Ok(years.into_iter().map(Year::new).collect())
}

pub fn deserialize_car_uri_components<'de, D>(deserializer: D) -> Result<CarUriComponents, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let uri_string: String = Deserialize::deserialize(deserializer)?;
    match car_uri_path_string_to_car_uri_components(&uri_string) {
        Some(components) => Ok(components),
        None => Err(serde::de::Error::custom("car URI was not valid")),
    }
}

/// Return up to 3 uri components as breadcrumbs
pub fn car_breadcrumbs(uri_path: &impl UriPath) -> Vec<Breadcrumb> {
    let mut result = Vec::with_capacity(3);
    for (i, uri_component) in uri_path.dirs().iter().take(3).enumerate() {
        result.push((
            uri_component.clone(),
            AbsoluteUriPath {
                dirs: uri_path.dirs()[0..i + 1].into(),
                file: None,
                fragment: None,
            },
        ));
    }
    result
}

#[derive(Clone)]
pub struct SiteBranding {
    pub name: String,
    pub slogan: String,
    pub latin_phrase: String,
    pub announcement: Option<String>,
}

pub fn add_header_and_footer(
    branding: &SiteBranding,
    inner_html: &str,
    breadcrumbs: &[Breadcrumb],
    breadcrumbs_need_more_context_predicate: impl Fn(&[Breadcrumb]) -> bool,
) -> String {
    let h1_title = breadcrumbs_to_title(breadcrumbs, breadcrumbs_need_more_context_predicate);
    let seo_title = breadcrumbs_to_seo_title(branding, breadcrumbs, &h1_title);
    let seo_description = breadcrumbs_to_seo_description(branding, breadcrumbs);

    html! {
        #doctype
        html {
            head {
                meta(charset: "utf-8");
                title { (&seo_title) }
                link(rel: "stylesheet", href: "/style.css");
                meta(name: "viewport", content: "width=device-width, initial-scale=1.0");
                meta(name: "description", content: (&seo_description));
            }
            body {
                div(class: "theme-colors header") {
                    div(class: "branding") {
                        b { (&branding.name) } ": " (&branding.slogan)
                    }
                    @(&breadcrumbs_to_html(breadcrumbs))
                }
                div(class: "main") {
                    h1 { (&h1_title) }
                    if let Some(announcement) = branding.announcement.as_ref() {
                        div(class: "other-warning other-announcement") {
                            #(announcement)
                        }
                    }
                    #(inner_html)
                }
                div(class: "theme-colors footer") {
                    i { (&branding.latin_phrase) } " · " a(href: "/about.html") { "About " (&branding.name) }
                }
                script(src: "/script.js") {}
            }
        }
    }
    .to_string()
}

pub type Breadcrumb = (UriComponent, AbsoluteUriPath);

fn builtin_bcs_need_more_context_predicate(bcs: &[Breadcrumb]) -> bool {
    bcs.len() == 3
        || bcs
            .last()
            .map(|x| x.0.as_str().chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
}

pub fn breadcrumbs_to_title(
    mut bcs: &[Breadcrumb],
    breadcrumbs_need_more_context_predicate: impl Fn(&[Breadcrumb]) -> bool,
) -> String {
    if bcs.is_empty() {
        return "Home: All Service Manuals".to_string();
    }
    let mut result = bcs.last().unwrap().0.decode_uri_component().0.into_owned();
    while builtin_bcs_need_more_context_predicate(bcs)
        || breadcrumbs_need_more_context_predicate(bcs)
    {
        bcs = &bcs[0..bcs.len() - 1];
        if bcs.is_empty() {
            log::error!("Tried to get more context on a singular breadcrumb");
            break;
        }
        result = format!(
            "{}: {}",
            bcs.last().unwrap().0.decode_uri_component().0,
            result
        );
    }
    result
}

pub fn breadcrumbs_to_api_breadcrumbs(breadcrumbs: &[Breadcrumb]) -> Vec<ApiBreadcrumb> {
    breadcrumbs
        .iter()
        .map(|(label, href)| ApiBreadcrumb {
            label: label.decode_uri_component().0.into_owned(),
            href: String::from(href.stringify()),
        })
        .collect()
}

pub fn breadcrumbs_to_topics(breadcrumbs: &[Breadcrumb]) -> Vec<String> {
    breadcrumbs
        .iter()
        .skip(3)
        .map(|(label, _)| label.decode_uri_component().0.into_owned())
        .collect()
}

pub fn manual_links_from_html(current_uri: &CanonicalUriPath, html: &str) -> Vec<NamedUri> {
    let mut links = Vec::new();
    for captures in A_HREF_REGEX.captures_iter(html) {
        let href = captures.get(1).unwrap().as_str();
        if !href.starts_with('/') {
            continue;
        }
        let Ok(parsed_href) = parse_uri_path(href) else {
            continue;
        };
        let Ok((parsed_href, _)) = parsed_href.reencode_properly() else {
            continue;
        };
        if parsed_href.file.is_some() || !parsed_href.is_absolute {
            continue;
        }
        if parsed_href.dirs.len() <= current_uri.dirs.len()
            || !parsed_href.dirs.starts_with(current_uri.dirs())
        {
            continue;
        }
        let label = STRIP_HTML_REGEX
            .replace_all(captures.get(2).unwrap().as_str(), "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if label.is_empty() || links.iter().any(|link: &NamedUri| link.uri == href) {
            continue;
        }
        links.push(NamedUri {
            name: label,
            uri: href.to_string(),
        });
    }
    links
}

/// Precondition: breadcrumbs nonempty
fn breadcrumbs_to_car_name(breadcrumbs: &[Breadcrumb]) -> String {
    match breadcrumbs {
        [] => panic!("Cannot call breadcrumbs_to_car_name with empty breadcrumbs"),

        [(make, _)] => make.decode_uri_component().0.to_string(),

        [(make, _), (year, _)] => format!(
            "{} {}",
            year.decode_uri_component().0,
            make.decode_uri_component().0
        ),

        [(make, _), (year, _), (model, _), ..] => {
            format!(
                "{} {} {}",
                year.decode_uri_component().0,
                make.decode_uri_component().0,
                model.decode_uri_component().0
            )
        }
    }
}

fn breadcrumbs_to_seo_title(
    branding: &SiteBranding,
    breadcrumbs: &[Breadcrumb],
    h1_title: &str,
) -> String {
    match breadcrumbs.len() {
        0 => format!("Free Car Service Manuals from {}", branding.name),
        1 | 2 => format!(
            "Free Service Manuals for {} vehicles ~ {}",
            breadcrumbs_to_car_name(breadcrumbs),
            branding.name
        ),
        3 => format!(
            "Free Service Manual for the {} ~ {}",
            breadcrumbs_to_car_name(breadcrumbs),
            branding.name
        ),
        _ => format!(
            "{} — {} Service Manual ~ {}",
            h1_title,
            breadcrumbs_to_car_name(breadcrumbs),
            branding.name,
        ),
    }
}

fn breadcrumbs_to_seo_description(branding: &SiteBranding, breadcrumbs: &[Breadcrumb]) -> String {
    match breadcrumbs.len() {
        0 => format!(
            "{} provides free repair/service/workshop manuals for about 10,000 US and Canada market vehicles from 1960-2025. No sign-up, no paywall!",
            branding.name
        ),
        1 | 2 => format!(
            "Repair/service/workshop manuals for many {} vehicles: electrical diagrams, bolt torques, labor times, and more.",
            breadcrumbs_to_car_name(breadcrumbs)
        ),
        _ => format!(
            "Repair manual for the {}, including electrical diagrams, bolt torques, and labor times.",
            breadcrumbs_to_car_name(breadcrumbs)
        ),
    }
}

fn breadcrumbs_to_html(breadcrumbs: &[Breadcrumb]) -> impl HtmlDisplay {
    let root_href = AbsoluteUriPath {
        dirs: vec![],
        file: None,
        fragment: None,
    };
    let root_component = UriComponent::unsafe_from_encoded_str("Home");
    let root_breadcrumb = (root_component, root_href);
    html! {
        @(&breadcrumb_to_html(&root_breadcrumb))
        for breadcrumb in breadcrumbs {
            " >> "
            @(&breadcrumb_to_html(breadcrumb))
        }
    }
}

fn breadcrumb_to_html(breadcrumb: &Breadcrumb) -> impl HtmlDisplay {
    html! {
        @(&safe_a(Some("breadcrumb-part"), &breadcrumb.1, html! { (breadcrumb.0.decode_uri_component().0) }))
    }
}

pub fn safe_a(
    class: Option<&str>,
    href: &impl UriPath,
    content: impl HtmlDisplay,
) -> impl HtmlDisplay {
    html! {
        a(class?: class, href: #(&String::from(href.stringify()))) { @(&content) }
    }
}

pub fn english_list<S: AsRef<str>>(list: &[S]) -> String {
    match list {
        [one] => one.as_ref().to_string(),
        [one, two] => format!("{} and {}", one.as_ref(), two.as_ref()),
        _ => {
            let mut result = String::new();
            for (i, elt) in list.iter().enumerate() {
                if i != 0 {
                    result += ", ";
                }
                if i == list.len() - 1 {
                    result += "and ";
                }
                result += elt.as_ref()
            }
            result
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::uri_path::{AbsoluteUriPath, UriComponent};

    fn absolute_path(dirs: &[&str]) -> AbsoluteUriPath {
        AbsoluteUriPath {
            dirs: dirs
                .iter()
                .map(|dir| UriComponent::unsafe_from_encoded_str(dir))
                .collect(),
            file: None,
            fragment: None,
        }
    }

    #[test]
    fn breadcrumbs_to_topics_skips_car_identity() {
        let breadcrumbs = vec![
            (
                UriComponent::unsafe_from_encoded_str("Chevrolet"),
                absolute_path(&["Chevrolet"]),
            ),
            (
                UriComponent::unsafe_from_encoded_str("2003"),
                absolute_path(&["Chevrolet", "2003"]),
            ),
            (
                UriComponent::unsafe_from_encoded_str("Suburban%20C2500%2C%208.1%20G"),
                absolute_path(&["Chevrolet", "2003", "Suburban%20C2500%2C%208.1%20G"]),
            ),
            (
                UriComponent::unsafe_from_encoded_str("Repair%20and%20Diagnosis"),
                absolute_path(&[
                    "Chevrolet",
                    "2003",
                    "Suburban%20C2500%2C%208.1%20G",
                    "Repair%20and%20Diagnosis",
                ]),
            ),
            (
                UriComponent::unsafe_from_encoded_str("Engine"),
                absolute_path(&[
                    "Chevrolet",
                    "2003",
                    "Suburban%20C2500%2C%208.1%20G",
                    "Repair%20and%20Diagnosis",
                    "Engine",
                ]),
            ),
        ];

        assert_eq!(
            breadcrumbs_to_topics(&breadcrumbs),
            vec!["Repair and Diagnosis", "Engine"]
        );
    }

    #[test]
    fn manual_links_from_html_keeps_descendant_manual_paths() {
        let current_uri = CanonicalUriPath {
            dirs: vec![
                UriComponent::unsafe_from_encoded_str("Buick"),
                UriComponent::unsafe_from_encoded_str("2012"),
                UriComponent::unsafe_from_encoded_str("LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203"),
                UriComponent::unsafe_from_encoded_str("Repair%20and%20Diagnosis"),
            ],
        };
        let html = r#"
            <a href="/Buick/2012/LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203/Repair%20and%20Diagnosis/Engine/">Engine</a>
            <a href="/Buick/2012/LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203/">Vehicle Root</a>
            <a href="/about.html">About</a>
        "#;
        assert_eq!(
            manual_links_from_html(&current_uri, html),
            vec![NamedUri {
                name: "Engine".to_string(),
                uri: "/Buick/2012/LaCrosse%20Leather%2C%203.6L%20Eng%20VIN%203/Repair%20and%20Diagnosis/Engine/".to_string(),
            }]
        );
    }
}

pub fn image_bytes_to_response(bytes: Vec<u8>) -> axum::response::Response {
    match ImageType::guess(&bytes) {
        Some(image_type) => ([(axum::http::header::CONTENT_TYPE, image_type.mime())], bytes).into_response(),
        None => (StatusCode::INTERNAL_SERVER_ERROR, "Malformed/unknown image type. If you see this on a CHARM manual, it's a known issue. If you see this on a LEMON manual, it's a bug, please email us: lemon-manuals@protonmail.com").into_response(),
    }
}

pub enum ImageType {
    Gif,
    Png,
    Jpeg,
    Svg,
}

impl ImageType {
    pub fn guess(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0x47, 0x49, 0x46, ..] => Some(Self::Gif),
            [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some(Self::Png),
            [255, 216, 255, ..] => Some(Self::Jpeg),

            [60, ..] => Some(Self::Svg),
            _ => None,
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Self::Gif => "image/gif",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Svg => "image/svg+xml",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Svg => "svg",
        }
    }
}

pub fn get_or_compute<'a, K, V>(
    map: &'a FrozenMap<K, Box<V>>,
    key: &K,
    compute: impl FnOnce() -> Result<Box<V>>,
) -> Result<&'a V>
where
    K: Clone + Hash + Eq,
{
    match map.get(key) {
        Some(v) => Ok(v),
        None => Ok(map.insert(key.clone(), compute()?)),
    }
}

pub struct SenderWriter {
    sender: tokio::sync::mpsc::Sender<Result<Vec<u8>>>,
}

impl Write for SenderWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        match self.sender.blocking_send(Ok(Vec::from(buf))) {
            Ok(()) => Ok(len),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Client disconnected",
            )),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn make_writer_to_bytes_stream() -> (SenderWriter, ReceiverStream<Result<Vec<u8>>>) {
    let (sender, receiver) = tokio::sync::mpsc::channel(25);
    (SenderWriter { sender }, ReceiverStream::new(receiver))
}

pub fn make_zip_static_files(
    included_dir: &include_dir::Dir,
    files: &[(&'static str, &'static str)],
) -> Result<Vec<(ServerUriPath, Vec<u8>)>> {
    let mut zip_static_files = vec![];
    for (include_dir_file_name, zip_file_name) in files {
        let included_file = included_dir
            .get_file(include_dir_file_name)
            .ok_or_else(|| anyhow!("Didn't find lemon zip static file: {include_dir_file_name}"))?;
        let uri_path = parse_uri_path(zip_file_name)?;
        let (mut uri_path, changed) = uri_path.reencode_properly()?;
        if changed {
            bail!("a uri path changed during make zip static files: {uri_path:?}");
        }
        if uri_path.is_absolute() {
            bail!("By convention you should only pass relative uris to make_zip_static_files");
        }

        uri_path.is_absolute = true;
        zip_static_files.push((uri_path.try_into()?, included_file.contents().into()))
    }
    Ok(zip_static_files)
}

pub fn aup_404() -> AbsoluteUriPath {
    AbsoluteUriPath {
        dirs: vec![],
        file: Some(UriComponent::unsafe_from_encoded_str("404.html")),
        fragment: None,
    }
}

pub fn aau_404() -> AbsoluteAdjustedUri {
    AbsoluteAdjustedUri(aup_404())
}

pub fn aou_404() -> AbsoluteOriginalUri {
    AbsoluteOriginalUri(ServerUriPath {
        dirs: vec![],
        file: Some(UriComponent::unsafe_from_encoded_str("404.html")),
    })
}

pub fn car_uri_components_to_human_readable_file_name(
    car_uri_components: &CarUriComponents,
) -> String {
    let car_human_readable_name = format!(
        "{} {} {}",
        car_uri_components[1].decode_uri_component().0,
        car_uri_components[0].decode_uri_component().0,
        car_uri_components[2].decode_uri_component().0
    );
    car_human_readable_name
        .chars()
        .map(|c| if c == '/' { '-' } else { c })
        .collect()
}
