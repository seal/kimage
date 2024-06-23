//! An Actix-based server for handling image uploads and serving uploaded images.
//!
//! This server provides endpoints for uploading images (converting from base64)
//! and serving previously uploaded images. It uses `pretty_env_logger` for logging.

use actix_multipart::Multipart;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use dirs::home_dir;
use futures::{StreamExt, TryStreamExt};
use log::{error, info};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Server configuration
#[derive(Deserialize, Clone)]
struct Config {
    /// Port number for the server to listen on
    port: u16,
    /// API key for authenticating upload requests
    api_key: String,
    /// Path to store uploaded images
    storage_path: PathBuf,
    /// URL of server
    server_url: String,
}

/// Response structure for successful uploads
#[derive(Serialize)]
struct UploadResponse {
    /// URL of the uploaded image
    url: String,
}

/// Handle image upload requests
async fn upload(req: HttpRequest, mut payload: Multipart) -> Result<HttpResponse, Error> {
    let config = load_config().map_err(|e| {
        error!("Failed to load config: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to load config")
    })?;

    // Check authorization
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            error!("Missing Authorization header");
            actix_web::error::ErrorUnauthorized("Missing Authorization header")
        })?;

    if auth_header != config.api_key {
        info!("Unauthorized access attempt");
        return Ok(HttpResponse::Unauthorized().finish());
    }

    // Process the multipart form data
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition();
        if let Some(name) = content_type.get_name() {
            if name == "image" {
                // Collect all chunks of the file
                let mut bytes = Vec::new();
                while let Some(chunk) = field.next().await {
                    let data = chunk.map_err(|e| {
                        error!("Failed to read multipart data: {}", e);
                        actix_web::error::ErrorInternalServerError("Failed to read multipart data")
                    })?;
                    bytes.extend_from_slice(&data);
                }

                // Decode the base64 image data
                let decoded = general_purpose::STANDARD.decode(&bytes).map_err(|e| {
                    error!("Invalid base64 data: {}", e);
                    actix_web::error::ErrorBadRequest("Invalid base64 data")
                })?;

                // Generate a unique filename and save the image
                let filename = generate_filename();
                let file_path = config.storage_path.join(&filename);
                info!("Saving file to: {:?}", file_path);
                fs::write(&file_path, &decoded).map_err(|e| {
                    error!("Failed to write file: {}", e);
                    actix_web::error::ErrorInternalServerError("Failed to write file")
                })?;

                // Construct and return the URL of the uploaded image
                let url = format!("{}/{}", config.server_url, filename);
                info!("File uploaded successfully: {}", url);
                return Ok(HttpResponse::Ok().json(UploadResponse { url }));
            }
        }
    }

    error!("Bad request: No image field found in payload");
    Ok(HttpResponse::BadRequest().finish())
}

/// Serve previously uploaded images
async fn serve_image(filename: web::Path<String>) -> Result<HttpResponse, Error> {
    let config = load_config().map_err(|e| {
        error!("Failed to load config: {}", e);
        actix_web::error::ErrorInternalServerError("Failed to load config")
    })?;

    let file_path = config.storage_path.join(filename.as_str());
    if file_path.exists() {
        let contents = fs::read(&file_path).map_err(|e| {
            error!("Failed to read file {:?}: {}", file_path, e);
            actix_web::error::ErrorInternalServerError("Failed to read file")
        })?;
        info!("Serving image: {:?}", file_path);
        Ok(HttpResponse::Ok().content_type("image/png").body(contents))
    } else {
        info!("Image not found: {:?}", file_path);
        Ok(HttpResponse::NotFound().finish())
    }
}

#[actix_web::main]
async fn main() -> Result<()> {
    // Initialize the logger
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();

    // Load the server configuration
    let config = load_config()?;
    let port = config.port;

    info!("Server running on http://localhost:{}", port);

    // Start the HTTP server
    HttpServer::new(move || {
        App::new()
            .route("/upload", web::post().to(upload))
            .route("/{filename}", web::get().to(serve_image))
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
    .context("Error running server")
}

/// Load the server configuration from a TOML file
fn load_config() -> Result<Config> {
    let config_path = home_dir()
        .context("Failed to get home directory")?
        .join(".config")
        .join("kimage.toml");

    info!("Loading config from: {:?}", config_path);
    let config_str = fs::read_to_string(&config_path).context("Failed to read config file")?;

    let mut config: Config = toml::from_str(&config_str).context("Failed to parse config file")?;

    // Convert relative storage path to absolute
    if config.storage_path.is_relative() {
        config.storage_path = home_dir()
            .context("Failed to get home directory")?
            .join(&config.storage_path);
    }

    info!("Config loaded successfully");
    Ok(config)
}

/// Generate a random filename for uploaded images
fn generate_filename() -> String {
    let mut rng = rand::thread_rng();
    let random_string: String = (0..10)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    format!("{}.png", random_string)
}
