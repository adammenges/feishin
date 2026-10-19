//! Accessibility support, provided by [AccessKit][accesskit].
//!
//! There are user-facing guide-level docs [here](crate::_accessibility).
//!
//! ## Architecture
//!
//! ```text
//!                              ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           ┌─▶│ AccessKit Adapter (MacOS)      │◀─▶│ MacOS System APIs   │
//!                           │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//! ┌──────┐   ┌───────────┐  │  ┌────────────────────────────────┐   ┌─────────────────────┐
//! │ GPUI │◀─▶│ AccessKit │◀─┼─▶│ AccessKit Adapter (Windows)    │◀─▶│ Windows System APIs │
//! └──────┘   └───────────┘  │  └────────────────────────────────┘   └─────────────────────┘
//!                           │
//!                           │  ┌────────────────────────────────┐   ┌─────────────────────┐
//!                           └─▶│ AccessKit Adapter (Linux)      │◀─▶│ dbus                │
//!                              └────────────────────────────────┘   └─────────────────────┘
//! ```
//!
//! In order for GPUI apps to be usable for people using assistive technology,
//! we must do a few things:
//! - Inform the system when the UI changes meaningfully. This includes:
//!   - Reporting new/removed/changed UI elements
//!   - *Not* reporting irrelevant UI changes, e.g. an invisible `div()` being
//!     added.
//!   - Reporting the appearance and capabilities of each UI element. For example:
//!     - What does this piece of text say?
//!     - How far along is this progress bar?
//!     - Can this node be focused?
//!     - Can this node have a value directly assigned? (e.g. a slider)
//! - Allowing the system to interact with the UI by dispatching actions to
//!   nodes. Note that AccessKit has its own [`Action`] type, which is not the
//!   [`crate::Action`] trait.
//! - Activate and deactivate accessibility features when requested by the
//!   system.
//!
//! Activating and deactivating at the right time is trivial, so I won't go into
//! detail here. The other two are almost orthogonal in implementation.
//!
//! The state for both lives in the [`A11y`] struct in this module.
//!
//! ### Reporting UI changes
//!
//! Every frame, we build a [`TreeUpdate`] and send it to the platform-specific
//! adapter. A [`TreeUpdate`] is a representation of a subset of the UI tree.
//! When the adapter receives the update, it diffs it against the previous
//! update, and calls platform-specific APIs to inform screen readers about the
//! changes. Nodes may have been created, destroyed, or updated.
//!
//! Each node has an ID, and this ID *should* be stable across frames. If a
//! node's ID changes, then, from AccessKit's point of view, it is a different
//! node.
//!
//! We derive the node ID from the [`GlobalElementId`] in
//! [`GlobalElementId::accesskit_node_id`]. Nodes without [`GlobalElementId`]s
//! use a parent-scoped positional ID. Explicit IDs remain recommended for
//! interactive controls; ordinary text is included automatically.
//!
//! This all happens in [`Drawable::prepaint`]. The [`A11y`] struct maintains a
//! stack of nodes during prepainting, which we can use to calculate the
//! [`NodeId`]s, and record parent-child relationships. Once all [`Element`]s in
//! a frame have been prepainted, we send the resulting [`TreeUpdate`] object to
//! the adapter and the screen reader can announce the changes.
//!
//! ### Responding to actions
//!  
//! On adapter creation, we provide a callback to the adapter, which can be used
//! to dispatch actions. This callback forwards to [`A11y::action_listeners`], a
//! mapping from [`NodeId`]s to action handlers (basically just `Box<dyn
//! Fn()>`).
//!
//! This is populated in:
//! - [`Window::on_a11y_action`], which is called by:
//! - [`Interactivity::paint`], which is called by:
//! - [`InteractiveElement::on_a11y_action`], which is a public-facing API
//!
//! These are cleared at the start of a frame, and re-populated during painting.
//!
//! [`Element`]: crate::Element
//! [`GlobalElementId`]: crate::GlobalElementId
//! [`div()`]: crate::div
//! [`Interactivity::paint`]: crate::Interactivity::paint
//! [`InteractiveElement::on_a11y_action`]: crate::InteractiveElement::on_a11y_action
//! [`NodeId`]: accesskit::NodeId
//! [`Drawable::prepaint`]: crate::Drawable::prepaint

use crate::{App, Bounds, FocusId, Pixels, Window};
use accesskit::{Action, NodeId, TreeUpdate};
use collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// The fixed AccessKit node ID used for the root of every window's a11y tree.
pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);

/// A listener for an accessibility action on a specific node.
pub(crate) type A11yActionListener =
    Box<dyn FnMut(&Option<accesskit::ActionData>, &mut Window, &mut App) + 'static>;

/// Per-window accessibility state.
///
/// Manages the AccessKit tree that is built each frame and the mappings
/// needed to dispatch incoming action requests back to the right elements.
pub(crate) struct A11y {
    /// Whether a11y features have been requested by the system.
    ///
    /// Updated by AccessKit using callbacks provided to the adapter. Can change
    /// halfway through a frame.
    active_flag: Arc<AtomicBool>,
    /// Whether a11y features are active for *this specific frame*.
    ///
    /// At the start of each frame, we load [`Self::active_flag`] (using
    /// [`Self::sync_active_flag`]) and use this to determine whether we
    /// should construct a [`TreeUpdate`] for this frame. It's important that
    /// this value is stable within a frame, because the builder API exposed by
    /// this type maintains a stack of nodes and each must be pushed and popped
    /// exactly once.
    ///
    /// At the end of the frame, we re-call [`Self::sync_active_flag`] to
    /// determine whether we should actually send the finished [`TreeUpdate`].
    active_this_frame: bool,
    pub(crate) hidden_depth: usize,
    pub(crate) nodes: A11yNodeBuilder,
    pub(crate) focus_ids: FxHashMap<NodeId, FocusId>,
    pub(crate) node_bounds: FxHashMap<NodeId, Bounds<Pixels>>,
    pub(crate) action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
    pub(crate) node_actions: FxHashMap<NodeId, accesskit::Node>,
    pub(crate) text_runs: FxHashMap<NodeId, (NodeId, Vec<usize>)>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) last_update: Option<TreeUpdate>,
}

impl A11y {
    pub(crate) fn new(active_flag: Arc<AtomicBool>) -> Self {
        Self {
            active_flag,
            active_this_frame: false,
            hidden_depth: 0,
            nodes: A11yNodeBuilder::new(),
            focus_ids: FxHashMap::default(),
            node_bounds: FxHashMap::default(),
            action_listeners: FxHashMap::default(),
            node_actions: FxHashMap::default(),
            text_runs: FxHashMap::default(),
            #[cfg(any(test, feature = "test-support"))]
            last_update: None,
        }
    }

    /// Ensures that [`Self::is_active`] returns up to date information.
    ///
    /// See the docs for [`Self::active_flag`] and [`Self::active_this_frame`]
    /// for more commentary.
    pub(crate) fn sync_active_flag(&mut self) {
        self.active_this_frame = self.active_flag.load(Ordering::SeqCst);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active_this_frame && self.hidden_depth == 0
    }

    /// Clear per-frame state and push the root node to start a new frame.
    pub(crate) fn begin_frame(&mut self) {
        self.focus_ids.clear();
        self.node_bounds.clear();
        self.action_listeners.clear();
        self.node_actions.clear();
        self.text_runs.clear();
        self.nodes.begin_frame();
    }

    /// Finalize the tree and produce a [`TreeUpdate`] for the platform adapter.
    pub(crate) fn end_frame(&mut self) -> TreeUpdate {
        let update = self.nodes.finalize();
        #[cfg(any(test, feature = "test-support"))]
        {
            self.last_update = Some(update.clone());
        }
        update
    }

    /// Build the text document expected by native text and selection interfaces.
    pub(crate) fn prepare_text_runs(
        &mut self,
        parent: NodeId,
        node: &mut accesskit::Node,
        selection: Option<(usize, usize)>,
    ) -> Vec<(NodeId, accesskit::Node)> {
        use std::hash::{Hash, Hasher};
        use unicode_segmentation::UnicodeSegmentation;
        if !matches!(
            node.role(),
            accesskit::Role::TextInput
                | accesskit::Role::MultilineTextInput
                | accesskit::Role::PasswordInput
        ) {
            return Vec::new();
        }
        let text = node.value().unwrap_or("").to_owned();
        let mut lines = text.split_inclusive('\n').collect::<Vec<_>>();
        if lines.is_empty() || text.ends_with('\n') {
            lines.push("");
        }
        let mut runs = Vec::with_capacity(lines.len());
        let mut byte_start = 0;
        for (line_index, line) in lines.into_iter().enumerate() {
            let mut units = Vec::new();
            let mut characters = line.char_indices().peekable();
            while let Some((offset, character)) = characters.next() {
                let length = if character == '\r'
                    && characters.peek().is_some_and(|(_, next)| *next == '\n')
                {
                    characters.next();
                    2
                } else {
                    character.len_utf8()
                };
                units.push((offset, length as u8));
            }
            let chunks = if units.is_empty() {
                vec![&units[..]]
            } else {
                units.chunks(255).collect::<Vec<_>>()
            };
            let mut previous_on_line: Option<NodeId> = None;
            for (chunk_index, chunk) in chunks.into_iter().enumerate() {
                let mut hasher = std::hash::DefaultHasher::default();
                ("gpui-input-text-run", parent, line_index, chunk_index).hash(&mut hasher);
                let id = NodeId(hasher.finish());
                let start = chunk.first().map_or(0, |(offset, _)| *offset);
                let end = chunk
                    .last()
                    .map_or(0, |(offset, length)| offset + *length as usize);
                let mut offsets = chunk
                    .iter()
                    .map(|(offset, _)| byte_start + offset)
                    .collect::<Vec<_>>();
                offsets.push(byte_start + end);
                let mut run = accesskit::Node::new(accesskit::Role::TextRun);
                run.set_value(line[start..end].to_owned());
                run.set_character_lengths(
                    chunk.iter().map(|(_, length)| *length).collect::<Vec<_>>(),
                );
                run.set_word_starts(
                    line.unicode_word_indices()
                        .filter_map(|(offset, _)| {
                            offsets
                                .binary_search(&(byte_start + offset))
                                .ok()
                                .filter(|index| *index < chunk.len())
                                .and_then(|index| u8::try_from(index).ok())
                        })
                        .collect::<Vec<_>>(),
                );
                if let Some(previous) = previous_on_line {
                    run.set_previous_on_line(previous);
                    let (_, previous_run): &mut (NodeId, accesskit::Node) =
                        runs.last_mut().unwrap();
                    previous_run.set_next_on_line(id);
                }
                previous_on_line = Some(id);
                self.text_runs.insert(id, (parent, offsets));
                runs.push((id, run));
            }
            byte_start += line.len();
        }
        if let Some((anchor, focus)) = selection {
            let position = |byte: usize| {
                let byte = byte.min(text.len());
                for (index, (id, _)) in runs.iter().enumerate() {
                    let offsets = &self.text_runs[id].1;
                    if byte < *offsets.last().unwrap() || index + 1 == runs.len() {
                        return accesskit::TextPosition {
                            node: *id,
                            character_index: offsets
                                .partition_point(|offset| *offset < byte)
                                .min(offsets.len() - 1),
                        };
                    }
                }
                unreachable!("an input always has at least one text run")
            };
            node.set_text_selection(accesskit::TextSelection {
                anchor: position(anchor),
                focus: position(focus),
            });
        }
        runs
    }

    pub(crate) fn selection_offsets(
        &self,
        selection: &accesskit::TextSelection,
    ) -> Option<(NodeId, usize, usize)> {
        let (anchor_parent, anchor) = self.text_runs.get(&selection.anchor.node)?;
        let (focus_parent, focus) = self.text_runs.get(&selection.focus.node)?;
        if anchor_parent != focus_parent {
            return None;
        }
        Some((
            *anchor_parent,
            *anchor.get(selection.anchor.character_index)?,
            *focus.get(selection.focus.character_index)?,
        ))
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn activate_for_test(&mut self) {
        self.active_flag.store(true, Ordering::SeqCst);
    }
}

pub(crate) struct A11yNodeBuilder {
    ids_stack: SmallVec<[NodeId; 16]>,
    nodes_stack: SmallVec<[accesskit::Node; 16]>,
    /// This is the exact type required by accesskit, so we can't just make it a
    /// `HashMap<NodeId, Node>` to remove the need for `seen_ids`
    all_nodes: Vec<(NodeId, accesskit::Node)>,
    seen_ids: FxHashSet<NodeId>,
    focus: NodeId,
    anonymous_counts: FxHashMap<NodeId, u64>,
}

impl A11yNodeBuilder {
    fn new() -> Self {
        Self {
            ids_stack: SmallVec::new(),
            nodes_stack: SmallVec::new(),
            all_nodes: Vec::new(),
            seen_ids: FxHashSet::default(),
            focus: ROOT_NODE_ID,
            anonymous_counts: FxHashMap::default(),
        }
    }

    /// Push a new node onto the stack. It becomes a child of the current
    /// top-of-stack node.
    ///
    /// Returns `true` if the node was successfully pushed.
    pub(crate) fn push(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        debug_assert!(!self.ids_stack.is_empty(), "push called before push_root");

        if !self.seen_ids.insert(id) {
            debug_assert!(
                false,
                "Duplicate a11y node id: {id:?}. In a release build, this node would be silently discarded from the a11y tree."
            );
            // We need to return `false` here because inserting a duplicate
            // node will cause a panic in accesskit
            return false;
        }

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.ids_stack.push(id);
        self.nodes_stack.push(node);
        true
    }

    /// Anonymous labels are stable by position within their nearest accessible parent.
    pub(crate) fn anonymous_id(&mut self) -> NodeId {
        use std::hash::{Hash, Hasher};
        let parent = self.ids_stack.last().copied().unwrap_or(ROOT_NODE_ID);
        let index = self.anonymous_counts.entry(parent).or_default();
        let mut hasher = std::hash::DefaultHasher::default();
        ("gpui-anonymous-label", parent, *index).hash(&mut hasher);
        *index += 1;
        NodeId(hasher.finish())
    }

    /// Pop the current node off the stack and finalize it into the all_nodes
    /// list.
    pub(crate) fn pop(&mut self) {
        debug_assert!(self.ids_stack.len() > 1, "pop would remove the root node");

        if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
            self.all_nodes.push((id, node));
        }
    }

    /// Push the root node to start a new frame.
    fn begin_frame(&mut self) {
        self.all_nodes.clear();
        self.ids_stack.clear();
        self.nodes_stack.clear();
        self.seen_ids.clear();
        self.anonymous_counts.clear();
        let root_node = accesskit::Node::new(accesskit::Role::Window);

        self.ids_stack.push(ROOT_NODE_ID);
        self.nodes_stack.push(root_node);
        self.focus = ROOT_NODE_ID;
    }

    /// Set the focused node for this frame.
    pub(crate) fn set_focus(&mut self, id: NodeId) {
        if self.seen_ids.contains(&id) {
            self.focus = id;
        }
    }

    fn finalize(&mut self) -> TreeUpdate {
        // Stack should contain only the root node
        debug_assert_eq!(self.ids_stack.len(), 1);
        debug_assert_eq!(self.ids_stack[0], ROOT_NODE_ID);

        // Pop remaining nodes (should just be the root).
        while !self.ids_stack.is_empty() {
            if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
                self.all_nodes.push((id, node));
            }
        }

        let nodes = std::mem::take(&mut self.all_nodes);
        TreeUpdate {
            nodes,
            tree: Some(accesskit::Tree::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: self.focus,
        }
    }
}
