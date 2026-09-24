//! The installer parses this exact line. Keep the format stable.

#[test]
fn version_flag_prints_package_version_and_exits() {
    let exe = env!("CARGO_BIN_EXE_rust-edit");
    let out = std::process::Command::new(exe)
        .arg("--version")
        .output()
        .expect("run rust-edit --version");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).expect("utf-8");
    assert_eq!(text, format!("rust-edit {}\n", env!("CARGO_PKG_VERSION")));
}
