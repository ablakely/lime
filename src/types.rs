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
pub struct RootResponse {
    pub makes: Vec<String>,
    pub breadcrumbs: Vec<ApiBreadcrumb>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeResponse {
    pub make: String,
    pub years: Vec<String>,
    pub breadcrumbs: Vec<ApiBreadcrumb>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeYearModelResponse {
    pub model: String,
    pub engines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeYearDatabaseResponse {
    pub name: String,
    pub info_html: String,
    pub models: Vec<MakeYearModelResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MakeYearResponse {
    pub make: String,
    pub year: String,
    pub databases: Vec<MakeYearDatabaseResponse>,
    pub breadcrumbs: Vec<ApiBreadcrumb>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManualPageResponse {
    pub title: String,
    pub content: String,
    pub breadcrumbs: Vec<ApiBreadcrumb>,
}
