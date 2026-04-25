use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=DOWNLOAD_MODELS");
    println!("cargo:rerun-if-env-changed=DEPTH_MODEL_URL");
    println!("cargo:rerun-if-env-changed=LAMA_MODEL_URL");
    println!("cargo:rerun-if-env-changed=MODEL_DIR");

    if std::env::var("DOWNLOAD_MODELS").is_err() {
        return;
    }

    let model_dir = std::env::var("MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt"));

    if let Ok(url) = std::env::var("DEPTH_MODEL_URL") {
        let dest = model_dir.join("depth_model.onnx");
        if !dest.exists() {
            download_model(&url, &dest);
        }
    }

    if let Ok(url) = std::env::var("LAMA_MODEL_URL") {
        let dest = model_dir.join("lama_model.onnx");
        if !dest.exists() {
            download_model(&url, &dest);
        }
    }
}

fn download_model(url: &str, dest: &Path) {
    eprintln!("build.rs: downloading model from {url} → {}", dest.display());
    let output = std::process::Command::new("curl")
        .args(["-fSL", "--create-dirs", "-o", dest.to_str().unwrap(), url])
        .output()
        .expect("curl is required to download models");

    if !output.status.success() {
        std::io::stderr().write_all(&output.stderr).ok();
        panic!("failed to download model from {url}");
    }
}
