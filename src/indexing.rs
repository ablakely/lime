

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
    types::{DatabaseMachineName, Engine, Make, Model, Year},
    uri_path::{
        CarUriComponents, FullUriPath, UriComponentDecoded, car_uri_components_to_uri_path,
    },
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
}

fn model_engine_human_readable<'a>(model: &'a Model, engine: Option<&Engine>) -> Cow<'a, str> {
    match engine {
        Some(eng) => Cow::Owned(format!("{} {}", model.as_ref(), eng.as_ref())),
        None => Cow::Borrowed(model.as_ref()),
    }
}
