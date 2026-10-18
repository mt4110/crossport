use crate::core::ProcessInfo;
use anyhow::Result;

#[cfg(any(unix, test))]
pub fn ownership_allowed(owner: Option<u32>, current: Option<u32>, all_users: bool) -> bool {
    all_users || matches!((owner,current), (Some(owner),Some(current)) if owner == current)
}
pub fn ensure_owned(process: &ProcessInfo, all_users: bool) -> Result<()> {
    let system = sysinfo::System::new_all();
    let current = system.process(sysinfo::Pid::from_u32(process.pid));
    anyhow::ensure!(
        process.started_at != 0
            && current.is_some_and(|p| p.start_time() == process.started_at
                && p.exe() == process.executable.as_deref()),
        "Process exited or changed; select it again"
    );
    #[cfg(unix)]
    {
        let owner = current.and_then(|p| p.user_id()).map(|uid| **uid);
        anyhow::ensure!(
            ownership_allowed(owner, Some(nix::unistd::geteuid().as_raw()), all_users),
            "Permission denied: owner differs or UID is unknown (CLI --all-users requires explicit opt-in)"
        );
    }
    #[cfg(not(unix))]
    {
        let caller = system
            .process(sysinfo::Pid::from_u32(std::process::id()))
            .and_then(|p| p.user_id());
        anyhow::ensure!(
            all_users
                || matches!((current.and_then(|p| p.user_id()), caller), (Some(owner),Some(caller)) if owner == caller),
            "Permission denied: process owner differs or is unknown"
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_and_foreign_uids_fail_closed() {
        assert!(ownership_allowed(Some(501), Some(501), false));
        for (owner, caller) in [
            (None, Some(501)),
            (Some(0), Some(501)),
            (Some(501), None),
            (None, None),
        ] {
            assert!(!ownership_allowed(owner, caller, false));
        }
        assert!(ownership_allowed(Some(0), Some(501), true));
    }
}
