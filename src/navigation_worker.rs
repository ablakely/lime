use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    http::header::CONTENT_TYPE,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{
    indexing::Indices,
    types::{Make, Year},
};

static WORKER_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub enum NavigationRequest {
    Root,
    Make(Make),
    MakeYear(Make, Year),
}

#[derive(Clone, Debug)]
pub struct NavigationWorker {
    temp_dir: PathBuf,
}

impl Default for NavigationWorker {
    fn default() -> Self {
        Self {
            temp_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("worker_temp"),
        }
    }
}

impl NavigationWorker {
    #[cfg(test)]
    fn new(temp_dir: PathBuf) -> Self {
        Self { temp_dir }
    }

    pub fn handle_request(
        &self,
        indices: &Indices,
        request: NavigationRequest,
    ) -> Result<Option<Response>> {
        match request {
            NavigationRequest::Root => self.serialize_response(&indices.root_json()).map(Some),
            NavigationRequest::Make(make) => indices
                .make_json(&make)
                .map(|response| self.serialize_response(&response))
                .transpose(),
            NavigationRequest::MakeYear(make, year) => indices
                .make_year_json(&make, &year)
                .map(|response| self.serialize_response(&response))
                .transpose(),
        }
    }

    fn serialize_response<T: Serialize>(&self, response: &T) -> Result<Response> {
        std::fs::create_dir_all(&self.temp_dir).with_context(|| {
            format!(
                "Failed to create worker temp directory {}",
                self.temp_dir.display()
            )
        })?;
        let temp_file = WorkerTempFile::new(&self.temp_dir);
        let json_bytes = serde_json::to_vec(response).context("Failed to serialize worker JSON")?;
        std::fs::write(temp_file.path(), &json_bytes).with_context(|| {
            format!("Failed to write worker temp file {}", temp_file.path().display())
        })?;
        let response_bytes = std::fs::read(temp_file.path()).with_context(|| {
            format!("Failed to read worker temp file {}", temp_file.path().display())
        })?;
        Ok(([(CONTENT_TYPE, "application/json")], response_bytes).into_response())
    }
}

struct WorkerTempFile {
    path: PathBuf,
}

impl WorkerTempFile {
    fn new(temp_dir: &Path) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock drifted before unix epoch")
            .as_nanos();
        let counter = WORKER_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            path: temp_dir.join(format!(
                "navigation-worker-{}-{nanos}-{counter}.tmp",
                std::process::id()
            )),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for WorkerTempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use super::*;
    use crate::{
        common::SenderWriter,
        database_engines::{DatabaseEngine, ResponseFormat},
        types::{DatabaseMachineName, Engine, Model},
        uri_path::{CanonicalUriPath, CarUriComponents, UriComponent},
    };

    struct DummyDb;

    impl DatabaseEngine for DummyDb {
        fn machine_readable_name(&self) -> DatabaseMachineName {
            DatabaseMachineName("lemon".to_string())
        }

        fn human_readable_name(&self) -> String {
            "LEMON".to_string()
        }

        fn priority_and_info(&self, _make: &Make, _year: &Year) -> (i32, String) {
            (0, String::new())
        }

        fn handle_car_request(
            &self,
            _uri_path: CanonicalUriPath,
            _response_format: ResponseFormat,
        ) -> Result<Option<Response>> {
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

        fn handle_global_request(&self, _uri_path: &CanonicalUriPath) -> Result<Option<Response>> {
            Ok(None)
        }
    }

    fn sample_indices() -> Indices {
        let mut indices = Indices::new();
        let db = Arc::new(DummyDb);
        indices.add_database(db.clone()).unwrap();
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
            db,
        );
        indices
    }

    #[test]
    fn worker_uses_temp_files_and_cleans_them_up() {
        let test_temp_dir = std::env::temp_dir().join(format!(
            "lime-navigation-worker-test-{}",
            WORKER_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let worker = NavigationWorker::new(test_temp_dir.clone());
        let response = worker
            .handle_request(&sample_indices(), NavigationRequest::Root)
            .expect("worker should succeed")
            .expect("root response should exist");

        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/json"
        );
        assert!(
            !test_temp_dir.exists()
                || test_temp_dir
                    .read_dir()
                    .expect("temp dir should be readable")
                    .next()
                    .is_none()
        );
    }
}
