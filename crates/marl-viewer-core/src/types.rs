//! Shared metadata types used by both the renderer and GUI.
//!
//! These types are pure data — they have no GPU or windowing dependencies.
//!
//! - [`SnapshotInfo`]: lightweight display metadata for the currently loaded snapshot
//! - [`GuiAction`]: user actions emitted by the GUI and processed by the renderer
//! - [`choose_initial_tick`]: pick the best initial tick given available snapshots
//! - [`neighbor_tick`]: navigate to a neighboring tick in a sorted list

use std::path::PathBuf;

use crate::args::{CellMode, ViewMode};

// ---------------------------------------------------------------------------
// SnapshotInfo
// ---------------------------------------------------------------------------

/// Lightweight display metadata describing the currently loaded snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub output_dir: PathBuf,
    pub tick: u64,
    pub species: u32,
    pub view_mode: ViewMode,
    pub cell_mode: CellMode,
    pub cell_count: usize,
    pub field_bytes: usize,
    pub grid: [u32; 3],
    pub s_ext: u32,
}

// ---------------------------------------------------------------------------
// GuiAction
// ---------------------------------------------------------------------------

/// Action emitted by the GUI that the renderer should process.
#[derive(Debug, Clone)]
pub enum GuiAction {
    OpenDirectoryDialog,
    LoadDirectory(PathBuf),
    LoadTick(u64),
    ReloadCurrent,
    FirstTick,
    LastTick,
    PrevTick,
    NextTick,
    ApplyViewSettings,
    ResetDraftFromLoaded,
}

// ---------------------------------------------------------------------------
// Tick navigation helpers
// ---------------------------------------------------------------------------

/// Choose the best initial tick given what the user asked for and what exists.
/// Returns the requested tick if it is in the list, else `0` if present,
/// otherwise the first (minimum) tick. Returns `None` if no ticks are available.
pub fn choose_initial_tick(requested: u64, available: &[u64]) -> Option<u64> {
    if available.is_empty() {
        return None;
    }
    if available.binary_search(&requested).is_ok() {
        return Some(requested);
    }
    if available.binary_search(&0).is_ok() {
        return Some(0);
    }
    available.first().copied()
}

/// Find the neighboring tick at `delta` offset in the sorted list.
/// Clamps at ends (returns `None` if moving past first or last).
pub fn neighbor_tick(current: u64, available: &[u64], delta: i32) -> Option<u64> {
    if available.is_empty() {
        return None;
    }
    let pos = available.binary_search(&current).ok()?;
    let new_pos = if delta < 0 {
        pos.checked_sub((-delta) as usize)?
    } else {
        pos.checked_add(delta as usize)?
    };
    available.get(new_pos).copied()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_initial_tick_requested_present() {
        let ticks = vec![0, 10, 20, 30];
        assert_eq!(choose_initial_tick(10, &ticks), Some(10));
    }

    #[test]
    fn choose_initial_tick_zero_fallback() {
        let ticks = vec![0, 500, 1000];
        assert_eq!(choose_initial_tick(42, &ticks), Some(0));
    }

    #[test]
    fn choose_initial_tick_first_when_zero_missing() {
        let ticks = vec![5, 10, 15];
        assert_eq!(choose_initial_tick(42, &ticks), Some(5));
    }

    #[test]
    fn choose_initial_tick_empty_returns_none() {
        assert_eq!(choose_initial_tick(0, &[]), None);
    }

    #[test]
    fn neighbor_tick_next() {
        let ticks = vec![0, 500, 1000];
        assert_eq!(neighbor_tick(500, &ticks, 1), Some(1000));
        assert_eq!(neighbor_tick(1000, &ticks, 1), None); // at end
    }

    #[test]
    fn neighbor_tick_prev() {
        let ticks = vec![0, 500, 1000];
        assert_eq!(neighbor_tick(500, &ticks, -1), Some(0));
        assert_eq!(neighbor_tick(0, &ticks, -1), None); // at start
    }

    #[test]
    fn neighbor_tick_current_not_found() {
        let ticks = vec![0, 500, 1000];
        assert_eq!(neighbor_tick(42, &ticks, 1), None);
    }

    #[test]
    fn neighbor_tick_empty() {
        assert_eq!(neighbor_tick(0, &[], 1), None);
    }
}
