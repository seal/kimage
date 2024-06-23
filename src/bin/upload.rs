//! A command-line tool for uploading images to a server and copying the resulting URL to the clipboard.
//!
//! This tool reads an image file, converts it to base64, sends it to a configured server,
//! and copies the returned URL to the clipboard. It uses `pretty_env_logger` for logging.
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use clipboard::{ClipboardContext, ClipboardProvider};
use dirs::home_dir;
use image::ImageOutputFormat;
use log::{error, info};
use serde::Deserialize;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

/// Command-line arguments for the image uploader
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the image file to upload
    #[arg(help = "Path to the image file to upload")]
    image_path: PathBuf,
}

/// Configuration for the image uploader
#[derive(Deserialize)]
struct Config {
    /// URL of the server to upload images to
    server_url: String,
    /// API key for authentication with the server
    api_key: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "info");
    // Initialize the logger
    pretty_env_logger::init();

    // Parse command-line arguments
    let args = Args::parse();
    // Load configuration
    let config = load_config()?;

    // Read the image file
    info!("Loading image from path: {:?}", args.image_path);
    let image_data = fs::read(&args.image_path).context("Failed to read image file")?;

    // Load the image into memory
    let img = image::load_from_memory(&image_data).context("Failed to load image")?;

    // Convert the image to PNG format
    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, ImageOutputFormat::Png)
        .context("Failed to encode image as PNG")?;

    // Convert the PNG data to base64
    let base64_image = general_purpose::STANDARD.encode(buffer.into_inner());

    // Send the image to the server
    info!("Sending image to server");
    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/upload", config.server_url))
        .header("Authorization", &config.api_key)
        .multipart(reqwest::multipart::Form::new().text("image", base64_image))
        .send()
        .await
        .context("Failed to send request")?;

    // Check if the upload was successful
    if !response.status().is_success() {
        error!("Server returned error: {}", response.status());
        return Err(anyhow!("Server returned error: {}", response.status()));
    }

    // Parse the response to get the URL of the uploaded image
    let upload_response: serde_json::Value =
        response.json().await.context("Failed to parse response")?;
    let url = upload_response["url"]
        .as_str()
        .ok_or_else(|| anyhow!("Invalid response format"))?
        .to_string();

    info!("Image uploaded successfully. URL: {}", url);

    // Copy the URL to the clipboard
    let mut ctx: ClipboardContext = ClipboardProvider::new()
        .map_err(|e| anyhow!("Failed to initialize clipboard context: {}", e))?;
    ctx.set_contents(url.clone())
        .map_err(|e| anyhow!("Failed to copy URL to clipboard: {}", e))?;

    info!("URL copied to clipboard.");

    Ok(())
}

/// Load the configuration from a TOML file in the user's home directory
fn load_config() -> Result<Config> {
    let config_path = home_dir()
        .context("Failed to get home directory")?
        .join(".config")
        .join("kimage.toml");

    info!("Loading config from: {:?}", config_path);
    let config_str = fs::read_to_string(config_path).context("Failed to read config file")?;

    let config: Config = toml::from_str(&config_str).context("Failed to parse config file")?;

    Ok(config)
}
