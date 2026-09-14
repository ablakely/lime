use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::{Result, bail};
use plait::html;

use crate::{
    common::safe_a,
    database_engines::DatabaseEngine,
    types::{
        ApiBreadcrumb, DatabaseMachineName, Engine, Make, MakeResponse, MakeYearDatabaseResponse,
        MakeYearModelResponse, MakeYearResponse, Model, RootResponse, Year,
    },
    uri_path::{
        CanonicalUriPath, CarUriComponents, FullUriPath, UriComponent, UriComponentDecoded,
        UriPath, car_uri_components_to_uri_path,
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

    pub fn root_html(&self) -> String {
        html! {
            ul {
                for make in self.hierarchical.keys() {
                    let make_uri_component = UriComponentDecoded(Cow::Borrowed(make.as_ref()))

                        .encode_uri_component().unwrap();
                    li {
                        @(&safe_a(None, &FullUriPath {
                            dirs: vec![make_uri_component],
                            file: None,
                            fragment: None,
                            is_absolute: false
                        }, &html! { (make.as_ref()) }))
                    }
                }
            }
        }
        .to_string()
    }

    pub fn root_json(&self) -> RootResponse {
        RootResponse {
            makes: self
                .hierarchical
                .keys()
                .map(|make| make.as_ref().to_string())
                .collect(),
            breadcrumbs: Vec::new(),
        }
    }

    pub fn make_html(&self, make: &Make) -> Option<String> {
        let years_map = self.hierarchical.get(make)?;
        Some(
            html! {
                ul {
                    for year in years_map.keys() {
                        let year_uri_component = UriComponentDecoded(Cow::Borrowed(year.as_ref()))
                            .encode_uri_component().unwrap();
                        li {
                            @(&safe_a(None, &FullUriPath {
                                dirs: vec![year_uri_component],
                                file: None,
                                fragment: None,
                                is_absolute: false,
                            }, &html! { (year.as_ref()) }))
                        }
                    }
                }
            }
            .to_string(),
        )
    }

    pub fn make_json(&self, make: &Make) -> Option<MakeResponse> {
        let years_map = self.hierarchical.get(make)?;
        Some(MakeResponse {
            make: make.as_ref().to_string(),
            years: years_map
                .keys()
                .map(|year| year.as_ref().to_string())
                .collect(),
            breadcrumbs: json_breadcrumbs(&[make.as_ref()]),
        })
    }

    pub fn make_year_html(&self, make: &Make, year: &Year) -> Option<String> {
        let dbs_map = self.hierarchical.get(make).and_then(|m| m.get(year))?;
        let mut dbs_and_models: Vec<(&Arc<dyn DatabaseEngine>, &ModelsMap)> = dbs_map
            .iter()
            .map(|(db_name, models_map)| (&self.engines[db_name], models_map))
            .collect();
        dbs_and_models.sort_by(|a, b| -> std::cmp::Ordering {
            b.0.priority_and_info(make, year)
                .0
                .cmp(&a.0.priority_and_info(make, year).0)
        });

        Some(html! {
            ul {
                for (db, models_map) in &dbs_and_models {
                    h3 { "Database: " (db.human_readable_name()) }
                    div { #(db.priority_and_info(make, year).1) }
                    br;
                    for (model, engines_map) in *models_map {
                        let has_multiple_engines = engines_map.len() > 1;
                        if has_multiple_engines {
                            li(class: "li-folder") {
                                a { (model.as_ref()) }
                                ul {
                                    for (engine, car_uri_components) in engines_map {
                                        let engine_str: &str = engine.as_ref().expect("When a car has multiple engines, all engines must be nonempty!").as_ref();
                                        li {
                                            @(&safe_a(None, &car_uri_components_to_uri_path(car_uri_components), html! { (&engine_str) }))
                                        }
                                    }
                                }
                            }
                        } else {
                            let (engine, car_uri_components) = engines_map.iter().next().expect("every model should have at least one engine");
                            li {
                                @(&safe_a(None, &car_uri_components_to_uri_path(car_uri_components), html! { (model_engine_human_readable(model, engine.as_ref())) }))
                            }
                        }
                    }
                }
            }
        }.to_string())
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

        Some(MakeYearResponse {
            make: make.as_ref().to_string(),
            year: year.as_ref().to_string(),
            databases: dbs_and_models
                .into_iter()
                .map(|(db, models_map)| MakeYearDatabaseResponse {
                    name: db.human_readable_name(),
                    info_html: db.priority_and_info(make, year).1,
                    models: models_map
                        .iter()
                        .map(|(model, engines_map)| MakeYearModelResponse {
                            model: model.as_ref().to_string(),
                            engines: engines_map
                                .keys()
                                .filter_map(|engine| {
                                    engine.as_ref().map(|engine| engine.as_ref().to_string())
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
            breadcrumbs: json_breadcrumbs(&[make.as_ref(), year.as_ref()]),
        })
    }
}

fn model_engine_human_readable<'a>(model: &'a Model, engine: Option<&Engine>) -> Cow<'a, str> {
    match engine {
        Some(eng) => Cow::Owned(format!("{} {}", model.as_ref(), eng.as_ref())),
        None => Cow::Borrowed(model.as_ref()),
    }
}

fn json_breadcrumbs(decoded_components: &[&str]) -> Vec<ApiBreadcrumb> {
    let mut dirs = Vec::with_capacity(decoded_components.len());
    let mut breadcrumbs = Vec::with_capacity(decoded_components.len());
    for component in decoded_components {
        let encoded = UriComponent::from_decoded_str(component)
            .expect("json breadcrumb component cannot be empty");
        dirs.push(encoded);
        breadcrumbs.push(ApiBreadcrumb {
            label: (*component).to_string(),
            href: String::from(CanonicalUriPath { dirs: dirs.clone() }.stringify()),
        });
    }
    breadcrumbs
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
        indices.add_database(lemon.clone()).unwrap();
        indices.add_database(charm.clone()).unwrap();
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
        indices
    }

    #[test]
    fn root_and_make_json_include_breadcrumbs() {
        let indices = sample_indices();
        assert_eq!(indices.root_json().makes, vec!["Toyota"]);
        assert_eq!(indices.root_json().breadcrumbs, Vec::<ApiBreadcrumb>::new());

        let make_json = indices
            .make_json(&Make::new("Toyota".to_string()))
            .expect("expected make json");
        assert_eq!(make_json.make, "Toyota");
        assert_eq!(make_json.years, vec!["2022"]);
        assert_eq!(
            make_json.breadcrumbs,
            vec![ApiBreadcrumb {
                label: "Toyota".to_string(),
                href: "/Toyota/".to_string(),
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
        assert_eq!(response.databases.len(), 2);
        assert_eq!(response.databases[0].name, "LEMON");
        assert_eq!(response.databases[0].models[0].model, "Camry");
        assert_eq!(
            response.databases[0].models[0].engines,
            vec!["2.5L", "Hybrid"]
        );
        assert_eq!(response.databases[1].name, "CHARM");
        assert_eq!(response.databases[1].models[0].model, "Corolla");
        assert_eq!(
            response.databases[1].models[0].engines,
            Vec::<String>::new()
        );
        assert_eq!(
            response.breadcrumbs,
            vec![
                ApiBreadcrumb {
                    label: "Toyota".to_string(),
                    href: "/Toyota/".to_string(),
                },
                ApiBreadcrumb {
                    label: "2022".to_string(),
                    href: "/Toyota/2022/".to_string(),
                },
            ]
        );
    }
}
