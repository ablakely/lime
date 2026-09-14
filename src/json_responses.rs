use serde::Serialize;

/// Standard API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            error: None,
        }
    }
}

/// Root endpoint response - list all car makes
#[derive(Serialize)]
pub struct RootResponse {
    pub makes: Vec<String>,
}

/// Make endpoint response - list all years for a make
#[derive(Serialize)]
pub struct MakeResponse {
    pub make: String,
    pub years: Vec<String>,
}

/// Make/Year endpoint response - list all models and variants for a year
#[derive(Serialize)]
pub struct MakeYearResponse {
    pub make: String,
    pub year: String,
    pub databases: Vec<DatabaseModels>,
}

/// Database-specific models for a make/year
#[derive(Serialize)]
pub struct DatabaseModels {
    pub database_name: String,
    pub database_machine_name: String,
    pub priority: i32,
    pub info: String,
    pub models: Vec<ModelVariant>,
}

/// A single model with its engine variants
#[derive(Serialize)]
pub struct ModelVariant {
    pub model: String,
    pub display_name: String,
    pub engines: Vec<EngineVariant>,
}

/// Engine variant with direct link to manual
#[derive(Serialize)]
pub struct EngineVariant {
    pub engine: Option<String>,
    pub display_name: String,
    pub path: String,
}

/// Error response
#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl ErrorResponse {
    pub fn not_found() -> Self {
        ErrorResponse {
            success: false,
            error: "Not found".to_string(),
        }
    }

    pub fn bad_request(msg: &str) -> Self {
        ErrorResponse {
            success: false,
            error: msg.to_string(),
        }
    }

    pub fn internal_error() -> Self {
        ErrorResponse {
            success: false,
            error: "Internal server error".to_string(),
        }
    }
}

/// Bundle download info response
#[derive(Serialize)]
pub struct BundleInfoResponse {
    pub car_name: String,
    pub make: String,
    pub year: String,
    pub model: String,
    pub message: String,
}
