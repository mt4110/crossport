//! Real stop lifecycle, confined to a subprocess created by this test.
#![cfg(unix)]
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
// Both cases create processes observed by real lsof. Avoid one case changing
// process visibility while the other captures its process metadata snapshot.
static LISTENER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "subprocess fixture; launched only by owned_child_can_be_stopped"]
fn listener_fixture() {
    assert_eq!(std::env::var("CROSSPORT_STOP_FIXTURE").as_deref(), Ok("1"));
    let _listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    println!("FIXTURE_PORT={}", _listener.local_addr().unwrap().port());
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park_timeout(Duration::from_secs(60));
    }
}
fn spawn_listener() -> (Fixture, String) {
    let mut child = Fixture(
        Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "listener_fixture", "--nocapture"])
            .env("CROSSPORT_STOP_FIXTURE", "1")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some((_, port)) = line.split_once("FIXTURE_PORT=") {
                let _ = tx.send(port.trim().to_string());
                break;
            }
        }
    });
    let port = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("fixture must become ready");
    (child, port)
}

#[test]
fn owned_child_can_be_stopped_and_disappears_from_scan() {
    let _serial = LISTENER_TEST_LOCK.lock().unwrap();
    // Keep discovery observable in the isolated builder after the child exits.
    // lsof exit 1 can mean either no visible listeners or a scan error; the
    // application deliberately rejects that ambiguous result when warnings exist.
    let _discovery_anchor = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let (mut child, port) = spawn_listener();
    let pid = child.0.id();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_crossport"))
            .args(["--config", "config/crossport.example.toml"])
            .args(args)
            .output()
            .unwrap()
    };
    let scan = run(&["scan", "--from", &port, "--to", &port, "--json"]);
    assert!(
        scan.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&scan.stdout).unwrap();
    assert!(rows.iter().any(|row| row["pid"] == pid));
    let stop = run(&["kill", &port, "--signal", "TERM", "--interactive", "false"]);
    assert!(
        stop.status.success(),
        "{}",
        String::from_utf8_lossy(&stop.stderr)
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.0.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "fixture must exit after TERM");
        std::thread::sleep(Duration::from_millis(20));
    }
    let scan = run(&["scan", "--from", &port, "--to", &port, "--json"]);
    assert!(
        scan.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&scan.stdout).unwrap();
    assert!(!rows.iter().any(|row| row["pid"] == pid));
}

#[test]
fn listener_born_during_scan_is_not_reported_as_free() {
    let _serial = LISTENER_TEST_LOCK.lock().unwrap();
    use std::{
        fs,
        io::Read,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn quote(path: &Path) -> String {
        format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "crossport-scan-race-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let shell = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("sh"))
        .find(|p| p.is_file())
        .expect("POSIX shell for backend fixture");
    let ready = dir.join("ready");
    let reply = dir.join("reply");
    let lsof = dir.join("lsof");
    fs::write(&lsof,format!("#!{}\n: > {}\nn=0\nwhile [ ! -f {} ]; do\n n=$((n + 1))\n [ \"$n\" -lt 200 ] || exit 2\n sleep 0.05\ndone\nwhile IFS= read -r line; do printf '%s\\n' \"$line\"; done < {}\n",shell.display(),quote(&ready),quote(&reply),quote(&reply))).unwrap();
    let docker = dir.join("docker");
    fs::write(&docker, format!("#!{}\nexit 0\n", shell.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for path in [&lsof, &docker] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut paths = vec![dir.clone()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut cli = Fixture(
        Command::new(env!("CARGO_BIN_EXE_crossport"))
            .args([
                "--config",
                "config/crossport.example.toml",
                "scan",
                "--from",
                "1",
                "--to",
                "65535",
                "--json",
            ])
            .env("PATH", std::env::join_paths(paths).unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(Instant::now() < deadline, "backend fixture must start");
        assert!(
            cli.0.try_wait().unwrap().is_none(),
            "CLI must wait for listener discovery"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // The application's process snapshot already exists. Only now start a listener.
    let (mut listener, port) = spawn_listener();
    let pid = listener.0.id();
    let temporary = dir.join("reply.ready");
    fs::write(&temporary, format!("p{pid}\nn127.0.0.1:{port}\n")).unwrap();
    fs::rename(&temporary, &reply).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = cli.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "CLI must finish discovery");
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = String::new();
    cli.0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    let mut stderr = String::new();
    cli.0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(
        !status.success(),
        "incomplete discovery must not succeed: {stdout}"
    );
    assert!(
        stdout.is_empty(),
        "no partial or free-port output: {stdout}"
    );
    assert!(
        stderr.contains("Scan incomplete")
            && stderr.contains(&port)
            && stderr.contains(&pid.to_string()),
        "{stderr}"
    );
    assert!(
        listener.0.try_wait().unwrap().is_none(),
        "fixture listener must still be running"
    );
    let _connection = std::net::TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap()))
        .expect("actual listener remains reachable");
}
