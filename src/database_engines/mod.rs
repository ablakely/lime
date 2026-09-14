use crate::{
    common::SenderWriter,
    json_responses::ManualPageResponse,
    types::{DatabaseMachineName, Make, Year},
    uri_path::{CanonicalUriPath, CarUriComponents},
};

use anyhow::Result;

pub mod charm;
pub mod lemon;

pub trait DatabaseEngine: Sync + Send {
    fn machine_readable_name(&self) -> DatabaseMachineName;
    fn human_readable_name(&self) -> String;
    /// Stuff to be shown on the appropriate index page. The string is treated as html and is not escaped further.
    fn priority_and_info(&self, make: &Make, year: &Year) -> (i32, String);
    // called inside spawn_blocking
    fn handle_car_request(
        &self,
        uri_path: CanonicalUriPath,
    ) -> Result<Option<axum::response::Response>>;
    // New method: return manual page as JSON
    fn handle_car_request_json(
        &self,
        uri_path: CanonicalUriPath,
    ) -> Result<Option<ManualPageResponse>>;
    // called inside spawn_blocking
    fn handle_bundle_request(
        &self,
        car_uri_components: &CarUriComponents,
        writer: SenderWriter,
    ) -> Result<()>;
    // called outside spawn_blocking, don't want to be doing inter-thread communication for global routes that don't even apply
    fn global_request_predicate(&self, uri_path: &CanonicalUriPath) -> bool;
    // called inside spawn_blocking
    fn handle_global_request(
        &self,
        uri_path: &CanonicalUriPath,
    ) -> Result<Option<axum::response::Response>>;
}
