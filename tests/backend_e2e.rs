//! Exercises the real CLI and OS listener discovery. No termination signals are sent.
use serde_json::Value;
use std::{net::TcpListener, process::Command};

fn cli(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_crossport"))
        .args(["--config", "config/crossport.example.toml"])
        .args(args)
        .output()
        .expect("run crossport CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

#[test]
fn real_listener_scan_metadata_and_dry_run_preserve_process() {
    // The test owns the sockets; dropping them releases only these test ports.
    let first = TcpListener::bind(("127.0.0.1", 0)).expect("first test listener");
    let second = TcpListener::bind(("127.0.0.1", 0)).expect("second test listener");
    let ports = [
        first.local_addr().unwrap().port(),
        second.local_addr().unwrap().port(),
    ];
    for port in ports {
        let port_text = port.to_string();
        let scan = || -> Vec<Value> {
            serde_json::from_str(&cli(&[
                "scan", "--from", &port_text, "--to", &port_text, "--json",
            ]))
            .expect("JSON scan results")
        };
        let rows = scan();
        assert!(!rows.is_empty(), "real listener must be discovered");
        assert!(rows.iter().all(|row| row["port"] == port));
        let own = rows
            .iter()
            .find(|row| row["pid"] == std::process::id())
            .expect("test process owns listener");
        assert_eq!(own["local_addr"], "127.0.0.1");
        #[cfg(unix)]
        assert_eq!(own["uid"], nix::unistd::geteuid().as_raw());
        assert!(own["started_at"].as_u64().unwrap() > 0);
        assert!(own["parent_pid"].as_u64().unwrap() > 0);
        assert!(!own["executable"].as_str().unwrap().is_empty());
        assert!(own["uptime_secs"].is_u64());

        let dry_run = cli(&["kill", &port_text, "--dry-run", "--interactive", "false"]);
        assert!(dry_run.contains(&format!("Would kill PID {}", std::process::id())));
        assert!(
            scan().iter().any(|row| row["pid"] == std::process::id()),
            "dry run must preserve the listener"
        );
        assert!(
            TcpListener::bind(("127.0.0.1", port)).is_err(),
            "test socket remains bound"
        );
    }
    // Negative case: the listener disappears when this test releases it.
    let released_port = ports[0].to_string();
    drop(first);
    let rows: Vec<Value> = serde_json::from_str(&cli(&[
        "scan",
        "--from",
        &released_port,
        "--to",
        &released_port,
        "--json",
    ]))
    .unwrap();
    assert!(
        !rows.iter().any(|row| row["pid"] == std::process::id()),
        "released listener must disappear"
    );
}

#[test]
fn config_default_port_verbose_color_history_and_cli_override() {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("crossport-e2e-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let history = dir.join("events.lock");
    let config = dir.join("crossport.toml");
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    fs::write(&config, format!("[general]\ndefault_port={port}\n[output]\nverbose=true\ncolor=false\n[history]\nenabled=true\npath={:?}\nmax_entries=2\n",history.to_string_lossy())).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_crossport"))
            .args(["--config", config.to_str().unwrap()])
            .args(args)
            .env("CLICOLOR_FORCE", "1")
            .output()
            .unwrap()
    };
    // Regression: the data filename itself may have a .lock extension.
    let empty = run(&["history", "--json"]);
    assert!(
        empty.status.success(),
        "{}",
        String::from_utf8_lossy(&empty.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Vec<Value>>(&empty.stdout).unwrap(),
        Vec::<Value>::new()
    );
    assert!(
        !history.exists(),
        "reading empty history must not create data"
    );
    let lookup = run(&[]);
    assert!(
        lookup.status.success(),
        "{}",
        String::from_utf8_lossy(&lookup.stderr)
    );
    let text = String::from_utf8(lookup.stdout).unwrap();
    assert!(text.contains(&port.to_string()));
    assert!(text.contains("executable="));
    assert!(!text.contains('\x1b'));
    for _ in 0..3 {
        let result = run(&[
            "kill",
            &port.to_string(),
            "--dry-run",
            "--interactive",
            "false",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let persisted: Vec<Value> = serde_json::from_slice(&fs::read(&history).unwrap()).unwrap();
    assert_eq!(persisted.len(), 2);
    for event in &persisted {
        assert_eq!(event["pid"], std::process::id());
        assert_eq!(event["outcome"], "dry-run");
        assert_eq!(event.as_object().unwrap().len(), 5);
    }
    let output = run(&["history", "--json"]);
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Vec<Value>>(&output.stdout).unwrap(),
        persisted
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(&history).unwrap().mode() & 0o777, 0o600);
    }
    fs::write(&config, "[output]\nverbose=false\ncolor=false\n").unwrap();
    let normal = run(&[&port.to_string()]);
    assert!(!String::from_utf8_lossy(&normal.stdout).contains("executable="));
    let verbose = run(&["--verbose", &port.to_string()]);
    assert!(verbose.status.success());
    assert!(String::from_utf8_lossy(&verbose.stdout).contains("executable="));
    assert!(TcpListener::bind(("127.0.0.1", port)).is_err());
    fs::write(&config, "[backend]\nstrategy='invalid'\n").unwrap();
    let invalid = run(&["scan"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("config"));
}
