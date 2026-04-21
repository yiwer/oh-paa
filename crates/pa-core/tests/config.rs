use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use pa_core::{AppConfig, AppError};

#[test]
fn load_from_path_reads_expected_fields() {
    let temp_dir = create_temp_dir("valid");
    let config_path = temp_dir.join("config.toml");

    fs::write(
        &config_path,
        r#"
database_url = "sqlite::memory:"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://eastmoney.example"
twelvedata_base_url = "https://twelvedata.example"
twelvedata_api_key = "secret"
"#,
    )
    .expect("config should be written");

    let config = AppConfig::load_from_path(&config_path).expect("config should parse");

    assert_eq!(config.database_url, "sqlite::memory:");
    assert_eq!(config.server_addr, "127.0.0.1:3000");
    assert_eq!(config.eastmoney_base_url, "https://eastmoney.example");
    assert_eq!(config.twelvedata_base_url, "https://twelvedata.example");
    assert_eq!(config.twelvedata_api_key, "secret");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn load_from_path_rejects_unknown_keys_and_preserves_parse_source() {
    let temp_dir = create_temp_dir("unknown-key");
    let config_path = temp_dir.join("config.toml");

    fs::write(
        &config_path,
        r#"
database_url = "sqlite::memory:"
server_addr = "127.0.0.1:3000"
eastmoney_base_url = "https://eastmoney.example"
twelvedata_base_url = "https://twelvedata.example"
twelvedata_api_key = "secret"
unexpected = "boom"
"#,
    )
    .expect("config should be written");

    let error = AppConfig::load_from_path(&config_path).expect_err("unknown keys should fail");

    match error {
        AppError::Validation { .. } => {}
        other => panic!("expected validation error, got {other}"),
    }

    assert!(
        Error::source(&error).is_some(),
        "expected underlying parse source"
    );

    cleanup_temp_dir(&temp_dir);
}

fn create_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "pa-core-config-tests-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn cleanup_temp_dir(path: &Path) {
    fs::remove_dir_all(path).expect("temp dir should be removed");
}
