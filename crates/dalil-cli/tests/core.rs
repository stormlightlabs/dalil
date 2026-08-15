use std::{path::PathBuf, process::Command};

use dalil_core::{AnalysisRequest, CacheMode, ColorPolicy, CommandDescriptor, OutputFormat, map, orient};

#[test]
fn core_map_matches_cli_json_without_cache() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut request = AnalysisRequest::new(CommandDescriptor::map(root.clone()));
    request.map.cache_mode = CacheMode::Disabled;
    request.output_format = OutputFormat::Json;
    request.color_policy = ColorPolicy::Auto;

    let library = map(request).expect("core map succeeds");
    let cli = cli_json("map", &root);
    let cli_map = cli["map"].clone();
    assert_eq!(serde_json::to_value(library).expect("core map serializes"), cli_map);
}

#[test]
fn core_orientation_matches_cli_json_without_cache() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut request = AnalysisRequest::new(CommandDescriptor::orient(root.clone()));
    request.map.cache_mode = CacheMode::Disabled;
    request.output_format = OutputFormat::Json;
    request.color_policy = ColorPolicy::Auto;

    let library = orient(request).expect("core orientation succeeds");
    let cli = cli_json("orient", &root);
    assert_eq!(
        serde_json::to_value(library).expect("core orientation serializes"),
        cli["orientation"]
    );
}

fn cli_json(command: &str, root: &PathBuf) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_dalil"))
        .args([command, "--no-cache", "--json"])
        .arg(root)
        .output()
        .expect("CLI starts");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}
