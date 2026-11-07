//!
//! The stack is generic over the
//! context its actions edit (`C`, the scene graph for now) and every replaying call takes that context explicitly, so
//! actions get `&mut C` instead of a captured pointer.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

/// Buffers an undo action shares with the scene or with other steps: each key counts once across the whole stack.
#[derive(Default, Debug, Clone)]
pub struct UndoShared {
    pub items: Vec<(usize, usize)>,
}

impl UndoShared {
    /// `key` identifies the shared buffer (its address); 0 is ignored.
    pub fn add(&mut self, key: usize, bytes: usize) {
        if key != 0 {
            self.items.push((key, bytes));
        }
    }
}

/// One reversible primitive edit. Subsystems record these; commands never write their own undo.
pub trait UndoAction<C> {
    fn undo(&mut self, ctx: &mut C);
    fn redo(&mut self, ctx: &mut C);
    /// True for selection changes, which do not make the scene unsaved.
    fn selection_only(&self) -> bool {
        false
    }
    /// Approximate bytes this action owns outright, for the undo memory budget.
    fn bytes(&self) -> usize {
        64
    }
    /// Shared buffers (meshes, data payloads) this action keeps alive.
    fn shared_bytes(&self, _shared: &mut UndoShared) {}
}

type ActionRc<C> = Rc<RefCell<dyn UndoAction<C>>>;

/// An action from a pair of closures.
pub struct FnAction<C> {
    undo: Box<dyn FnMut(&mut C)>,
    redo: Box<dyn FnMut(&mut C)>,
    selection: bool,
    bytes: usize,
    shared: UndoShared,
}

impl<C> FnAction<C> {
    pub fn new(undo: impl FnMut(&mut C) + 'static, redo: impl FnMut(&mut C) + 'static) -> Self {
        Self {
            undo: Box::new(undo),
            redo: Box::new(redo),
            selection: false,
            bytes: 64,
            shared: UndoShared::default(),
        }
    }
    pub fn selection(mut self, on: bool) -> Self {
        self.selection = on;
        self
    }
    /// Bytes owned and shared buffers held, for the memory budget.
    pub fn sized(mut self, bytes: usize, shared: UndoShared) -> Self {
        self.bytes = bytes;
        self.shared = shared;
        self
    }
}

impl<C> UndoAction<C> for FnAction<C> {
    fn undo(&mut self, ctx: &mut C) {
        (self.undo)(ctx)
    }
    fn redo(&mut self, ctx: &mut C) {
        (self.redo)(ctx)
    }
    fn selection_only(&self) -> bool {
        self.selection
    }
    fn bytes(&self) -> usize {
        self.bytes
    }
    fn shared_bytes(&self, shared: &mut UndoShared) {
        shared.items.extend_from_slice(&self.shared.items);
    }
}

/// Reverts a step's own edits on redo and reapplies them on undo; shares them with that step.
struct RevertAction<C> {
    edits: Vec<ActionRc<C>>,
}

impl<C> UndoAction<C> for RevertAction<C> {
    fn undo(&mut self, ctx: &mut C) {
        for a in &self.edits {
            a.borrow_mut().redo(ctx);
        }
    }
    fn redo(&mut self, ctx: &mut C) {
        for a in self.edits.iter().rev() {
            a.borrow_mut().undo(ctx);
        }
    }
    fn selection_only(&self) -> bool {
        self.edits.iter().all(|a| a.borrow().selection_only())
    }
    fn shared_bytes(&self, s: &mut UndoShared) {
        for a in &self.edits {
            a.borrow().shared_bytes(s);
        }
    }
}

struct Chunk<C> {
    label: String,
    actions: Vec<ActionRc<C>>,
    /// Leading actions that revert the step below (an amend); the rest are this step's own edits.
    amends: usize,
    id: u64,
    selection_only: bool,
    owned: usize,
    shared: UndoShared,
}

impl<C> Chunk<C> {
    fn new(label: String) -> Self {
        Self {
            label,
            actions: Vec::new(),
            amends: 0,
            id: 0,
            selection_only: false,
            owned: 0,
            shared: UndoShared::default(),
        }
    }
}

#[derive(Default)]
struct SharedEntry {
    bytes: usize,
    refs: usize,
}

/// What a listener is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoEvent {
    /// A step was pushed, dropped, or the stack cleared or trimmed.
    Changed,
    Undo(String),
    Redo(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub selection_only: bool,
}

/// Main thread only.
pub struct UndoStack<C> {
    done: VecDeque<Chunk<C>>,
    undone: Vec<Chunk<C>>,
    open: Chunk<C>,
    depth: i32,
    enabled: bool,
    replaying: bool,
    reopened: bool,
    limit: usize,
    budget: usize,
    owned_bytes: usize,
    shared_bytes: usize,
    shared: HashMap<usize, SharedEntry>,
    quiet: bool,
    serial: u64,
    next_id: u64,
    base_id: u64,
    listeners: Vec<Box<dyn FnMut(&UndoEvent)>>,
}

impl<C: 'static> Default for UndoStack<C> {
    fn default() -> Self {
        Self {
            done: VecDeque::new(),
            undone: Vec::new(),
            open: Chunk::new(String::new()),
            depth: 0,
            enabled: true,
            replaying: false,
            reopened: false,
            limit: 0,
            budget: 0,
            owned_bytes: 0,
            shared_bytes: 0,
            shared: HashMap::new(),
            quiet: false,
            serial: 0,
            next_id: 1,
            base_id: 0,
            listeners: Vec::new(),
        }
    }
}

impl<C: 'static> UndoStack<C> {
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty stack that reports replaying and records nothing: what an owner leaves in place while its real stack
    /// is out replaying against it.
    pub fn replaying_placeholder() -> Self {
        Self {
            enabled: false,
            replaying: true,
            ..Self::default()
        }
    }

    /// Chunks nest; only the outermost one lands on the stack.
    pub fn open_chunk(&mut self, label: impl Into<String>) {
        if self.depth == 0 {
            self.open = Chunk::new(label.into());
        }
        self.depth += 1;
    }

    pub fn close_chunk(&mut self) {
        if self.depth == 0 {
            return;
        }
        self.depth -= 1;
        if self.depth > 0 {
            return;
        }
        if self.open.actions.is_empty() {
            // A reopened step that now records nothing is gone from the stack.
            if std::mem::take(&mut self.reopened) {
                self.serial += 1;
                self.notify(UndoEvent::Changed);
            }
            return;
        }
        self.reopened = false;
        let c = std::mem::replace(&mut self.open, Chunk::new(String::new()));
        self.push(c);
    }

    /// Closes the outermost chunk, reverting and dropping everything recorded in it.
    pub fn cancel_chunk(&mut self, ctx: &mut C) {
        if self.depth == 0 {
            return;
        }
        self.depth = 0;
        self.reopened = false;
        let c = std::mem::replace(&mut self.open, Chunk::new(String::new()));
        self.replaying = true;
        for a in c.actions.iter().rev() {
            a.borrow_mut().undo(ctx);
        }
        self.replaying = false;
        self.serial += 1;
        self.notify(UndoEvent::Changed);
    }

    /// Edits recorded so far in the open chunk; 0 when none is open. A mark for `rollback_to`.
    pub fn open_edits(&self) -> usize {
        if self.depth > 0 {
            self.open.actions.len()
        } else {
            0
        }
    }

    /// Reverts and drops the open chunk's edits after the first `mark`, leaving the chunk and the edits before it open.
    pub fn rollback_to(&mut self, mark: usize, ctx: &mut C) {
        if self.depth == 0 || mark >= self.open.actions.len() {
            return;
        }
        self.replaying = true;
        for i in (mark..self.open.actions.len()).rev() {
            self.open.actions[i].borrow_mut().undo(ctx);
        }
        self.replaying = false;
        self.open.actions.truncate(mark);
    }

    /// Opens a chunk that amends the newest step: that step's own edits are reverted now, again whenever the new chunk
    /// is redone, and come back when it is undone. False if a chunk is open or the stack is empty.
    pub fn open_amend(&mut self, label: impl Into<String>, ctx: &mut C) -> bool {
        let Some(newest) = self.done.back() else {
            return false;
        };
        if self.depth > 0 {
            return false;
        }
        let edits: Vec<ActionRc<C>> = newest.actions[newest.amends..].to_vec();
        let mut revert = RevertAction { edits };
        self.replaying = true;
        revert.redo(ctx);
        self.replaying = false;
        self.open_chunk(label);
        self.open.actions.push(Rc::new(RefCell::new(revert)));
        self.open.amends = 1;
        true
    }

    /// Takes the newest step back off the stack to record it again: its own edits are reverted and dropped, what it
    /// amends is kept, and it is reopened as the open chunk under `label`. False if a chunk is open or the stack is empty.
    pub fn reopen_newest(&mut self, label: impl Into<String>, ctx: &mut C) -> bool {
        if self.depth > 0 || self.done.is_empty() {
            return false;
        }
        let mut c = self.done.pop_back().unwrap();
        self.account(&c, false);
        self.replaying = true;
        for i in (c.amends..c.actions.len()).rev() {
            c.actions[i].borrow_mut().undo(ctx);
        }
        self.replaying = false;
        c.actions.truncate(c.amends);
        let amends = c.amends;
        self.open = Chunk::new(label.into());
        self.open.actions = c.actions;
        self.open.amends = amends;
        self.depth = 1;
        self.reopened = true;
        true
    }

    /// Dropped silently while undoing/redoing or when recording is off.
    pub fn record(&mut self, action: impl UndoAction<C> + 'static) {
        self.record_rc(Rc::new(RefCell::new(action)));
    }

    pub fn record_boxed(&mut self, action: Box<dyn UndoAction<C>>) {
        struct Boxed<C>(Box<dyn UndoAction<C>>);
        impl<C> UndoAction<C> for Boxed<C> {
            fn undo(&mut self, ctx: &mut C) {
                self.0.undo(ctx)
            }
            fn redo(&mut self, ctx: &mut C) {
                self.0.redo(ctx)
            }
            fn selection_only(&self) -> bool {
                self.0.selection_only()
            }
            fn bytes(&self) -> usize {
                self.0.bytes()
            }
            fn shared_bytes(&self, s: &mut UndoShared) {
                self.0.shared_bytes(s)
            }
        }
        if !self.enabled || self.replaying {
            return;
        }
        self.record_rc(Rc::new(RefCell::new(Boxed(action))));
    }

    fn record_rc(&mut self, action: ActionRc<C>) {
        if !self.enabled || self.replaying {
            return;
        }
        if self.depth > 0 {
            self.open.actions.push(action);
            return;
        }
        // Edit made outside any command: give it a chunk of its own.
        let mut c = Chunk::new(String::new());
        c.actions.push(action);
        self.push(c);
    }

    pub fn undo(&mut self, ctx: &mut C) -> bool {
        if self.done.is_empty() || self.depth > 0 {
            return false;
        }
        let c = self.done.pop_back().unwrap();
        self.replaying = true;
        for a in c.actions.iter().rev() {
            a.borrow_mut().undo(ctx);
        }
        self.replaying = false;
        let label = c.label.clone();
        self.undone.push(c);
        self.serial += 1;
        self.notify(UndoEvent::Changed);
        self.notify(UndoEvent::Undo(label));
        true
    }

    pub fn redo(&mut self, ctx: &mut C) -> bool {
        if self.undone.is_empty() || self.depth > 0 {
            return false;
        }
        let c = self.undone.pop().unwrap();
        self.replaying = true;
        for a in &c.actions {
            a.borrow_mut().redo(ctx);
        }
        self.replaying = false;
        let label = c.label.clone();
        self.done.push_back(c);
        self.serial += 1;
        self.notify(UndoEvent::Changed);
        self.notify(UndoEvent::Redo(label));
        true
    }

    pub fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
        self.reset_accounting();
        self.base_id = self.next_id;
        self.next_id += 1;
        self.serial += 1;
        self.notify(UndoEvent::Changed);
    }

    /// Drops all undo/redo steps but keeps the scene state id, so an unchanged scene stays saved.
    pub fn flush(&mut self) {
        self.base_id = self.state_id();
        self.done.clear();
        self.undone.clear();
        self.reset_accounting();
        self.serial += 1;
        self.notify(UndoEvent::Changed);
    }

    /// Drops the steps undo would revert, keeping the redo steps and the scene state id.
    pub fn flush_past(&mut self) {
        self.base_id = self.state_id();
        let done = std::mem::take(&mut self.done);
        for c in &done {
            self.account(c, false);
        }
        self.serial += 1;
        self.notify(UndoEvent::Changed);
    }

    /// Drops the steps redo would replay.
    pub fn flush_future(&mut self) {
        let undone = std::mem::take(&mut self.undone);
        for c in &undone {
            self.account(c, false);
        }
        self.serial += 1;
        self.notify(UndoEvent::Changed);
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
    pub fn undo_label(&self) -> String {
        self.done
            .back()
            .map(|c| c.label.clone())
            .unwrap_or_default()
    }
    pub fn redo_label(&self) -> String {
        self.undone
            .last()
            .map(|c| c.label.clone())
            .unwrap_or_default()
    }
    /// Identifies the newest undo step for as long as it exists, through undo and redo; 0 when there is none.
    pub fn newest_id(&self) -> u64 {
        self.done.back().map(|c| c.id).unwrap_or(0)
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn replaying(&self) -> bool {
        self.replaying
    }
    /// True while a chunk is open: edits are still being gathered into the next step.
    pub fn chunk_open(&self) -> bool {
        self.depth > 0
    }
    pub fn chunk_depth(&self) -> i32 {
        self.depth
    }

    /// 0 keeps every step.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        let before = self.done.len();
        self.trim();
        if self.done.len() != before {
            self.serial += 1;
            self.notify(UndoEvent::Changed);
        }
    }
    pub fn limit(&self) -> usize {
        self.limit
    }
    /// Oldest steps drop while the stack holds more than this many bytes; 0 is unlimited.
    pub fn set_byte_budget(&mut self, bytes: usize) {
        self.budget = bytes;
        let before = self.done.len();
        self.trim();
        if self.done.len() != before {
            self.serial += 1;
            self.notify(UndoEvent::Changed);
        }
    }
    pub fn byte_budget(&self) -> usize {
        self.budget
    }
    /// Approximate bytes held by every undo and redo step, shared buffers counted once.
    pub fn memory_bytes(&self) -> usize {
        self.owned_bytes + self.shared_bytes
    }

    /// Undo steps oldest first, then redo steps in the order redo would replay them.
    pub fn steps(&self) -> Vec<Step> {
        self.done
            .iter()
            .chain(self.undone.iter().rev())
            .map(|c| Step {
                label: c.label.clone(),
                selection_only: c.selection_only,
            })
            .collect()
    }
    /// Steps undo can go back through; `steps()[undo_count() - 1]` is the current one.
    pub fn undo_count(&self) -> usize {
        self.done.len()
    }
    pub fn redo_count(&self) -> usize {
        self.undone.len()
    }

    /// Undoes or redoes until `undo_count() == position`, notifying listeners once. False if out of range.
    pub fn jump_to(&mut self, position: usize, ctx: &mut C) -> bool {
        if self.depth > 0 || position > self.done.len() + self.undone.len() {
            return false;
        }
        if position == self.done.len() {
            return true;
        }
        self.quiet = true;
        while self.done.len() > position {
            self.undo(ctx);
        }
        while self.done.len() < position {
            self.redo(ctx);
        }
        self.quiet = false;
        self.notify(UndoEvent::Changed);
        true
    }

    /// Changes whenever the stack does, so callers can tell if theirs is still on top.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Identifies the scene state the stack sits at: equal ids mean undo/redo brought the scene back to the same state.
    pub fn state_id(&self) -> u64 {
        self.done
            .iter()
            .rev()
            .find(|c| !c.selection_only)
            .map(|c| c.id)
            .unwrap_or(self.base_id)
    }

    /// Called after a step is pushed, undone, redone or the stack cleared.
    pub fn add_listener(&mut self, f: impl FnMut(&UndoEvent) + 'static) {
        self.listeners.push(Box::new(f));
    }

    /// Moves listeners from another stack (the placeholder an owner swapped in while replaying) onto this one.
    pub fn take_listeners_from(&mut self, other: &mut UndoStack<C>) {
        self.listeners.append(&mut other.listeners);
    }

    fn push(&mut self, mut c: Chunk<C>) {
        c.id = self.next_id;
        self.next_id += 1;
        c.selection_only = c.actions.iter().all(|a| a.borrow().selection_only());
        for a in &c.actions {
            let a = a.borrow();
            c.owned += a.bytes();
            a.shared_bytes(&mut c.shared);
        }
        let undone = std::mem::take(&mut self.undone);
        for u in &undone {
            self.account(u, false);
        }
        self.account(&c, true);
        self.done.push_back(c);
        self.serial += 1;
        self.trim();
        self.notify(UndoEvent::Changed);
    }

    fn trim(&mut self) {
        // The newest step always stays, however large, so the last edit can be undone.
        while !self.done.is_empty()
            && ((self.limit != 0 && self.done.len() > self.limit)
                || (self.budget != 0 && self.memory_bytes() > self.budget && self.done.len() > 1))
        {
            let front = self.done.pop_front().unwrap();
            if !front.selection_only {
                self.base_id = front.id;
            }
            self.account(&front, false);
        }
    }

    fn account(&mut self, c: &Chunk<C>, add: bool) {
        if add {
            self.owned_bytes += c.owned;
        } else {
            self.owned_bytes -= c.owned;
        }
        for &(key, bytes) in &c.shared.items {
            if add {
                let e = self.shared.entry(key).or_default();
                if e.refs == 0 {
                    e.bytes = bytes;
                    self.shared_bytes += bytes;
                }
                e.refs += 1;
                continue;
            }
            if let Some(e) = self.shared.get_mut(&key) {
                e.refs -= 1;
                if e.refs == 0 {
                    self.shared_bytes -= e.bytes;
                    self.shared.remove(&key);
                }
            }
        }
    }

    fn reset_accounting(&mut self) {
        self.shared.clear();
        self.owned_bytes = 0;
        self.shared_bytes = 0;
    }

    fn notify(&mut self, e: UndoEvent) {
        if self.quiet && e == UndoEvent::Changed {
            return;
        }
        for f in &mut self.listeners {
            f(&e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Doc {
        v: Vec<i32>,
    }

    fn push(stack: &mut UndoStack<Doc>, doc: &mut Doc, x: i32) {
        doc.v.push(x);
        stack.record(FnAction::new(
            |d: &mut Doc| {
                d.v.pop();
            },
            move |d: &mut Doc| d.v.push(x),
        ));
    }

    #[test]
    fn chunks_undo_and_redo_as_one() {
        let mut s = UndoStack::new();
        let mut d = Doc::default();
        s.open_chunk("two");
        push(&mut s, &mut d, 1);
        push(&mut s, &mut d, 2);
        s.close_chunk();
        push(&mut s, &mut d, 3);
        assert_eq!(s.undo_count(), 2);
        assert!(s.undo(&mut d));
        assert_eq!(d.v, vec![1, 2]);
        assert!(s.undo(&mut d));
        assert!(d.v.is_empty());
        assert!(s.redo(&mut d));
        assert_eq!(d.v, vec![1, 2]);
        assert_eq!(s.redo_label(), "");
    }

    #[test]
    fn cancel_and_rollback() {
        let mut s = UndoStack::new();
        let mut d = Doc::default();
        s.open_chunk("c");
        push(&mut s, &mut d, 1);
        let mark = s.open_edits();
        push(&mut s, &mut d, 2);
        s.rollback_to(mark, &mut d);
        assert_eq!(d.v, vec![1]);
        s.cancel_chunk(&mut d);
        assert!(d.v.is_empty());
        assert!(!s.can_undo());
    }

    #[test]
    fn amend_replaces_newest_step() {
        let mut s = UndoStack::new();
        let mut d = Doc::default();
        s.open_chunk("first");
        push(&mut s, &mut d, 1);
        s.close_chunk();
        assert!(s.open_amend("again", &mut d));
        assert!(d.v.is_empty());
        push(&mut s, &mut d, 5);
        s.close_chunk();
        assert_eq!(d.v, vec![5]);
        s.undo(&mut d);
        assert_eq!(d.v, vec![1]);
        s.redo(&mut d);
        assert_eq!(d.v, vec![5]);
    }

    #[test]
    fn state_id_returns_after_undo_redo() {
        let mut s = UndoStack::new();
        let mut d = Doc::default();
        let clean = s.state_id();
        push(&mut s, &mut d, 1);
        assert_ne!(s.state_id(), clean);
        s.undo(&mut d);
        assert_eq!(s.state_id(), clean);
    }

    #[test]
    fn shared_bytes_count_once() {
        let mut s: UndoStack<Doc> = UndoStack::new();
        let mut sh = UndoShared::default();
        sh.add(0x1000, 500);
        s.record(FnAction::new(|_: &mut Doc| {}, |_: &mut Doc| {}).sized(10, sh.clone()));
        s.record(FnAction::new(|_: &mut Doc| {}, |_: &mut Doc| {}).sized(10, sh));
        assert_eq!(s.memory_bytes(), 520);
        s.set_byte_budget(515);
        assert_eq!(s.undo_count(), 1);
        assert_eq!(s.memory_bytes(), 510);
    }
}
