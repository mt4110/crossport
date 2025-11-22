use crossport::core::process::ProcessKind;
use std::path::PathBuf;

#[test]
fn test_process_kind_as_str() {
    assert_eq!(ProcessKind::System.as_str(), "system");
    assert_eq!(ProcessKind::Brew.as_str(), "brew");
    assert_eq!(ProcessKind::Dev.as_str(), "dev");
    assert_eq!(ProcessKind::Docker.as_str(), "docker");
    assert_eq!(ProcessKind::Other.as_str(), "other");
}

#[test]
fn test_env_backup_naming() {
    // Test that backup names preserve the full original filename
    let test_cases = vec![
        (".env", ".env.bak"),
        (".env.local", ".env.local.bak"),
        (".env.production", ".env.production.bak"),
    ];

    for (original, expected) in test_cases {
        let path = PathBuf::from(original);
        let file_name = path.file_name().unwrap().to_string_lossy();
        let backup_name = format!("{}.bak", file_name);
        let backup_path = path.with_file_name(backup_name);

        assert_eq!(backup_path.to_str().unwrap(), expected);
    }
}

#[test]
fn test_json_serialization() {
    use crossport::core::process::{ProcessInfo, ProcessKind};
    use std::path::PathBuf;

    let info = ProcessInfo {
        pid: 12345,
        user: "testuser".to_string(),
        uid: None,
        cmd: "node".to_string(),
        cwd: PathBuf::from("/test"),
        project_root: Some(PathBuf::from("/test/project")),
        container_name: None,
        kind: ProcessKind::Dev,
        port: 3000,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"pid\":12345"));
    assert!(json.contains("\"port\":3000"));
    assert!(json.contains("\"kind\":\"Dev\""));
}
