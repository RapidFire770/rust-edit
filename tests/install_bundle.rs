#[test]
fn install_script_layout_and_version_prompts() {
    let root = env!("CARGO_MANIFEST_DIR");
    let status = std::process::Command::new("sh")
        .arg(format!("{root}/tests/install_bundle.sh"))
        .status()
        .expect("run install bundle test");
    assert!(status.success());
}
