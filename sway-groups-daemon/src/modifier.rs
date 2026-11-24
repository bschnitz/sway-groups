//! Expanding the collapsed workspaces while the bar modifier is held.
//!
//! sway reports the modifier of a bar in `mode hide` as `bar_state_update`
//! events: `visible_by_modifier` turns true on press and false on release.
//! The user's real bar docks and gets none, so a second, invisible bar
//! (`mode hide`, `swaybar_command true`) is configured only to produce them.
//!
//! The same events arrive for every `$mod+…` binding. So the bar expands only
//! once the modifier has been held for [`HOLD`] — a quick binding leaves it
//! folded — and collapses at once on release. The state goes to the DB
//! (`bar_expanded`) rather than living here, because swayg's own commands
//! redraw the bar too, and a `$mod+l` pressed while expanded must not fold it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;

use sway_groups_config::SwaygConfig;
use sway_groups_core::db::DatabaseManager;
use sway_groups_core::services::WaybarSyncService;
use sway_groups_core::sway::SwayIpcClient;
use tracing::{error, info, warn};

/// How long the modifier has to be held before the bar expands.
const HOLD: Duration = Duration::from_millis(300);

#[derive(Clone)]
pub struct ModifierWatcher {
    db_path: PathBuf,
    ipc: SwayIpcClient,
    config: Arc<SwaygConfig>,
    /// Bumped by every event; a delayed expand that finds it moved on was
    /// overtaken by a release.
    generation: Arc<AtomicU64>,
    /// Whether this daemon set `bar_expanded`, so a release only redraws
    /// when there is something to fold back.
    expanded: Arc<AtomicBool>,
    /// Serialises the DB writes, so an expand and a release racing each
    /// other cannot leave the older state on record.
    writing: Arc<Mutex<()>>,
}

impl ModifierWatcher {
    pub fn new(db_path: PathBuf, ipc: SwayIpcClient, config: Arc<SwaygConfig>) -> Self {
        Self {
            db_path,
            ipc,
            config,
            generation: Arc::new(AtomicU64::new(0)),
            expanded: Arc::new(AtomicBool::new(false)),
            writing: Arc::new(Mutex::new(())),
        }
    }

    /// Clear a `bar_expanded` left behind by a daemon that died while the
    /// modifier was held. It does not redraw: the daemon has not subscribed
    /// yet, and the next redraw folds the bar anyway.
    pub async fn reset(&self) {
        self.write(false).await;
    }

    /// Note a `bar_state_update` event payload.
    pub fn bar_state_update(&self, payload: &[u8]) {
        let event: serde_json::Value = match serde_json::from_slice(payload) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse bar_state_update event: {}", e);
                return;
            }
        };
        let held = event
            .get("visible_by_modifier")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let watcher = self.clone();
        tokio::spawn(async move {
            if held {
                tokio::time::sleep(HOLD).await;
                if watcher.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                watcher.expanded.store(true, Ordering::SeqCst);
                watcher.write(true).await;
            } else if watcher.expanded.swap(false, Ordering::SeqCst) {
                watcher.write(true).await;
            }
        });
    }

    /// Put the current state on record and, if `redraw`, redraw the bar. It
    /// reads the state only once it holds the lock, so the last writer always
    /// writes the latest.
    async fn write(&self, redraw: bool) {
        let _writing = self.writing.lock().await;
        let expanded = self.expanded.load(Ordering::SeqCst);
        let db = match DatabaseManager::new(self.db_path.clone()).await {
            Ok(db) => db,
            Err(e) => {
                error!("Failed to open DB '{}': {}", self.db_path.display(), e);
                return;
            }
        };
        let waybar = WaybarSyncService::with_config(db, self.ipc.clone(), &self.config);
        if let Err(e) = waybar.set_bar_expanded(expanded).await {
            warn!("Failed to set bar_expanded = {}: {}", expanded, e);
            return;
        }
        info!("bar_expanded = {}", expanded);
        if redraw && let Err(e) = waybar.update_waybar().await {
            warn!("Failed to redraw the workspaces bar: {}", e);
        }
    }
}
