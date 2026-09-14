use serde::{Deserialize, Serialize};

use crate::common::{deserialize_car_uri_components, deserialize_years};
use crate::uri_path::CarUriComponents;

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct Make(pub String);
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct Year(pub String);
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct Model(String);
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct Engine(String);
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct DatabaseMachineName(pub String);

impl Make {
    pub fn new(make: String) -> Self {
        assert!(!make.is_empty());
        Self(make)
    }
}

impl Year {
    pub fn new(year: String) -> Self {
        assert!(!year.is_empty());
        Self(year)
    }
}

impl Model {
    #[allow(dead_code)]
    pub fn new(model: String) -> Self {
        assert!(!model.is_empty());
        Self(model)
    }
}

impl Engine {
    #[allow(dead_code)]
    pub fn new(engine: String) -> Self {
        assert!(!engine.is_empty());
        Self(engine)
    }
}

impl AsRef<str> for Make {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Year {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Model {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Engine {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VehicleJsonCommon {
    pub make: Make,
    #[serde(deserialize_with = "deserialize_years")]
    pub years: Vec<Year>,
    pub model: Model,
    pub engine: Option<Engine>,
    #[serde(alias = "uriPath", deserialize_with = "deserialize_car_uri_components")]
    pub uri_path: CarUriComponents,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VehicleJson<VM> {
    #[serde(flatten)]
    pub common: VehicleJsonCommon,
    #[serde(flatten)]
    pub db_specific_metadata: VM,
}

#[derive(Debug, Deserialize)]
pub struct IndexJsonCommonMeta {
    pub database: String,
}

#[derive(Debug, Deserialize)]
pub struct IndexJsonMeta<M> {
    #[allow(dead_code)]
    #[serde(flatten)]
    pub common: IndexJsonCommonMeta,
    #[serde(flatten)]
    pub db_specific: M,
}

#[derive(Debug, Deserialize)]
pub struct IndexJsonCommon {
    #[serde(flatten)]
    pub meta: IndexJsonCommonMeta,
    pub vehicles: Vec<VehicleJsonCommon>,
}

#[derive(Debug, Deserialize)]
pub struct IndexJson<M, VM> {
    #[serde(flatten)]
    pub meta: IndexJsonMeta<M>,
    pub vehicles: Vec<VehicleJson<VM>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum DatabaseFileType {
    #[serde(rename = "lmdb")]
    Lmdb,
    #[serde(rename = "mtbl")]
    Mtbl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiBreadcrumb {
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamedUri {
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YearUri {
    pub year: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineUri {
    pub name: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeYearModelResponse {
    pub model: String,
    pub uri: Option<String>,
    pub engines: Vec<EngineUri>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RootResponse {
    pub makes: Vec<NamedUri>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeResponse {
    pub years: Vec<YearUri>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeYearResponse {
    pub models: Vec<MakeYearModelResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManualPageResponse {
    pub title: String,
    pub breadcrumbs: Vec<ApiBreadcrumb>,
    pub topics: Vec<String>,
    pub manuals: Vec<NamedUri>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn manual_page_json_omits_content_field() {
        let response = ManualPageResponse {
            title: "2012 Buick LaCrosse - Repair and Diagnosis".to_string(),
            breadcrumbs: vec![],
            topics: vec![],
            manuals: vec![],
        };
        let value = serde_json::to_value(response).expect("manual response serializes");
        let object = value.as_object().expect("manual response is object");
        assert!(!object.contains_key("content"));
    }
}
