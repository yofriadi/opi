//! CLI image attachment tests for task 3.4.
//!
//! Validates the --image CLI flag reads files from isolated temp directories
//! and constructs correct InputContent::Image messages. Never reads user
//! runtime data.

use clap::Parser;
use opi_ai::message::{ImageSource, InputContent, MediaType};
use opi_coding_agent::cli::Cli;
use std::path::PathBuf;

// --- CLI flag parsing ---

#[test]
fn cli_accepts_image_flag() {
    let args = Cli::try_parse_from(["opi", "--image", "photo.png", "Describe this"]);
    assert!(args.is_ok(), "failed to parse --image: {:?}", args.err());
    let cli = args.unwrap();
    assert_eq!(cli.image.len(), 1);
    assert_eq!(cli.image[0], PathBuf::from("photo.png"));
}

#[test]
fn cli_accepts_multiple_image_flags() {
    let args = Cli::try_parse_from([
        "opi",
        "--image",
        "a.png",
        "--image",
        "b.jpg",
        "Compare these",
    ]);
    assert!(
        args.is_ok(),
        "failed to parse multiple --image: {:?}",
        args.err()
    );
    let cli = args.unwrap();
    assert_eq!(cli.image.len(), 2);
}

#[test]
fn cli_image_flag_optional() {
    let args = Cli::try_parse_from(["opi", "Hello"]);
    assert!(args.is_ok());
    let cli = args.unwrap();
    assert!(cli.image.is_empty());
}

// --- Image file reading and media type detection ---

#[test]
fn detect_media_type_from_png_extension() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.png")),
        Some(MediaType::Png)
    );
}

#[test]
fn detect_media_type_from_jpg_extension() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.jpg")),
        Some(MediaType::Jpeg)
    );
}

#[test]
fn detect_media_type_from_jpeg_extension() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.jpeg")),
        Some(MediaType::Jpeg)
    );
}

#[test]
fn detect_media_type_from_gif_extension() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.gif")),
        Some(MediaType::Gif)
    );
}

#[test]
fn detect_media_type_from_webp_extension() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.webp")),
        Some(MediaType::WebP)
    );
}

#[test]
fn detect_media_type_unknown_returns_none() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.bmp")),
        None
    );
}

#[test]
fn detect_media_type_case_insensitive() {
    assert_eq!(
        opi_coding_agent::image::detect_media_type(PathBuf::from("photo.PNG")),
        Some(MediaType::Png)
    );
}

// --- Load image from temp file ---

#[test]
fn load_image_from_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.png");
    let data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A];
    std::fs::write(&file_path, &data).unwrap();

    let content = opi_coding_agent::image::load_image(&file_path).unwrap();
    let InputContent::Image { source, media_type } = content else {
        panic!("expected Image variant");
    };
    assert_eq!(media_type, MediaType::Png);
    let ImageSource::Bytes { data: loaded } = source else {
        panic!("expected Bytes source");
    };
    assert_eq!(loaded, data);
}

#[test]
fn load_image_nonexistent_file_returns_error() {
    let result = opi_coding_agent::image::load_image(&PathBuf::from("/nonexistent/photo.png"));
    assert!(result.is_err());
}

#[test]
fn load_image_unsupported_extension_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("photo.bmp");
    std::fs::write(&file_path, b"BM").unwrap();

    let result = opi_coding_agent::image::load_image(&file_path);
    assert!(result.is_err());
}
