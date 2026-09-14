






use std::{borrow::Cow, fmt::Display};

use anyhow::{Error, Result, anyhow, bail};

use crate::common::{Breadcrumb, car_breadcrumbs};




#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct UriComponent(String);

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct UriComponentImproperlyEncoded<'a>(pub Cow<'a, str>);
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct UriComponentDecoded<'a>(pub Cow<'a, str>);
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct UriFragment(String);


const PROPER_ENCODE_SET: &percent_encoding_rfc3986::AsciiSet =
    &percent_encoding_rfc3986::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');

impl<'a> UriComponentDecoded<'a> {
    pub fn encode_uri_component(&self) -> Result<UriComponent> {
        if self.0.is_empty() {
            bail!("empty strings are not proper UriComponents");
        }
        Ok(UriComponent(
            Cow::from(percent_encoding_rfc3986::utf8_percent_encode(
                self.0.as_ref(),
                PROPER_ENCODE_SET,
            ))
            .into_owned(),
        ))
    }
}

impl<'a> From<&'a UriComponent> for UriComponentImproperlyEncoded<'a> {
    fn from(properly_encoded: &'a UriComponent) -> UriComponentImproperlyEncoded<'a> {
        UriComponentImproperlyEncoded(Cow::Borrowed(&properly_encoded.0))
    }
}

impl<'a, 'b> UriComponentImproperlyEncoded<'a> {
    pub fn decode_uri_component(&self) -> Result<UriComponentDecoded<'b>> {
        Ok(UriComponentDecoded(Cow::Owned(
            percent_encoding_rfc3986::percent_decode_str(&self.0)
                .map_err(|e| anyhow!("Percent decode error: {e}"))?
                .decode_utf8()?
                .into_owned(),
        )))
    }
}

impl<'a> UriComponent {
    pub fn decode_uri_component(&self) -> UriComponentDecoded<'a> {
        UriComponentImproperlyEncoded::from(self)
            .decode_uri_component()
            .expect("Failed to decode a properly encoded URI!!!")
            .clone()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub trait UriPath {
    fn dirs(&self) -> &[UriComponent];
    fn file(&self) -> Option<&UriComponent>;
    fn is_absolute(&self) -> bool;
    fn fragment(&self) -> Option<&UriFragment>;

    fn stringify(&self) -> StringifiedUriPath {
        let estimated_capacity: usize = self
            .dirs()
            .iter()
            .map(|x| x.as_str().len() + 1)
            .sum::<usize>()
            + self.file().map(|x| x.as_str().len()).unwrap_or(0)
            + self.fragment().map(|x| x.0.len()).unwrap_or(0);
        let mut result = String::with_capacity(estimated_capacity + 20);
        if self.is_absolute() {
            result += "/";
        }
        for dir in self.dirs() {
            result += &dir.0;
            result += "/";
        }
        if let Some(file) = self.file() {
            result += &file.0;
        }
        if let Some(fragment) = self.fragment() {
            result += "#";
            result += &fragment.0;
        }
        StringifiedUriPath(result)
    }

    fn extract_car_uri_components(&self) -> Option<CarUriComponents> {
        let dirs = self.dirs();
        (dirs.len() >= 3).then(|| [dirs[0].clone(), dirs[1].clone(), dirs[2].clone()])
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct FullUriPath {
    pub dirs: Vec<UriComponent>,
    pub file: Option<UriComponent>,
    pub is_absolute: bool,
   
    pub fragment: Option<UriFragment>,
}

impl UriPath for FullUriPath {
    fn dirs(&self) -> &[UriComponent] {
        &self.dirs
    }

    fn file(&self) -> Option<&UriComponent> {
        self.file.as_ref()
    }

    fn fragment(&self) -> Option<&UriFragment> {
        self.fragment.as_ref()
    }

    fn is_absolute(&self) -> bool {
        self.is_absolute
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ServerUriPath {
    pub dirs: Vec<UriComponent>,
    pub file: Option<UriComponent>,
}

impl From<ServerUriPath> for FullUriPath {
    fn from(value: ServerUriPath) -> Self {
        Self {
            fragment: value.fragment().cloned(),
            is_absolute: value.is_absolute(),
            dirs: value.dirs,
            file: value.file,
        }
    }
}

impl UriPath for ServerUriPath {
    fn dirs(&self) -> &[UriComponent] {
        &self.dirs
    }

    fn file(&self) -> Option<&UriComponent> {
        self.file.as_ref()
    }

    fn fragment(&self) -> Option<&UriFragment> {
        None
    }

    fn is_absolute(&self) -> bool {
        true
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct FragmentlessUriPath {
    pub dirs: Vec<UriComponent>,
    pub file: Option<UriComponent>,
    pub is_absolute: bool,
}

impl UriPath for FragmentlessUriPath {
    fn dirs(&self) -> &[UriComponent] {
        &self.dirs
    }

    fn file(&self) -> Option<&UriComponent> {
        self.file.as_ref()
    }

    fn is_absolute(&self) -> bool {
        self.is_absolute
    }

    fn fragment(&self) -> Option<&UriFragment> {
        None
    }
}

impl TryFrom<FragmentlessUriPath> for ServerUriPath {
    type Error = Error;

    fn try_from(value: FragmentlessUriPath) -> Result<Self> {
        if value.is_absolute {
            Ok(Self {
                dirs: value.dirs,
                file: value.file,
            })
        } else {
            Err(anyhow!(
                "Can't convert relative fragmentless to server uri path: {value:?}"
            ))
        }
    }
}

impl From<FragmentlessUriPath> for FullUriPath {
    fn from(value: FragmentlessUriPath) -> Self {
        Self {
            fragment: value.fragment().cloned(),
            dirs: value.dirs,
            file: value.file,
            is_absolute: value.is_absolute,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CanonicalUriPath {
    pub dirs: Vec<UriComponent>,
}

impl UriPath for CanonicalUriPath {
    fn dirs(&self) -> &[UriComponent] {
        &self.dirs
    }

    fn file(&self) -> Option<&UriComponent> {
        None
    }

    fn fragment(&self) -> Option<&UriFragment> {
        None
    }

    fn is_absolute(&self) -> bool {
        true
    }
}

impl From<CanonicalUriPath> for AbsoluteUriPath {
    fn from(value: CanonicalUriPath) -> Self {
        Self {
            dirs: value.dirs,
            file: None,
            fragment: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct AbsoluteUriPath {
    pub dirs: Vec<UriComponent>,
    pub file: Option<UriComponent>,
    pub fragment: Option<UriFragment>,
}

impl UriPath for AbsoluteUriPath {
    fn dirs(&self) -> &[UriComponent] {
        &self.dirs
    }

    fn file(&self) -> Option<&UriComponent> {
        self.file.as_ref()
    }

    fn fragment(&self) -> Option<&UriFragment> {
        self.fragment.as_ref()
    }

    fn is_absolute(&self) -> bool {
        true
    }
}

impl From<ServerUriPath> for AbsoluteUriPath {
    fn from(value: ServerUriPath) -> Self {
        Self {
            dirs: value.dirs,
            file: value.file,
            fragment: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct RelativeUriPath {
    pub dirs: Vec<UriComponent>,
    pub file: Option<UriComponent>,
    pub fragment: Option<UriFragment>,
}

impl UriPath for RelativeUriPath {
    fn dirs(&self) -> &[UriComponent] {
        &self.dirs
    }

    fn file(&self) -> Option<&UriComponent> {
        self.file.as_ref()
    }

    fn fragment(&self) -> Option<&UriFragment> {
        self.fragment.as_ref()
    }

    fn is_absolute(&self) -> bool {
        false
    }
}


#[derive(Debug, PartialEq, Eq)]
pub struct FragmentlessUriPathImproperlyEncoded<'a> {
    dirs: Vec<UriComponentImproperlyEncoded<'a>>,
    file: Option<UriComponentImproperlyEncoded<'a>>,
    is_absolute: bool,
}

pub struct StringifiedUriPath(String);

impl From<StringifiedUriPath> for String {
    fn from(value: StringifiedUriPath) -> Self {
        value.0
    }
}

impl Display for StringifiedUriPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl UriComponent {
    pub fn unsafe_from_encoded_str(string: &str) -> Self {
        assert!(!string.is_empty(), "cannot make an empty uri component");
        assert!(
            !string.contains('/'),
            "cannot make a uri component with a slash"
        );
        Self(string.to_string())
    }

    /// re-encodes
    pub fn from_encoded_str(string: &str) -> Result<Self> {
        UriComponentImproperlyEncoded(Cow::Borrowed(string)).reencode_properly()
    }

    pub fn from_decoded_str(string: &str) -> Result<Self> {
        UriComponentDecoded(Cow::Borrowed(string)).encode_uri_component()
    }
}

impl UriComponentImproperlyEncoded<'_> {
    pub fn reencode_properly(&self) -> Result<UriComponent> {
        self.decode_uri_component()?.encode_uri_component()
    }
}


pub fn dirs_to_relative_fragment(dirs: &[UriComponent]) -> UriFragment {
    let mut fragment = String::new();
    for dir in dirs {
        fragment += &dir.0;
        fragment += "/";
    }
    UriFragment(fragment)
}


pub fn parse_uri_path(path_string: &str) -> Result<FragmentlessUriPathImproperlyEncoded<'_>> {
    if path_string.is_empty() {
        bail!("valid URI paths are not empty");
    }
   
    let components_vec: Vec<&str> = path_string.split("/").collect();
    let mut components: &[&str] = &components_vec;
    let is_absolute = components[0].is_empty();
    if is_absolute {
        components = &components[1..];
    }
    let final_component = components.last().unwrap();
    components = &components[0..components.len() - 1];
    let file = if !final_component.is_empty() {
        Some(UriComponentImproperlyEncoded(Cow::Borrowed(
            final_component,
        )))
    } else {
        None
    };
    let dirs = components
        .iter()
        .map(|component| UriComponentImproperlyEncoded(Cow::Borrowed(component)))
        .collect();
    Ok(FragmentlessUriPathImproperlyEncoded {
        dirs,
        file,
        is_absolute,
    })
}

impl<'a> FragmentlessUriPathImproperlyEncoded<'a> {
    /// The bool is whether the path changed at all during re-encoding.
    pub fn reencode_properly(&self) -> Result<(FragmentlessUriPath, bool)> {
        let file = self
            .file
            .as_ref()
            .map(|c| c.reencode_properly())
            .transpose()?;
        let mut any_empty_dirs = false;
        let dirs: Vec<UriComponent> = self
            .dirs
            .iter()
            .filter(|c| {
                if c.0.is_empty() {
                    any_empty_dirs = true;
                    false
                } else {
                    true
                }
            })
            .map(|c| c.reencode_properly())
            .collect::<Result<Vec<UriComponent>>>()?;
        let reencodings_differ = file.as_ref().map(|c| c.0.as_ref())
            != self.file.as_ref().map(|c| c.0.as_ref())
            || !dirs
                .iter()
                .map(|c| &c.0)
                .eq(self.dirs.iter().map(|c| c.0.as_ref()));
        Ok((
            FragmentlessUriPath {
                dirs,
                file,
                is_absolute: self.is_absolute,
            },
            reencodings_differ,
        ))
    }
}

impl ServerUriPath {
    /// Bool is true if the uri changed at all in the process of becoming canonical
    pub fn canonicalize(self) -> (CanonicalUriPath, bool) {
        match self.file {
            Some(file) => {
                let mut dirs = self.dirs;
                dirs.push(file);
                (CanonicalUriPath { dirs }, true)
            }
            None => (CanonicalUriPath { dirs: self.dirs }, false),
        }
    }
}

pub type CarUriComponents = [UriComponent; 3];

/// Only meant for converting a "car URI path", which is a uri path with exactly three components (not more). For getting the car portion of a larger URI, simply uri_path_string_to_components then extract_car_uri_components
pub fn car_uri_path_string_to_car_uri_components(path_string: &str) -> Option<CarUriComponents> {
   
    parse_uri_path(path_string)
        .ok()
        .and_then(|parsed| parsed.reencode_properly().ok())
        .and_then(|(reencoded, changed)| {
            (!changed && reencoded.file().is_none() && reencoded.dirs().len() == 3).then_some([
                reencoded.dirs()[0].clone(),
                reencoded.dirs()[1].clone(),
                reencoded.dirs()[2].clone(),
            ])
        })
}

pub fn car_uri_components_to_uri_path(car_uri_components: &CarUriComponents) -> impl UriPath {
    CanonicalUriPath {
        dirs: car_uri_components.into(),
    }
}

pub fn join_absolute_relative_uri_path(base: &ServerUriPath, link: &impl UriPath) -> ServerUriPath {
    let dirs: Vec<UriComponent> = if link.is_absolute() {
        link.dirs().into()
    } else {
        let mut res: Vec<UriComponent> = base.dirs().into();
        res.extend_from_slice(link.dirs());
        res
    };

    ServerUriPath {
        dirs,
        file: link.file().cloned(),
    }
}

pub fn absolute_to_relative(base: &AbsoluteUriPath, link: &AbsoluteUriPath) -> RelativeUriPath {
    let mut dirs = vec![UriComponent::unsafe_from_encoded_str(".."); base.dirs().len()];
    dirs.extend_from_slice(link.dirs());
    RelativeUriPath {
        dirs,
        file: link.file.clone(),
        fragment: link.fragment.clone(),
    }
}

pub enum ConcretizeResult {
    Found,
    NotFoundYet,
    NotFoundEver,
}

/// Call a function on prefixes of decreasing length of the given canonical uri. If the predicate returns Found, the prefix is made the path, and remainder made fragment. If NotFoundYet, shortens the prefix and tries again. If NotFoundEver, it's a shortcut
pub fn concretize_uri(
    uri: &CanonicalUriPath,
    concrete_predicate: &impl Fn(&[UriComponent]) -> Result<ConcretizeResult>,
) -> Result<Option<AbsoluteUriPath>> {
    let mut num_concrete_parts = uri.dirs().len();
    while num_concrete_parts > 0 {
        match concrete_predicate(&uri.dirs()[0..num_concrete_parts])? {
            ConcretizeResult::Found => {
                return Ok(Some(AbsoluteUriPath {
                    dirs: uri.dirs()[0..num_concrete_parts].into(),
                    fragment: (num_concrete_parts < uri.dirs().len())
                        .then(|| dirs_to_relative_fragment(&uri.dirs()[num_concrete_parts..])),
                    file: None,
                }));
            }
            ConcretizeResult::NotFoundYet => {
                num_concrete_parts -= 1;
            }
            ConcretizeResult::NotFoundEver => {
                return Ok(None);
            }
        }
    }
    Ok(None)
}

pub fn make_breadcrumbs(
    uri_path: &CanonicalUriPath,
    concrete_predicate: &impl Fn(&[UriComponent]) -> Result<bool>,
) -> Result<Vec<Breadcrumb>> {
    let mut breadcrumbs = car_breadcrumbs(uri_path);
    let dirs = uri_path.dirs();
    let mut concrete_components_so_far = &dirs[0..3];
    for uri_components_prefix_len in 4..=dirs.len() {
        let uri_components_prefix = &dirs[0..uri_components_prefix_len];
        let cur_is_concrete = concrete_predicate(uri_components_prefix)?;
        if cur_is_concrete {
            concrete_components_so_far = &dirs[0..uri_components_prefix_len];
        }
        let breadcrumb_uri_path = AbsoluteUriPath {
            dirs: concrete_components_so_far.into(),
            file: None,
            fragment: (!cur_is_concrete).then(|| {
                dirs_to_relative_fragment(
                    &uri_components_prefix[concrete_components_so_far.len()..],
                )
            }),
        };
        breadcrumbs.push((
            uri_components_prefix.last().unwrap().clone(),
            breadcrumb_uri_path,
        ));
    }
    Ok(breadcrumbs)
}

#[cfg(test)]
mod test {
    use super::*;

    fn improp(s: &str) -> UriComponentImproperlyEncoded<'_> {
        UriComponentImproperlyEncoded(Cow::Borrowed(s))
    }

    fn prop(s: &str) -> UriComponent {
        assert!(!s.is_empty());
        UriComponent(s.to_string())
    }

    #[test]
    fn test_parse_uri_path_string() {
        parse_uri_path("").unwrap_err();
        assert_eq!(
            parse_uri_path("/").unwrap(),
            FragmentlessUriPathImproperlyEncoded {
                dirs: vec![],
                file: None,
                is_absolute: true,
            },
        );
        assert_eq!(
            parse_uri_path("/hi/").unwrap(),
            FragmentlessUriPathImproperlyEncoded {
                dirs: vec![improp("hi")],
                file: None,
                is_absolute: true,
            }
        );
        assert_eq!(
            parse_uri_path("/hello/blah").unwrap(),
            FragmentlessUriPathImproperlyEncoded {
                dirs: vec![improp("hello")],
                file: Some(improp("blah")),
                is_absolute: true,
            }
        );
        assert_eq!(
            parse_uri_path("hello/blah").unwrap(),
            FragmentlessUriPathImproperlyEncoded {
                dirs: vec![improp("hello")],
                file: Some(improp("blah")),
                is_absolute: false,
            }
        );
    }

    #[test]
    fn test_reencode_uri_path_properly() {
        assert_eq!(
            parse_uri_path("/hello/world")
                .unwrap()
                .reencode_properly()
                .unwrap(),
            (
                FragmentlessUriPath {
                    dirs: vec![prop("hello")],
                    file: Some(prop("world")),
                    is_absolute: true,
                },
                false
            )
        );
        assert_eq!(
            parse_uri_path("/hello world/blah/")
                .unwrap()
                .reencode_properly()
                .unwrap(),
            (
                FragmentlessUriPath {
                    dirs: vec![prop("hello%20world"), prop("blah")],
                    file: None,
                    is_absolute: true,
                },
                true
            )
        );
        assert_eq!(
            parse_uri_path("/hello world")
                .unwrap()
                .reencode_properly()
                .unwrap(),
            (
                FragmentlessUriPath {
                    dirs: vec![],
                    file: Some(prop("hello%20world")),
                    is_absolute: true,
                },
                true
            )
        );
        parse_uri_path("/he%bb/")
            .unwrap()
            .reencode_properly()
            .unwrap_err();
        assert_eq!(
            parse_uri_path("/hello//world/")
                .unwrap()
                .reencode_properly()
                .unwrap(),
            (
                FragmentlessUriPath {
                    dirs: vec![prop("hello"), prop("world")],
                    file: None,
                    is_absolute: true,
                },
                true
            )
        );
        assert_eq!(
            parse_uri_path("he llo")
                .unwrap()
                .reencode_properly()
                .unwrap(),
            (
                FragmentlessUriPath {
                    dirs: vec![],
                    file: Some(prop("he%20llo")),
                    is_absolute: false,
                },
                true
            )
        );
    }

    #[test]
    fn test_stringify_uri_path() {
        assert_eq!(CanonicalUriPath { dirs: vec![] }.stringify().0, "/");
        assert_eq!(
            ServerUriPath {
                dirs: vec![prop("blah")],
                file: None,
            }
            .stringify()
            .0,
            "/blah/"
        );

        assert_eq!(
            FullUriPath {
                is_absolute: false,
                dirs: vec![],
                file: None,
                fragment: None,
            }
            .stringify()
            .0,
            ""
        );
        assert_eq!(
            FullUriPath {
                is_absolute: false,
                dirs: vec![prop("hi"), prop("there")],
                file: None,
                fragment: None,
            }
            .stringify()
            .0,
            "hi/there/"
        );

        assert_eq!(
            FullUriPath {
                is_absolute: true,
                dirs: vec![],
                file: Some(prop("hi%20world")),
                fragment: Some(UriFragment("kek".to_string())),
            }
            .stringify()
            .0,
            "/hi%20world#kek"
        );
        assert_eq!(
            FullUriPath {
                is_absolute: false,
                dirs: vec![prop("hi%20world")],
                file: None,
                fragment: Some(UriFragment("kek".to_string())),
            }
            .stringify()
            .0,
            "hi%20world/#kek"
        );
    }
}
