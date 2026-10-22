//! Resolving process ids to names, paths and parents.

use std::collections::HashMap;
use std::path::PathBuf;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::model::ProcessInfo;

/// Caches the process list so the UI can resolve pids cheaply.
pub struct ProcessCache {
    system: System,
    /// Snapshot of the last refresh, keyed by pid.
    known: HashMap<u32, ProcessInfo>,
}

impl ProcessCache {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        Self {
            system,
            known: HashMap::new(),
        }
    }

    /// Re-read the process list.
    pub fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        self.known.clear();
    }

    /// Look up a process by pid.
    pub fn get(&self, pid: u32) -> Option<ProcessInfo> {
        if let Some(info) = self.known.get(&pid) {
            return Some(info.clone());
        }

        let process = self.system.process(Pid::from(pid as usize))?;
        let name = process.name().to_string_lossy().to_string();
        let exe = process.exe().map(PathBuf::from);
        let parent = process.parent().and_then(|p| {
            self.system
                .process(p)
                .map(|p| p.name().to_string_lossy().to_string())
        });

        let info = ProcessInfo {
            pid,
            name,
            exe,
            parent,
        };
        Some(info)
    }
}

impl Default for ProcessCache {
    fn default() -> Self {
        Self::new()
    }
}
