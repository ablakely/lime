use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::{Result, bail};

use crate::{
    database_engines::DatabaseEngine,
    types::{
        DatabaseMachineName, Engine, EngineUri, Make, MakeResponse, MakeYearModelResponse,
        MakeYearResponse, Model, NamedUri, RootResponse, Year, YearUri,
    },
    uri_path::{
        CanonicalUriPath, CarUriComponents, UriComponent, UriPath, car_uri_components_to_uri_path,
    },
};

type HierarchicalIndex = BTreeMap<Make, BTreeMap<Year, BTreeMap<DatabaseMachineName, ModelsMap>>>;

type ModelsMap = BTreeMap<Model, BTreeMap<Option<Engine>, CarUriComponents>>;

type FlatIndex = HashMap<CarUriComponents, Arc<dyn DatabaseEngine>>;

pub struct Indices {
    hierarchical: HierarchicalIndex,
    flat: FlatIndex,
    engines: HashMap<DatabaseMachineName, Arc<dyn DatabaseEngine>>,
}

impl Indices {
    pub fn new() -> Self {
        Self {
            hierarchical: BTreeMap::new(),
            flat: HashMap::new(),
            engines: HashMap::new(),
        }
    }

    pub fn database_engine_for_car(
        &self,
        car_uri_components: &CarUriComponents,
    ) -> Option<Arc<dyn DatabaseEngine>> {
        self.flat.get(car_uri_components).cloned()
    }

    pub fn add_database(&mut self, db_engine: Arc<dyn DatabaseEngine>) -> Result<()> {
        let machine_name = db_engine.machine_readable_name();
        if self.engines.contains_key(&machine_name) {
            bail!(
                "Can't add the same database type twice: {}",
                db_engine.human_readable_name()
            );
        }
        self.engines.insert(machine_name, db_engine);
        Ok(())
    }

    pub fn databases(&self) -> impl Iterator<Item = &Arc<dyn DatabaseEngine>> {
        self.engines.values()
    }

    pub fn add_vehicle(
        &mut self,
        make: Make,
        years: &[Year],
        model: Model,
        engine: Option<Engine>,
        uri_components: CarUriComponents,
        db_engine: Arc<dyn DatabaseEngine>,
    ) {
        assert!(
            self.engines
                .contains_key(&db_engine.machine_readable_name()),
            "Cannot add vehicle before adding the database"
        );
        for year in years {
            if self
                .hierarchical
                .entry(make.clone())
                .or_default()
                .entry(year.clone())
                .or_default()
                .entry(db_engine.machine_readable_name())
                .or_default()
                .entry(model.clone())
                .or_default()
                .insert(engine.clone(), uri_components.clone())
                .is_some()
            {
                panic!("duplicate hierarchical vehicle index");
            }

            if self
                .flat
                .insert(uri_components.clone(), db_engine.clone())
                .is_some()
            {
                panic!("duplicate flat vehicles index");
            }
        }
    }

    pub fn root_json(&self) -> RootResponse {
        RootResponse {
            makes: self
                .hierarchical
                .keys()
                .map(|make| NamedUri {
                    name: make.as_ref().to_string(),
                    uri: uri_for_components(&[make.as_ref()]),
                })
                .collect(),
        }
    }

    pub fn make_json(&self, make: &Make) -> Option<MakeResponse> {
        let years_map = self.hierarchical.get(make)?;
        Some(MakeResponse {
            years: years_map
                .keys()
                .map(|year| YearUri {
                    year: year.as_ref().to_string(),
                    uri: uri_for_components(&[make.as_ref(), year.as_ref()]),
                })
                .collect(),
        })
    }

    pub fn make_year_json(&self, make: &Make, year: &Year) -> Option<MakeYearResponse> {
        let dbs_map = self.hierarchical.get(make).and_then(|m| m.get(year))?;
        let mut dbs_and_models: Vec<(&Arc<dyn DatabaseEngine>, &ModelsMap)> = dbs_map
            .iter()
            .map(|(db_name, models_map)| (&self.engines[db_name], models_map))
            .collect();
        dbs_and_models.sort_by(|a, b| {
            b.0.priority_and_info(make, year)
                .0
                .cmp(&a.0.priority_and_info(make, year).0)
        });

        let mut models = BTreeMap::<String, Vec<EngineUri>>::new();
        for (_, models_map) in dbs_and_models {
            for (model, engines_map) in models_map {
                models
                    .entry(model.as_ref().to_string())
                    .or_default()
                    .extend(engines_map.iter().map(|(engine, car_uri_components)| EngineUri {
                        name: engine
                            .as_ref()
                            .map(|engine| engine.as_ref().to_string())
                            .unwrap_or_else(|| model.as_ref().to_string()),
                        uri: String::from(
                            car_uri_components_to_uri_path(car_uri_components).stringify(),
                        ),
                    }));
            }
        }

        Some(MakeYearResponse {
            models: models
                .into_iter()
                .map(|(model, engines)| MakeYearModelResponse { model, engines })
                .collect(),
        })
    }
}

fn uri_for_components(decoded_components: &[&str]) -> String {
    let dirs = decoded_components
        .iter()
        .map(|component| {
            UriComponent::from_decoded_str(component)
                .expect("navigation uri component cannot be empty")
        })
        .collect();
    String::from(CanonicalUriPath { dirs }.stringify())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        common::SenderWriter, database_engines::ResponseFormat, types::DatabaseMachineName,
        uri_path::CanonicalUriPath,
    };

    struct DummyDb {
        machine_name: &'static str,
        human_name: &'static str,
        priority: i32,
    }

    impl DatabaseEngine for DummyDb {
        fn machine_readable_name(&self) -> DatabaseMachineName {
            DatabaseMachineName(self.machine_name.to_string())
        }

        fn human_readable_name(&self) -> String {
            self.human_name.to_string()
        }

        fn priority_and_info(&self, _make: &Make, _year: &Year) -> (i32, String) {
            (self.priority, format!("{} info", self.human_name))
        }

        fn handle_car_request(
            &self,
            _uri_path: CanonicalUriPath,
            _response_format: ResponseFormat,
        ) -> Result<Option<axum::response::Response>> {
            Ok(None)
        }

        fn handle_bundle_request(
            &self,
            _car_uri_components: &CarUriComponents,
            _writer: SenderWriter,
        ) -> Result<()> {
            Ok(())
        }

        fn global_request_predicate(&self, _uri_path: &CanonicalUriPath) -> bool {
            false
        }

        fn handle_global_request(
            &self,
            _uri_path: &CanonicalUriPath,
        ) -> Result<Option<axum::response::Response>> {
            Ok(None)
        }
    }

    fn sample_indices() -> Indices {
        let mut indices = Indices::new();
        let lemon = Arc::new(DummyDb {
            machine_name: "lemon",
            human_name: "LEMON",
            priority: 1,
        });
        let charm = Arc::new(DummyDb {
            machine_name: "charm",
            human_name: "CHARM",
            priority: 0,
        });
        let charm2 = Arc::new(DummyDb {
            machine_name: "charm2",
            human_name: "CHARM 2",
            priority: -1,
        });
        indices.add_database(lemon.clone()).unwrap();
        indices.add_database(charm.clone()).unwrap();
        indices.add_database(charm2.clone()).unwrap();
        indices.add_vehicle(
            Make::new("Toyota".to_string()),
            &[Year::new("2022".to_string())],
            Model::new("Camry".to_string()),
            Some(Engine::new("2.5L".to_string())),
            [
                UriComponent::from_decoded_str("Toyota").unwrap(),
                UriComponent::from_decoded_str("2022").unwrap(),
                UriComponent::from_decoded_str("Camry").unwrap(),
            ],
            lemon.clone(),
        );
        indices.add_vehicle(
            Make::new("Toyota".to_string()),
            &[Year::new("2022".to_string())],
            Model::new("Camry".to_string()),
            Some(Engine::new("Hybrid".to_string())),
            [
                UriComponent::from_decoded_str("Toyota").unwrap(),
                UriComponent::from_decoded_str("2022").unwrap(),
                UriComponent::from_decoded_str("Camry Hybrid").unwrap(),
            ],
            lemon,
        );
        indices.add_vehicle(
            Make::new("Toyota".to_string()),
            &[Year::new("2022".to_string())],
            Model::new("Corolla".to_string()),
            None,
            [
                UriComponent::from_decoded_str("Toyota").unwrap(),
                UriComponent::from_decoded_str("2022").unwrap(),
                UriComponent::from_decoded_str("Corolla").unwrap(),
            ],
            charm,
        );
        indices.add_vehicle(
            Make::new("Toyota".to_string()),
            &[Year::new("2022".to_string())],
            Model::new("Camry".to_string()),
            Some(Engine::new("3.0L".to_string())),
            [
                UriComponent::from_decoded_str("Toyota").unwrap(),
                UriComponent::from_decoded_str("2022").unwrap(),
                UriComponent::from_decoded_str("Camry 3.0L").unwrap(),
            ],
            charm2,
        );
        indices
    }

    #[test]
    fn root_and_make_json_include_navigation_links() {
        let indices = sample_indices();
        assert_eq!(
            indices.root_json().makes,
            vec![NamedUri {
                name: "Toyota".to_string(),
                uri: "/Toyota/".to_string()
            }]
        );

        let make_json = indices
            .make_json(&Make::new("Toyota".to_string()))
            .expect("expected make json");
        assert_eq!(
            make_json.years,
            vec![YearUri {
                year: "2022".to_string(),
                uri: "/Toyota/2022/".to_string(),
            }]
        );
    }

    #[test]
    fn make_year_json_groups_models_and_engines() {
        let indices = sample_indices();
        let response = indices
            .make_year_json(
                &Make::new("Toyota".to_string()),
                &Year::new("2022".to_string()),
            )
            .expect("expected make/year json");
        assert_eq!(response.models.len(), 2);
        assert_eq!(response.models[0].model, "Camry");
        assert_eq!(
            response.models[0].engines,
            vec![
                EngineUri {
                    name: "2.5L".to_string(),
                    uri: "/Toyota/2022/Camry/".to_string(),
                },
                EngineUri {
                    name: "Hybrid".to_string(),
                    uri: "/Toyota/2022/Camry%20Hybrid/".to_string(),
                },
                EngineUri {
                    name: "3.0L".to_string(),
                    uri: "/Toyota/2022/Camry%203.0L/".to_string(),
                },
            ]
        );
        assert_eq!(response.models[1].model, "Corolla");
        assert_eq!(
            vec![
                EngineUri {
                    name: "Corolla".to_string(),
                    uri: "/Toyota/2022/Corolla/".to_string(),
                },
            ],
            response.models[1].engines
        );
    }
}
