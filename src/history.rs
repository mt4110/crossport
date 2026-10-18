//! Minimal local action history. Persistence is opt-in; arguments and paths are never recorded.
use crate::config::HistoryConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Stop,
    Restart,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Success,
    Failed,
    DryRun,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub timestamp: u64,
    pub pid: u32,
    pub port: u16,
    pub action: Action,
    pub outcome: Outcome,
}
impl Event {
    pub fn new(pid: u32, port: u16, action: Action, outcome: Outcome) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            pid,
            port,
            action,
            outcome,
        }
    }
    pub fn line(&self) -> String {
        format!(
            "{} UTC-unix | PID {} | port {} | {:?} | {:?}",
            self.timestamp, self.pid, self.port, self.action, self.outcome
        )
    }
}
fn path(config: &HistoryConfig) -> Result<PathBuf> {
    config
        .path
        .clone()
        .or_else(|| dirs::data_local_dir().map(|p| p.join("crossport/history.json")))
        .context("History path unavailable")
}
fn private_open(path: &Path, create_new: bool) -> Result<File> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "History symlinks are not allowed"
        );
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .context("Cannot open private history file")?;
    let meta = file.metadata()?;
    anyhow::ensure!(meta.is_file(), "History path is not a regular file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        anyhow::ensure!(
            meta.uid() == nix::unistd::geteuid().as_raw()
                && meta.mode() & 0o077 == 0
                && meta.nlink() == 1,
            "History file must be private and owned by the current user"
        );
    }
    Ok(file)
}
fn lock(config: &HistoryConfig) -> Result<(PathBuf, File)> {
    let path = path(config)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(parent)?;
    }
    let mut lock_name = path.as_os_str().to_os_string();
    lock_name.push(".lock");
    let file = private_open(Path::new(&lock_name), false)?;
    file.try_lock().context("History is busy")?;
    Ok((path, file))
}
fn read_at(path: &Path) -> Result<Vec<Event>> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "History symlinks are not allowed"
        );
    }
    if !path.try_exists()? {
        return Ok(Vec::new());
    }
    let mut file = private_open(path, false)?;
    anyhow::ensure!(
        file.metadata()?.len() <= 2_000_000,
        "History exceeds size limit"
    );
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let events =
        serde_json::from_str(&text).context("History is corrupt; existing data preserved")?;
    Ok(events)
}
pub fn read(config: &HistoryConfig) -> Result<Vec<Event>> {
    if !config.enabled {
        return Ok(Vec::new());
    }
    let (path, _lock) = lock(config)?;
    let mut events = read_at(&path)?;
    retain(&mut events, config.max_entries);
    Ok(events)
}
pub fn record(config: &HistoryConfig, event: &Event) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }
    anyhow::ensure!(
        (1..=10_000).contains(&config.max_entries),
        "Invalid history retention"
    );
    let (path, _lock) = lock(config)?;
    let mut events = read_at(&path)?;
    events.push(event.clone());
    retain(&mut events, config.max_entries);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temporary = path.with_extension(format!("{}.{}.tmp", std::process::id(), stamp));
    let mut file = private_open(&temporary, true)?;
    file.write_all(&serde_json::to_vec(&events)?)?;
    file.sync_all()?;
    fs::rename(&temporary, &path).context("Cannot replace history atomically")?;
    Ok(())
}
fn retain(events: &mut Vec<Event>, max: usize) {
    if events.len() > max {
        events.drain(..events.len() - max);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Wall-clock timestamps alone can collide across parallel test threads.
    static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn fixture() -> HistoryConfig {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        HistoryConfig {
            enabled: true,
            path: Some(std::env::temp_dir().join(format!(
                "crossport-history-{}-{stamp}-{sequence}/history.json",
                std::process::id()
            ))),
            max_entries: 2,
        }
    }
    #[test]
    fn lock_extension_history_round_trips_without_name_collision() {
        let mut config = fixture();
        config.path = config.path.map(|path| path.with_extension("lock"));
        let data = config.path.as_ref().unwrap();
        assert!(read(&config).unwrap().is_empty());
        assert!(
            !data.exists(),
            "reading empty history must not create the data file"
        );
        record(
            &config,
            &Event::new(12, 8080, Action::Stop, Outcome::DryRun),
        )
        .unwrap();
        let events = read(&config).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pid, 12);
        assert!(data.with_file_name("history.lock.lock").exists());
        let lock_path = data.with_file_name("history.lock.lock");
        let file = private_open(&lock_path, false).unwrap();
        file.try_lock().unwrap();
        assert!(
            record(
                &config,
                &Event::new(13, 8080, Action::Stop, Outcome::DryRun)
            )
            .is_err()
        );
        drop(file);
        assert_eq!(read(&config).unwrap().len(), 1);
    }

    #[test]
    fn persistence_retention_private_permissions_and_corruption_preservation() {
        let config = fixture();
        for pid in 1..=3 {
            record(
                &config,
                &Event::new(pid, 8080, Action::Stop, Outcome::DryRun),
            )
            .unwrap();
        }
        let events = read(&config).unwrap();
        assert_eq!(events.iter().map(|e| e.pid).collect::<Vec<_>>(), vec![2, 3]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(config.path.as_ref().unwrap()).unwrap().mode() & 0o777,
                0o600
            );
        }
        fs::write(config.path.as_ref().unwrap(), b"broken history").unwrap();
        assert!(record(&config, &Event::new(4, 1, Action::Stop, Outcome::Success)).is_err());
        assert_eq!(
            fs::read(config.path.as_ref().unwrap()).unwrap(),
            b"broken history"
        );
    }
    #[test]
    fn concurrent_writer_reports_busy_without_overwriting() {
        let config = fixture();
        let (_path, _lock) = lock(&config).unwrap();
        assert!(record(&config, &Event::new(1, 1, Action::Stop, Outcome::Failed)).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_history_is_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;
        let config = fixture();
        let (path, locked) = lock(&config).unwrap();
        drop(locked);
        let target = path.with_extension("target");
        fs::write(&target, b"preserve").unwrap();
        symlink(&target, &path).unwrap();
        assert!(record(&config, &Event::new(1, 1, Action::Stop, Outcome::Success)).is_err());
        assert_eq!(fs::read(target).unwrap(), b"preserve");
    }

    #[test]
    fn disabled_history_never_creates_files() {
        let config = HistoryConfig {
            path: Some(PathBuf::from("/unavailable/history")),
            ..Default::default()
        };
        record(&config, &Event::new(1, 2, Action::Stop, Outcome::Failed)).unwrap();
        assert!(read(&config).unwrap().is_empty());
    }
    #[test]
    fn schema_excludes_private_process_details() {
        let json = serde_json::to_value(Event::new(1, 2, Action::Stop, Outcome::Success)).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 5);
        assert!(json.get("args").is_none());
        assert!(json.get("cwd").is_none());
    }
}
