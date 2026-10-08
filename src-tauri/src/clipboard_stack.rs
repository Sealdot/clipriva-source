//! Process-memory clipboard Stack state.
//!
//! The Stack deliberately owns only opaque clipboard Item identifiers and
//! transition state. It has no serializer and is never written to SQLite,
//! preferences, diagnostics, logs, exports, or Local Link.

use std::collections::HashSet;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

pub const CLIPBOARD_STACK_LIMIT: usize = 20;
pub const CLIPBOARD_STACK_CHANGED_EVENT: &str = "clipriva://clipboard-stack-changed-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardStackAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardStackEntry {
    pub item_id: String,
    pub availability: ClipboardStackAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardStackSnapshot {
    pub version: u8,
    pub order: Vec<ClipboardStackEntry>,
    pub cursor: usize,
    pub collecting: bool,
    pub busy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardStackEnqueueResult {
    pub added_count: usize,
    pub duplicate_count: usize,
    pub limit_skipped_count: usize,
    pub snapshot: ClipboardStackSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardStackReservation {
    pub token: String,
    pub item_id: String,
}

fn emit_snapshot(app: &AppHandle, snapshot: &ClipboardStackSnapshot) {
    let _ = app.emit(CLIPBOARD_STACK_CHANGED_EVENT, snapshot);
}

#[tauri::command]
pub fn get_clipboard_stack(stack: State<'_, ClipboardStack>) -> ClipboardStackSnapshot {
    stack.snapshot()
}

#[tauri::command]
pub fn add_clipboard_items_to_stack(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
    item_ids: Vec<String>,
) -> ClipboardStackEnqueueResult {
    let result = stack.enqueue(&item_ids);
    emit_snapshot(&app, &result.snapshot);
    result
}

#[tauri::command]
pub fn set_clipboard_stack_collecting(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
    collecting: bool,
) -> ClipboardStackSnapshot {
    let snapshot = stack.set_collecting(collecting);
    emit_snapshot(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn move_clipboard_stack_item(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
    from: usize,
    to: usize,
) -> Result<ClipboardStackSnapshot, String> {
    let snapshot = stack
        .move_waiting(from, to)
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn remove_clipboard_stack_item(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
    index: usize,
) -> Result<ClipboardStackSnapshot, String> {
    let snapshot = stack
        .remove_waiting(index)
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn begin_clipboard_stack_activation(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
) -> Result<ClipboardStackReservation, String> {
    let reservation = stack
        .reserve_activation()
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &stack.snapshot());
    Ok(reservation)
}

#[tauri::command]
pub fn finish_clipboard_stack_activation(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
    token: String,
    succeeded: bool,
) -> Result<ClipboardStackSnapshot, String> {
    let snapshot = stack
        .finish_activation(&token, succeeded)
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn advance_clipboard_stack(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
) -> Result<ClipboardStackSnapshot, String> {
    let snapshot = stack
        .advance_without_activation()
        .map_err(|error| error.to_string())?;
    emit_snapshot(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn mark_clipboard_stack_item_unavailable(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
    item_id: String,
) -> ClipboardStackSnapshot {
    let snapshot = stack.mark_unavailable(&item_id);
    emit_snapshot(&app, &snapshot);
    snapshot
}

#[tauri::command]
pub fn reset_clipboard_stack(
    app: AppHandle,
    stack: State<'_, ClipboardStack>,
) -> Result<ClipboardStackSnapshot, String> {
    let snapshot = stack.reset().map_err(|error| error.to_string())?;
    emit_snapshot(&app, &snapshot);
    Ok(snapshot)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardStackError {
    Empty,
    Busy,
    Unavailable,
    InvalidPosition,
    CompletedEntry,
    StaleReservation,
}

impl std::fmt::Display for ClipboardStackError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Empty => "The Stack is empty.",
            Self::Busy => "The current Stack item is already being activated.",
            Self::Unavailable => "The current Stack item is unavailable.",
            Self::InvalidPosition => "That Stack position is invalid.",
            Self::CompletedEntry => "Completed Stack items cannot be changed.",
            Self::StaleReservation => "That Stack activation is no longer current.",
        };
        formatter.write_str(message)
    }
}

#[derive(Debug, Default)]
struct ClipboardStackInner {
    order: Vec<ClipboardStackEntry>,
    cursor: usize,
    collecting: bool,
    busy_token: Option<String>,
}

/// The one native source of truth shared by the main and Quick Paste windows.
#[derive(Debug, Default)]
pub struct ClipboardStack {
    inner: Mutex<ClipboardStackInner>,
}

impl ClipboardStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> ClipboardStackSnapshot {
        let inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        snapshot(&inner)
    }

    pub fn enqueue(&self, item_ids: &[String]) -> ClipboardStackEnqueueResult {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        enqueue(&mut inner, item_ids)
    }

    /// Adds one newly persisted capture only while explicit collect mode is on.
    pub fn enqueue_captured(&self, item_id: &str) -> Option<ClipboardStackEnqueueResult> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        if !inner.collecting {
            return None;
        }
        Some(enqueue(&mut inner, &[item_id.to_owned()]))
    }

    pub fn set_collecting(&self, collecting: bool) -> ClipboardStackSnapshot {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        inner.collecting = collecting;
        snapshot(&inner)
    }

    pub fn move_waiting(
        &self,
        from: usize,
        to: usize,
    ) -> Result<ClipboardStackSnapshot, ClipboardStackError> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        if inner.busy_token.is_some() {
            return Err(ClipboardStackError::Busy);
        }
        if from >= inner.order.len() || to >= inner.order.len() {
            return Err(ClipboardStackError::InvalidPosition);
        }
        if from < inner.cursor || to < inner.cursor {
            return Err(ClipboardStackError::CompletedEntry);
        }
        if from != to {
            let entry = inner.order.remove(from);
            inner.order.insert(to, entry);
        }
        Ok(snapshot(&inner))
    }

    pub fn remove_waiting(
        &self,
        index: usize,
    ) -> Result<ClipboardStackSnapshot, ClipboardStackError> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        if inner.busy_token.is_some() {
            return Err(ClipboardStackError::Busy);
        }
        if index >= inner.order.len() {
            return Err(ClipboardStackError::InvalidPosition);
        }
        if index < inner.cursor {
            return Err(ClipboardStackError::CompletedEntry);
        }
        inner.order.remove(index);
        if inner.cursor > inner.order.len() {
            inner.cursor = inner.order.len();
        }
        Ok(snapshot(&inner))
    }

    pub fn reserve_activation(&self) -> Result<ClipboardStackReservation, ClipboardStackError> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        if inner.busy_token.is_some() {
            return Err(ClipboardStackError::Busy);
        }
        let current = inner
            .order
            .get(inner.cursor)
            .ok_or(ClipboardStackError::Empty)?;
        if current.availability == ClipboardStackAvailability::Unavailable {
            return Err(ClipboardStackError::Unavailable);
        }
        let item_id = current.item_id.clone();
        let token = Uuid::new_v4().to_string();
        inner.busy_token = Some(token.clone());
        Ok(ClipboardStackReservation { token, item_id })
    }

    pub fn finish_activation(
        &self,
        token: &str,
        succeeded: bool,
    ) -> Result<ClipboardStackSnapshot, ClipboardStackError> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        match inner.busy_token.as_deref() {
            Some(active_token) if active_token == token => {}
            _ => return Err(ClipboardStackError::StaleReservation),
        }
        inner.busy_token = None;
        if succeeded && inner.cursor < inner.order.len() {
            inner.cursor += 1;
        }
        Ok(snapshot(&inner))
    }

    pub fn advance_without_activation(
        &self,
    ) -> Result<ClipboardStackSnapshot, ClipboardStackError> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        if inner.busy_token.is_some() {
            return Err(ClipboardStackError::Busy);
        }
        if inner.cursor >= inner.order.len() {
            return Err(ClipboardStackError::Empty);
        }
        inner.cursor += 1;
        Ok(snapshot(&inner))
    }

    pub fn mark_unavailable(&self, item_id: &str) -> ClipboardStackSnapshot {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        for entry in &mut inner.order {
            if entry.item_id == item_id {
                entry.availability = ClipboardStackAvailability::Unavailable;
            }
        }
        snapshot(&inner)
    }

    pub fn reset(&self) -> Result<ClipboardStackSnapshot, ClipboardStackError> {
        let mut inner = self.inner.lock().expect("clipboard Stack mutex poisoned");
        if inner.busy_token.is_some() {
            return Err(ClipboardStackError::Busy);
        }
        *inner = ClipboardStackInner::default();
        Ok(snapshot(&inner))
    }
}

fn enqueue(inner: &mut ClipboardStackInner, item_ids: &[String]) -> ClipboardStackEnqueueResult {
    let mut existing = inner
        .order
        .iter()
        .map(|entry| entry.item_id.clone())
        .collect::<HashSet<_>>();
    let mut added_count = 0;
    let mut duplicate_count = 0;
    let mut limit_skipped_count = 0;

    for item_id in item_ids {
        let item_id = item_id.trim();
        if item_id.is_empty() {
            continue;
        }
        if existing.contains(item_id) {
            duplicate_count += 1;
            continue;
        }
        if inner.order.len() >= CLIPBOARD_STACK_LIMIT {
            limit_skipped_count += 1;
            continue;
        }
        inner.order.push(ClipboardStackEntry {
            item_id: item_id.to_owned(),
            availability: ClipboardStackAvailability::Available,
        });
        added_count += 1;
        existing.insert(item_id.to_owned());
    }

    ClipboardStackEnqueueResult {
        added_count,
        duplicate_count,
        limit_skipped_count,
        snapshot: snapshot(inner),
    }
}

fn snapshot(inner: &ClipboardStackInner) -> ClipboardStackSnapshot {
    ClipboardStackSnapshot {
        version: 1,
        order: inner.order.clone(),
        cursor: inner.cursor,
        collecting: inner.collecting,
        busy: inner.busy_token.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClipboardStack, ClipboardStackAvailability, ClipboardStackError, CLIPBOARD_STACK_LIMIT,
    };

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn enqueue_preserves_order_deduplicates_and_reports_the_limit() {
        let stack = ClipboardStack::new();
        let mut input = (0..CLIPBOARD_STACK_LIMIT)
            .map(|index| format!("item-{index}"))
            .collect::<Vec<_>>();
        input.push("item-0".to_owned());
        input.push("over-limit".to_owned());

        let result = stack.enqueue(&input);

        assert_eq!(result.added_count, CLIPBOARD_STACK_LIMIT);
        assert_eq!(result.duplicate_count, 1);
        assert_eq!(result.limit_skipped_count, 1);
        assert_eq!(result.snapshot.order[0].item_id, "item-0");
        assert_eq!(result.snapshot.order[19].item_id, "item-19");
    }

    #[test]
    fn collecting_adds_only_while_enabled() {
        let stack = ClipboardStack::new();
        assert!(stack.enqueue_captured("before").is_none());
        stack.set_collecting(true);
        assert_eq!(stack.enqueue_captured("captured").unwrap().added_count, 1);
        stack.set_collecting(false);
        assert!(stack.enqueue_captured("after").is_none());
        assert_eq!(stack.snapshot().order.len(), 1);
    }

    #[test]
    fn waiting_entries_can_move_but_completed_and_busy_entries_cannot() {
        let stack = ClipboardStack::new();
        stack.enqueue(&ids(&["a", "b", "c"]));
        stack.move_waiting(2, 0).unwrap();
        assert_eq!(stack.snapshot().order[0].item_id, "c");

        let reservation = stack.reserve_activation().unwrap();
        assert_eq!(stack.move_waiting(1, 2), Err(ClipboardStackError::Busy));
        stack.finish_activation(&reservation.token, true).unwrap();
        assert_eq!(
            stack.move_waiting(0, 2),
            Err(ClipboardStackError::CompletedEntry)
        );
    }

    #[test]
    fn success_advances_exactly_once_and_stale_tokens_do_nothing() {
        let stack = ClipboardStack::new();
        stack.enqueue(&ids(&["a", "b"]));
        let reservation = stack.reserve_activation().unwrap();

        assert_eq!(
            stack
                .finish_activation(&reservation.token, true)
                .unwrap()
                .cursor,
            1
        );
        assert_eq!(
            stack.finish_activation(&reservation.token, true),
            Err(ClipboardStackError::StaleReservation)
        );
        assert_eq!(stack.snapshot().cursor, 1);
    }

    #[test]
    fn failure_keeps_current_and_allows_retry() {
        let stack = ClipboardStack::new();
        stack.enqueue(&ids(&["a"]));
        let first = stack.reserve_activation().unwrap();
        assert_eq!(
            stack.finish_activation(&first.token, false).unwrap().cursor,
            0
        );
        let retry = stack.reserve_activation().unwrap();
        assert_eq!(retry.item_id, "a");
        assert_ne!(retry.token, first.token);
    }

    #[test]
    fn unavailable_current_is_never_substituted() {
        let stack = ClipboardStack::new();
        stack.enqueue(&ids(&["a", "b"]));
        let snapshot = stack.mark_unavailable("a");
        assert_eq!(
            snapshot.order[0].availability,
            ClipboardStackAvailability::Unavailable
        );
        assert_eq!(
            stack.reserve_activation(),
            Err(ClipboardStackError::Unavailable)
        );
        assert_eq!(stack.snapshot().cursor, 0);
    }

    #[test]
    fn reset_rejects_an_inflight_activation_then_clears_idle_state() {
        let stack = ClipboardStack::new();
        stack.enqueue(&ids(&["a"]));
        stack.set_collecting(true);
        let reservation = stack.reserve_activation().unwrap();

        assert_eq!(stack.reset(), Err(ClipboardStackError::Busy));
        stack.finish_activation(&reservation.token, false).unwrap();

        let reset = stack.reset().unwrap();
        assert!(reset.order.is_empty());
        assert_eq!(reset.cursor, 0);
        assert!(!reset.collecting);
        assert!(!reset.busy);
    }
}
