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
