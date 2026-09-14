use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::Result;

use crate::{
    database_engines::DatabaseEngine,
    json_responses::{DatabaseModels, EngineVariant, MakeYearResponse, ModelVariant},
    types::{DatabaseMachineName, Engine, Make, Model, Year},
    uri_path::CarUriComponents,
};

type HierarchicalIndex = BTreeMap<
    Make,
    BTreeMap<
        Year,
        BTreeMap<DatabaseMachineName, ModelsMap>,
    >,
>;

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
            anyhow::bail!(
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

    /// Get all car makes
    pub fn get_all_makes(&self) -> Vec<String> {
        self.hierarchical
            .keys()
            .map(|make| make.as_ref().to_string())
            .collect()
    }

    /// Get all years for a specific make
    pub fn get_years_for_make(&self, make: &Make) -> Option<Vec<String>> {
        self.hierarchical.get(make).map(|years_map| {
            years_map
                .keys()
                .map(|year| year.as_ref().to_string())
                .collect()
        })
    }

    /// Get all models and variants for a make/year combination
    pub fn get_models_for_make_year(&self, make: &Make, year: &Year) -> Option<MakeYearResponse> {
        let dbs_map = self.hierarchical.get(make).and_then(|m| m.get(year))?;
        let mut dbs_and_models: Vec<(&Arc<dyn DatabaseEngine>, &ModelsMap)> = dbs_map
            .iter()
            .map(|(db_name, models_map)| (&self.engines[db_name], models_map))
            .collect();
        
        // Sort by priority (highest first)
        dbs_and_models.sort_by(|a, b| -> std::cmp::Ordering {
            b.0.priority_and_info(make, year)
                .0
                .cmp(&a.0.priority_and_info(make, year).0)
        });

        let databases = dbs_and_models
            .into_iter()
            .map(|(db, models_map)| {
                let (priority, info) = db.priority_and_info(make, year);
                
                let models: Vec<ModelVariant> = models_map
                    .iter()
                    .map(|(model, engines_map)| {
                        let has_multiple_engines = engines_map.len() > 1;
                        let engines: Vec<EngineVariant> = engines_map
                            .iter()
                            .map(|(engine, car_uri_components)| {
                                let engine_str = engine.as_ref().map(|e| e.as_ref().to_string());
                                let display_name = match engine {
                                    Some(eng) => format!("{} {}", model.as_ref(), eng.as_ref()),
                                    None => model.as_ref().to_string(),
                                };
                                let path = format!(
                                    "/{}/{}/{}",
                                    car_uri_components[0].decode_uri_component().0,
                                    car_uri_components[1].decode_uri_component().0,
                                    car_uri_components[2].decode_uri_component().0,
                                );
                                EngineVariant {
                                    engine: engine_str,
                                    display_name,
                                    path,
                                }
                            })
                            .collect();

                        ModelVariant {
                            model: model.as_ref().to_string(),
                            display_name: if has_multiple_engines {
                                model.as_ref().to_string()
                            } else {
                                engines.first().map(|e| e.display_name.clone()).unwrap_or_default()
                            },
                            engines,
                        }
                    })
                    .collect();

                DatabaseModels {
                    database_name: db.human_readable_name(),
                    database_machine_name: db.machine_readable_name().0.clone(),
                    priority,
                    info,
                    models,
                }
            })
            .collect();

        Some(MakeYearResponse {
            make: make.as_ref().to_string(),
            year: year.as_ref().to_string(),
            databases,
        })
    }

    /// Legacy HTML methods (kept for backward compatibility if needed)
    pub fn root_html(&self) -> String {
        "Use JSON API instead".to_string()
    }

    pub fn make_html(&self, _make: &Make) -> Option<String> {
        None
    }

    pub fn make_year_html(&self, _make: &Make, _year: &Year) -> Option<String> {
        None
    }
}
