use crate::model::Item;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub key: Uuid,
    pub item: Item,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Clone, Copy)]
pub enum Enqueue {
    Now,
    Next,
    End,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlayBehavior {
    #[default]
    Now,
    Next,
    Last,
    Shuffle,
}
impl PlayBehavior {
    pub fn mode(self) -> Enqueue {
        match self {
            Self::Next => Enqueue::Next,
            Self::Last => Enqueue::End,
            _ => Enqueue::Now,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Queue {
    pub entries: Vec<Entry>,
    pub order: Vec<Uuid>,
    pub current: Option<Uuid>,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub position: f64,
    #[serde(default)]
    pub up_next: Vec<Uuid>,
    /// Runtime copy of the setting; old saved queues opt in automatically.
    #[serde(skip, default = "up_next_default")]
    pub up_next_enabled: bool,
}

fn up_next_default() -> bool {
    true
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            entries: vec![],
            order: vec![],
            current: None,
            shuffle: false,
            repeat: Repeat::Off,
            position: 0.,
            up_next: vec![],
            up_next_enabled: true,
        }
    }
}

#[derive(Default)]
pub struct AutoSave {
    previous: Option<Uuid>,
    count: u32,
}

impl AutoSave {
    /// Match the original counter: save after `interval` tracks when the next entry starts.
    pub fn observe(&mut self, key: Option<Uuid>, interval: u32) -> bool {
        if key == self.previous {
            return false;
        }
        self.previous = key;
        let save = self.count >= interval.max(1);
        self.count = if save {
            1
        } else {
            self.count.saturating_add(1)
        };
        save
    }
}

impl Queue {
    /// Keep the pending block after the playing track in both playback and base order.
    pub fn sync_up_next(&mut self) {
        self.arrange_up_next(true);
    }

    fn arrange_up_next(&mut self, keep_pinned: bool) {
        if !self.up_next_enabled {
            self.up_next.clear();
        }
        if self.up_next.is_empty() {
            return;
        }
        let valid = self
            .order
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::new();
        self.up_next
            .retain(|key| Some(*key) != self.current && valid.contains(key) && seen.insert(*key));
        if self.up_next.is_empty() {
            return;
        }
        let settle = |ids: &mut Vec<Uuid>| {
            let current = self
                .current
                .and_then(|key| ids.iter().position(|id| *id == key));
            let start = ids.iter().position(|id| *id == self.up_next[0]);
            // Normal songs deliberately dropped above the block stay there.
            if keep_pinned
                && start.is_some_and(|start| {
                    current.is_none_or(|current| start > current)
                        && ids.get(start..start + self.up_next.len())
                            == Some(self.up_next.as_slice())
                })
            {
                return false;
            }
            ids.retain(|id| !seen.contains(id));
            let index = self
                .current
                .and_then(|key| ids.iter().position(|id| *id == key))
                .map_or(0, |i| i + 1);
            ids.splice(index..index, self.up_next.iter().copied());
            true
        };
        settle(&mut self.order);
        let mut base = self
            .entries
            .iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        if !settle(&mut base) {
            return;
        }
        let indexes = base
            .into_iter()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect::<std::collections::HashMap<_, _>>();
        self.entries.sort_by_key(|entry| indexes[&entry.key]);
    }

    /// The lower edge of a normal row above Up next belongs to the normal queue.
    pub fn pins_above_up_next(&self, target: Option<Uuid>, after: bool) -> bool {
        if !after {
            return false;
        }
        let index = |key| self.order.iter().position(|id| Some(*id) == key);
        match (
            index(self.current),
            index(target),
            index(self.up_next.first().copied()),
        ) {
            (Some(current), Some(target), Some(start)) => current <= target && target < start,
            _ => false,
        }
    }

    fn regroup_up_next(&mut self, moved: &[Uuid], pin: bool) {
        if self.up_next.is_empty() {
            return;
        }
        let moved = moved
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let kept = self
            .up_next
            .iter()
            .filter(|id| !moved.contains(id))
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let last_kept = self.order.iter().rposition(|id| kept.contains(id));
        let mut previous = false;
        self.up_next = self
            .order
            .iter()
            .enumerate()
            .filter_map(|(index, id)| {
                if Some(*id) == self.current {
                    previous = true;
                    return None;
                }
                if kept.contains(id) {
                    previous = true;
                    return Some(*id);
                }
                if moved.contains(id) {
                    return (previous && !pin).then_some(*id);
                }
                previous &= last_kept.is_some_and(|last| index < last);
                None
            })
            .collect();
        self.sync_up_next();
    }

    pub fn current(&self) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| Some(entry.key) == self.current)
    }

    pub fn ordered(&self) -> Vec<&Entry> {
        let entries = self
            .entries
            .iter()
            .map(|entry| (entry.key, entry))
            .collect::<std::collections::HashMap<_, _>>();
        self.order
            .iter()
            .filter_map(|key| entries.get(key).copied())
            .collect()
    }

    pub fn add(&mut self, items: Vec<Item>, mode: Enqueue) {
        self.sync_up_next();
        let priority =
            matches!(mode, Enqueue::Next) && self.up_next_enabled && self.current.is_some();
        if matches!(mode, Enqueue::Now) {
            let pending = self
                .up_next
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            self.entries.retain(|entry| pending.contains(&entry.key));
            self.order.clear();
            self.current = None;
            self.position = 0.0;
        }
        let entries: Vec<_> = items
            .into_iter()
            .map(|item| Entry {
                key: Uuid::new_v4(),
                item,
            })
            .collect();
        let mut keys: Vec<_> = entries.iter().map(|entry| entry.key).collect();
        if self.shuffle && !priority {
            keys.shuffle(&mut rand::thread_rng());
        }
        let first_added = keys.first().copied();
        let index = if matches!(mode, Enqueue::Next) {
            self.up_next
                .last()
                .copied()
                .filter(|_| priority)
                .or(self.current)
                .and_then(|key| self.order.iter().position(|id| *id == key))
                .map_or(self.order.len(), |i| i + 1)
        } else {
            self.order.len()
        };
        if priority {
            self.up_next.extend(&keys);
        }
        self.order.splice(index..index, keys);
        if matches!(mode, Enqueue::Now) {
            let index = usize::from(first_added.is_some());
            self.order
                .splice(index..index, self.up_next.iter().copied());
        }
        self.entries.extend(entries);
        if self.current.is_none() {
            self.current = first_added;
        }
        self.sync_up_next();
    }

    pub fn next_entry(&self, automatic: bool) -> Option<&Entry> {
        if automatic && self.repeat == Repeat::One && self.current.is_some() {
            return self.current();
        }
        let index = self
            .current
            .and_then(|key| self.order.iter().position(|id| *id == key));
        let next = match index {
            Some(i) if i + 1 < self.order.len() => Some(self.order[i + 1]),
            Some(_) if self.repeat == Repeat::All => self.order.first().copied(),
            None => self.order.first().copied(),
            _ => None,
        };
        self.entries.iter().find(|entry| Some(entry.key) == next)
    }
    pub fn next(&mut self, automatic: bool) -> Option<&Entry> {
        self.current = self.next_entry(automatic).map(|entry| entry.key);
        self.position = 0.0;
        self.sync_up_next();
        self.current()
    }

    pub fn previous(&mut self) -> Option<&Entry> {
        if self.position > 3.0 {
            self.position = 0.0;
            return self.current();
        }
        let index = self
            .current
            .and_then(|key| self.order.iter().position(|id| *id == key));
        self.current = match index {
            Some(i) if i > 0 => Some(self.order[i - 1]),
            Some(_) if self.repeat == Repeat::All => self.order.last().copied(),
            _ => self.order.first().copied(),
        };
        self.position = 0.0;
        self.sync_up_next();
        self.current()
    }

    /// Skip contiguous tracks belonging to the same album and server.
    pub fn jump_album(&mut self, forward: bool) -> Option<&Entry> {
        let ordered = self.ordered();
        let index = ordered
            .iter()
            .position(|entry| Some(entry.key) == self.current)?;
        let same = |a: &Entry, b: &Entry| {
            a.item.server_id == b.item.server_id
                && if !a.item.album_id.is_empty() && !b.item.album_id.is_empty() {
                    a.item.album_id == b.item.album_id
                } else {
                    a.item.album == b.item.album && a.item.artist == b.item.artist
                }
        };
        let target = if forward {
            (index + 1..ordered.len())
                .find(|i| !same(ordered[index], ordered[*i]))
                .or_else(|| (self.repeat == Repeat::All).then_some(0))
        } else {
            let start = (0..=index)
                .rev()
                .take_while(|i| same(ordered[index], ordered[*i]))
                .last()
                .unwrap_or(index);
            if start == 0 {
                Some(0)
            } else {
                Some(
                    (0..start)
                        .rev()
                        .take_while(|i| same(ordered[start - 1], ordered[*i]))
                        .last()
                        .unwrap_or(start - 1),
                )
            }
        };
        self.current = target.map(|index| ordered[index].key);
        self.position = 0.0;
        self.sync_up_next();
        self.current()
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
        self.order = self.entries.iter().map(|entry| entry.key).collect();
        if enabled {
            self.order.shuffle(&mut rand::thread_rng());
            if let Some(current) = self.current {
                self.order.retain(|id| *id != current);
                self.order.insert(0, current);
            }
        }
        self.arrange_up_next(false);
    }

    pub fn remove(&mut self, key: Uuid) -> bool {
        let current = self.current == Some(key);
        if current {
            self.next(false);
        }
        self.order.retain(|id| *id != key);
        self.entries.retain(|entry| entry.key != key);
        if self.current == Some(key) {
            self.current = None;
        }
        self.sync_up_next();
        current
    }

    pub fn move_entry(&mut self, key: Uuid, to: usize) {
        if let Some(index) = self.order.iter().position(|id| *id == key) {
            self.order.remove(index);
            self.order.insert(to.min(self.order.len()), key);
            self.regroup_up_next(&[key], false);
            if !self.shuffle {
                self.sort_entries();
            }
        }
    }

    /// Move selected identities as a block, keeping their current playback order.
    pub fn move_before(&mut self, keys: &[Uuid], before: Option<Uuid>) {
        let selected = keys
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if before.is_some_and(|key| selected.contains(&key)) {
            return;
        }
        let moved = self
            .order
            .iter()
            .filter(|key| selected.contains(key))
            .copied()
            .collect::<Vec<_>>();
        self.order.retain(|key| !selected.contains(key));
        let index = before
            .and_then(|key| self.order.iter().position(|entry| *entry == key))
            .unwrap_or(self.order.len());
        self.order.splice(index..index, moved);
        self.regroup_up_next(keys, false);
        if !self.shuffle {
            self.sort_entries();
        }
    }
    /// Insert a dragged block at either edge of a row, preserving source identities and order.
    pub fn move_at(&mut self, keys: &[Uuid], target: Option<Uuid>, after: bool) {
        let pin = self.pins_above_up_next(target, after);
        self.move_at_boundary(keys, target, after, pin);
    }

    /// Preserve the drop boundary while library tracks are fetched asynchronously.
    pub fn move_at_boundary(
        &mut self,
        keys: &[Uuid],
        target: Option<Uuid>,
        after: bool,
        pin: bool,
    ) {
        let boundary = target
            .and_then(|key| self.order.iter().position(|entry| *entry == key))
            .map_or(self.order.len(), |index| index + usize::from(after));
        let valid = self
            .order
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::new();
        let moved = keys
            .iter()
            .copied()
            .filter(|key| valid.contains(key) && seen.insert(*key))
            .collect::<Vec<_>>();
        let index = self.order[..boundary]
            .iter()
            .filter(|key| !seen.contains(key))
            .count();
        self.order.retain(|key| !seen.contains(key));
        self.order.splice(index..index, moved);
        self.regroup_up_next(keys, pin);
        if !self.shuffle {
            self.sort_entries();
        }
    }
    fn sort_entries(&mut self) {
        let indexes = self
            .order
            .iter()
            .enumerate()
            .map(|(index, key)| (*key, index))
            .collect::<std::collections::HashMap<_, _>>();
        self.entries
            .sort_by_key(|entry| indexes.get(&entry.key).copied());
    }
    /// Permanently shuffle each side of the playing track, preserving its index.
    pub fn shuffle_all(&mut self) {
        let mut rng = rand::thread_rng();
        if let Some(index) = self
            .current
            .and_then(|key| self.order.iter().position(|entry| *entry == key))
        {
            self.order[..index].shuffle(&mut rng);
            self.order[index + 1..].shuffle(&mut rng);
        } else {
            self.order.shuffle(&mut rng);
        }
        self.arrange_up_next(false);
        self.sort_entries();
    }
    pub fn shuffle_selected(&mut self, selected: &[Uuid]) {
        let positions = self
            .order
            .iter()
            .enumerate()
            .filter_map(|(index, key)| selected.contains(key).then_some(index))
            .collect::<Vec<_>>();
        let mut entries = positions
            .iter()
            .map(|index| self.order[*index])
            .collect::<Vec<_>>();
        entries.shuffle(&mut rand::thread_rng());
        for (index, entry) in positions.into_iter().zip(entries) {
            self.order[index] = entry;
        }
        self.arrange_up_next(false);
        self.sort_entries();
    }

    pub fn repair(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.entries.retain(|entry| seen.insert(entry.key));
        let mut seen = std::collections::HashSet::new();
        self.order
            .retain(|key| self.entries.iter().any(|entry| entry.key == *key) && seen.insert(*key));
        for entry in &self.entries {
            if !self.order.contains(&entry.key) {
                self.order.push(entry.key);
            }
        }
        if self.current.is_some() && !self.order.iter().any(|key| Some(*key) == self.current) {
            self.current = self.order.first().copied();
        }
        if !self.position.is_finite() || self.position < 0.0 {
            self.position = 0.0;
        }
        self.sync_up_next();
    }
}

#[cfg(test)]
mod tests {
    use super::{Enqueue, Queue};
    use crate::model::Item;

    fn named_songs(ids: &[&str]) -> Vec<Item> {
        ids.iter()
            .map(|id| Item {
                id: (*id).into(),
                ..Default::default()
            })
            .collect()
    }
    fn ids(queue: &Queue) -> Vec<&str> {
        queue
            .ordered()
            .iter()
            .map(|entry| entry.item.id.as_str())
            .collect()
    }

    #[test]
    fn up_next_fifo_survives_shuffle_replacement_playback_and_restore() {
        let mut queue = Queue::default();
        queue.add(named_songs(&["a", "b", "c"]), Enqueue::Now);
        queue.add(named_songs(&["x", "y"]), Enqueue::Next);
        queue.add(named_songs(&["z"]), Enqueue::Next);
        let pending = queue.up_next.clone();
        assert_eq!(ids(&queue), ["a", "x", "y", "z", "b", "c"]);
        for shuffle in [true, false] {
            queue.set_shuffle(shuffle);
            assert_eq!(queue.up_next, pending);
            queue.shuffle_all();
            assert_eq!(queue.up_next, pending);
            queue.shuffle_selected(&queue.order.clone());
            let current = queue
                .order
                .iter()
                .position(|id| Some(*id) == queue.current)
                .unwrap();
            assert_eq!(&queue.order[current + 1..current + 4], &pending);
        }
        queue.set_shuffle(false);
        queue.add(named_songs(&["new", "later"]), Enqueue::Now);
        assert_eq!(ids(&queue), ["new", "x", "y", "z", "later"]);
        assert_eq!(queue.up_next, pending);
        assert_eq!(queue.next(false).unwrap().item.id, "x");
        assert_eq!(queue.up_next, pending[1..]);
        queue.remove(pending[1]);
        assert_eq!(queue.up_next, pending[2..]);
        let mut restored: Queue =
            serde_json::from_value(serde_json::to_value(&queue).unwrap()).unwrap();
        restored.repair();
        assert_eq!(ids(&restored), ids(&queue));
        assert_eq!(restored.up_next, queue.up_next);
        assert_eq!(restored.next(false).unwrap().item.id, "z");
        assert!(restored.up_next.is_empty());
    }

    #[test]
    fn up_next_drop_edges_pinned_rows_and_move_out_are_distinct() {
        let mut queue = Queue::default();
        queue.add(named_songs(&["a", "b", "c", "d"]), Enqueue::Now);
        let normal = queue.order.clone();
        queue.add(named_songs(&["x", "y"]), Enqueue::Next);
        let pending = queue.up_next.clone();
        queue.move_at(&[normal[1]], Some(normal[0]), true);
        assert_eq!(ids(&queue), ["a", "b", "x", "y", "c", "d"]);
        assert_eq!(queue.up_next, pending);
        queue.move_at(&[normal[2]], Some(pending[0]), false);
        assert_eq!(ids(&queue), ["a", "b", "c", "x", "y", "d"]);
        assert_eq!(queue.up_next, [normal[2], pending[0], pending[1]]);
        queue.move_at(&[pending[0]], Some(normal[3]), true);
        assert_eq!(ids(&queue), ["a", "b", "c", "y", "d", "x"]);
        assert_eq!(queue.up_next, [normal[2], pending[1]]);
        // A newly fetched drop uses the boundary captured before the fetch.
        let pin = queue.pins_above_up_next(Some(normal[1]), true);
        queue.add(named_songs(&["fetched"]), Enqueue::End);
        let added = *queue.order.last().unwrap();
        queue.move_at_boundary(&[added], Some(normal[1]), true, pin);
        assert_eq!(ids(&queue), ["a", "b", "fetched", "c", "y", "d", "x"]);
        assert!(!queue.up_next.contains(&added));
        queue.current = Some(normal[3]);
        queue.sync_up_next();
        assert_eq!(ids(&queue), ["a", "b", "fetched", "d", "c", "y", "x"]);
        assert_eq!(queue.up_next, [normal[2], pending[1]]);
    }

    #[test]
    fn up_next_setting_defaults_and_duplicate_entries_are_preserved() {
        let mut queue = Queue::default();
        queue.add(named_songs(&["same", "same"]), Enqueue::Now);
        queue.add(named_songs(&["same", "same"]), Enqueue::Next);
        assert_eq!(queue.entries.len(), 4);
        assert_eq!(queue.up_next.len(), 2);
        assert_ne!(queue.up_next[0], queue.up_next[1]);
        queue.up_next_enabled = false;
        queue.sync_up_next();
        assert!(queue.up_next.is_empty());
        queue.add(named_songs(&["x"]), Enqueue::Next);
        queue.add(named_songs(&["y"]), Enqueue::Next);
        assert_eq!(&ids(&queue)[..3], &["same", "y", "x"]);
        queue.add(named_songs(&["replace"]), Enqueue::Now);
        assert_eq!(ids(&queue), ["replace"]);
        let mut old = serde_json::to_value(&queue).unwrap();
        old.as_object_mut().unwrap().remove("up_next");
        let restored: Queue = serde_json::from_value(old).unwrap();
        assert!(restored.up_next_enabled && restored.up_next.is_empty());
        let imported =
            crate::migration::preview(&Default::default(), br#"{"general":{"upNextQueue":false}}"#)
                .unwrap();
        assert!(!imported.settings.up_next_queue);
        assert!(imported.omitted.is_empty());
    }

    #[test]
    fn dragged_row_edges_match_original_store_with_duplicate_songs() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/drag-placement-reference.json")).unwrap();
        let key = |id: &serde_json::Value| {
            uuid::Uuid::from_u128(id.as_str().unwrap().as_bytes()[0] as u128)
        };
        for case in reference["cases"].as_array().unwrap() {
            let current = case["current"]
                .as_array()
                .unwrap()
                .iter()
                .map(key)
                .collect::<Vec<_>>();
            for shuffle in [false, true] {
                let mut queue = super::Queue {
                    entries: current
                        .iter()
                        .map(|key| super::Entry {
                            key: *key,
                            item: crate::model::Item {
                                id: "duplicate-song".into(),
                                ..Default::default()
                            },
                        })
                        .collect(),
                    order: current.clone(),
                    current: Some(current[2]),
                    position: 12.5,
                    shuffle,
                    ..Default::default()
                };
                let selected = case["source"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(key)
                    .collect::<Vec<_>>();
                queue.move_at(
                    &selected,
                    Some(key(&case["target"])),
                    case["edge"] == "bottom",
                );
                assert_eq!(
                    queue.order,
                    case["queue"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(key)
                        .collect::<Vec<_>>(),
                    "{case}"
                );
                assert_eq!(queue.current, Some(current[2]));
                assert_eq!(queue.position, 12.5);
                assert_eq!(queue.entries.len(), 5);
                if !shuffle {
                    assert_eq!(
                        queue
                            .entries
                            .iter()
                            .map(|entry| entry.key)
                            .collect::<Vec<_>>(),
                        queue.order
                    );
                }
            }
        }
    }

    use super::*;
    #[test]
    fn shuffling_selected_duplicate_entries_preserves_other_slots_and_current_identity() {
        let mut queue = Queue::default();
        queue.add(
            (0..8)
                .map(|track| Item {
                    id: "duplicate".into(),
                    track,
                    ..Default::default()
                })
                .collect(),
            Enqueue::Now,
        );
        let order = queue.order.clone();
        let selected = [order[1], order[4], order[6], order[1], Uuid::new_v4()];
        queue.current = Some(order[4]);
        for _ in 0..30 {
            queue.shuffle_selected(&selected);
            for index in [0, 2, 3, 5, 7] {
                assert_eq!(queue.order[index], order[index]);
            }
            assert_eq!(
                queue
                    .order
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>(),
                order.iter().copied().collect()
            );
            assert_eq!(queue.current().unwrap().item.track, 4);
            assert_eq!(
                queue
                    .entries
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>(),
                queue.order
            );
        }
    }
    #[test]
    fn autosave_counts_entry_changes_and_keeps_duplicate_tracks_distinct() {
        let mut counter = AutoSave::default();
        assert!(!counter.observe(None, 2));
        let first = Uuid::new_v4();
        assert!(!counter.observe(Some(first), 2));
        assert!(!counter.observe(Some(first), 2));
        assert!(!counter.observe(Some(Uuid::new_v4()), 2));
        let duplicate = Uuid::new_v4();
        assert!(counter.observe(Some(duplicate), 2));
        assert!(!counter.observe(Some(duplicate), 2));
        assert!(!counter.observe(Some(Uuid::new_v4()), 2));
        assert!(counter.observe(Some(Uuid::new_v4()), 2));
        // Reducing the configured interval takes effect on the next entry change.
        assert!(counter.observe(Some(Uuid::new_v4()), 1));
    }
    fn songs() -> Vec<Item> {
        ["a", "a", "b"]
            .into_iter()
            .map(|id| Item {
                id: id.into(),
                ..Item::default()
            })
            .collect()
    }

    #[test]
    fn appending_after_queue_end_starts_the_new_entries_without_replaying_old_tracks() {
        let mut queue = Queue::default();
        queue.add(
            vec![Item {
                id: "old".into(),
                ..Default::default()
            }],
            Enqueue::Now,
        );
        queue.next(true);
        assert!(queue.current.is_none());
        queue.add(
            vec![Item {
                id: "new".into(),
                ..Default::default()
            }],
            Enqueue::End,
        );
        assert_eq!(queue.current().unwrap().item.id, "new");
        queue.next(true);
        queue.add(vec![], Enqueue::End);
        assert!(queue.current.is_none());
    }
    #[test]
    fn moving_groups_and_shuffling_keep_duplicate_keys_and_the_playing_index() {
        let mut queue = Queue::default();
        queue.add(
            ["a", "a", "b", "c", "d", "e"]
                .into_iter()
                .map(|id| Item {
                    id: id.into(),
                    ..Default::default()
                })
                .collect(),
            Enqueue::Now,
        );
        let keys = queue.order.clone();
        queue.current = Some(keys[2]);
        queue.position = 17.0;
        queue.move_before(&[keys[1], keys[0]], Some(keys[4]));
        assert_eq!(
            queue.order,
            [keys[2], keys[3], keys[0], keys[1], keys[4], keys[5]]
        );
        queue.move_before(&[keys[0]], Some(keys[0]));
        assert_eq!(queue.order[2], keys[0]);
        queue.move_before(&[keys[2]], Some(keys[0]));
        assert_eq!(queue.order[1], keys[2]);
        for _ in 0..10 {
            queue.shuffle_all();
            assert_eq!(queue.order[1], keys[2]);
        }
        assert_eq!(queue.position, 17.0);
        assert_eq!(
            queue
                .order
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            6
        );
        queue.set_shuffle(false);
        assert_eq!(queue.order[1], keys[2]);
        let expected = queue
            .order
            .iter()
            .filter(|key| **key == keys[0] || **key == keys[1])
            .copied()
            .collect::<Vec<_>>();
        queue.move_before(&[keys[0], keys[1]], None);
        assert_eq!(&queue.order[4..], &expected);
    }
    #[test]
    fn album_navigation_respects_server_identity_and_contiguous_groups() {
        let server = Uuid::new_v4();
        let mut queue = Queue::default();
        queue.add(
            [
                ("a", server),
                ("a", server),
                ("b", server),
                ("b", server),
                ("b", Uuid::new_v4()),
            ]
            .into_iter()
            .map(|(album, server_id)| Item {
                album_id: album.into(),
                server_id,
                ..Default::default()
            })
            .collect(),
            Enqueue::Now,
        );
        let keys = queue.order.clone();
        queue.jump_album(true);
        assert_eq!(queue.current, Some(keys[2]));
        queue.jump_album(true);
        assert_eq!(queue.current, Some(keys[4]));
        queue.jump_album(false);
        assert_eq!(queue.current, Some(keys[2]));
        queue.jump_album(false);
        assert_eq!(queue.current, Some(keys[0]));
        queue.current = keys.last().copied();
        queue.jump_album(true);
        assert!(queue.current.is_none());
        queue.current = keys.last().copied();
        queue.repeat = Repeat::All;
        queue.jump_album(true);
        assert_eq!(queue.current, Some(keys[0]));
    }

    #[test]
    fn removing_only_repeating_track_leaves_no_current_entry() {
        let mut queue = Queue::default();
        queue.add(vec![Item::default()], Enqueue::Now);
        queue.repeat = Repeat::All;
        queue.remove(queue.current.unwrap());
        assert!(queue.current.is_none());
        assert!(queue.entries.is_empty());
    }

    #[test]
    fn duplicate_songs_have_distinct_queue_identity() {
        let mut queue = Queue::default();
        queue.add(songs(), Enqueue::Now);
        let first = queue.current.unwrap();
        assert_eq!(queue.next(false).unwrap().item.id, "a");
        assert_ne!(queue.current.unwrap(), first);
        assert_eq!(queue.next(false).unwrap().item.id, "b");
        assert!(queue.next(true).is_none());
    }

    #[test]
    fn repeat_one_only_applies_to_natural_end_and_shuffle_keeps_current() {
        let mut queue = Queue::default();
        queue.add(songs(), Enqueue::Now);
        queue.repeat = Repeat::One;
        let first = queue.current;
        queue.next(true);
        assert_eq!(queue.current, first);
        queue.next(false);
        assert_ne!(queue.current, first);
        let current = queue.current;
        queue.set_shuffle(true);
        assert_eq!(queue.order.first().copied(), current);
        queue.set_shuffle(false);
        assert_eq!(queue.current, current);
        queue.repeat = Repeat::All;
        queue.current = queue.order.last().copied();
        assert_eq!(queue.next(true).unwrap().item.id, "a");
    }

    #[test]
    fn play_next_remove_reorder_and_restore() {
        let mut queue = Queue::default();
        queue.add(songs(), Enqueue::Now);
        queue.add(
            vec![Item {
                id: "next".into(),
                ..Item::default()
            }],
            Enqueue::Next,
        );
        assert_eq!(queue.next(false).unwrap().item.id, "next");
        let current = queue.current.unwrap();
        queue.remove(current);
        assert_eq!(queue.current().unwrap().item.id, "a");
        let key = queue.order[2];
        queue.move_entry(key, 0);
        assert_eq!(queue.order[0], key);
        queue.order.push(Uuid::new_v4());
        queue.order.push(key);
        queue.repair();
        assert_eq!(queue.order.len(), queue.entries.len());
        let restored: Queue =
            serde_json::from_str(&serde_json::to_string(&queue).unwrap()).unwrap();
        assert_eq!(restored.current, queue.current);
    }
}
