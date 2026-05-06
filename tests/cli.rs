use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steamfetch"))
}

fn unique_temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "steamfetch-cli-test-{}-{}-{}",
        label,
        std::process::id(),
        nanos
    ))
}

#[test]
fn config_path_flag_prints_config_path_and_exits() {
    let root = unique_temp_root("config-path");
    std::fs::create_dir_all(&root).unwrap();

    let output = Command::new(binary())
        .arg("--config-path")
        .env("XDG_CONFIG_HOME", &root)
        .output()
        .expect("steamfetch should run");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    let printed = PathBuf::from(stdout.trim());

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(printed.ends_with(Path::new("steamfetch").join("config.toml")));
    assert!(
        stderr.is_empty(),
        "--config-path should not print diagnostics on success"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn demo_flag_renders_demo_profile_without_config() {
    let output = Command::new(binary())
        .arg("--demo")
        .output()
        .expect("steamfetch should run");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stdout.contains("unhappychoice@Steam"));
    assert!(stdout.contains("Games:"));
    assert!(stdout.contains("Top Played"));
}
