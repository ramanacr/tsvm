#![forbid(unsafe_code)]

use std::collections::VecDeque;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct HeapHandle {
    index: u32,
    generation: u32,
}

impl HeapHandle {
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CollectionReport {
    pub marked: usize,
    pub collected: usize,
    pub live_after: usize,
}

pub trait Trace {
    fn trace(&self, tracer: &mut Tracer<'_>);
}

impl Trace for () {
    fn trace(&self, _tracer: &mut Tracer<'_>) {}
}

pub struct Tracer<'heap> {
    marks: &'heap mut [bool],
    pending: &'heap mut VecDeque<usize>,
    slots: &'heap [SlotMarker],
}

impl Tracer<'_> {
    pub fn mark(&mut self, handle: HeapHandle) {
        let index = handle.index as usize;
        let Some(slot) = self.slots.get(index) else {
            return;
        };
        if slot.generation != handle.generation || !slot.occupied || self.marks[index] {
            return;
        }
        self.marks[index] = true;
        self.pending.push_back(index);
    }
}

#[derive(Debug, Clone)]
pub struct GcHeap<T> {
    slots: Vec<Slot<T>>,
    free_list: Vec<usize>,
    live_len: usize,
}

impl<T> Default for GcHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GcHeap<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            live_len: 0,
        }
    }

    pub fn allocate(&mut self, value: T) -> HeapHandle {
        let index = self.free_list.pop().unwrap_or(self.slots.len());
        if index == self.slots.len() {
            self.slots.push(Slot {
                generation: 0,
                value: Some(value),
            });
        } else {
            self.slots[index].value = Some(value);
        }
        self.live_len += 1;
        HeapHandle {
            index: index as u32,
            generation: self.slots[index].generation,
        }
    }

    pub fn get(&self, handle: HeapHandle) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, handle: HeapHandle) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    pub fn contains(&self, handle: HeapHandle) -> bool {
        self.get(handle).is_some()
    }

    pub fn live_len(&self) -> usize {
        self.live_len
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
}

impl<T: Trace> GcHeap<T> {
    pub fn collect<I>(&mut self, roots: I) -> CollectionReport
    where
        I: IntoIterator<Item = HeapHandle>,
    {
        let markers = self
            .slots
            .iter()
            .map(|slot| SlotMarker {
                generation: slot.generation,
                occupied: slot.value.is_some(),
            })
            .collect::<Vec<_>>();
        let mut marks = vec![false; self.slots.len()];
        let mut pending = VecDeque::new();

        {
            let mut tracer = Tracer {
                marks: &mut marks,
                pending: &mut pending,
                slots: &markers,
            };
            for root in roots {
                tracer.mark(root);
            }
        }

        while let Some(index) = pending.pop_front() {
            if let Some(value) = self.slots[index].value.as_ref() {
                let mut tracer = Tracer {
                    marks: &mut marks,
                    pending: &mut pending,
                    slots: &markers,
                };
                value.trace(&mut tracer);
            }
        }

        let marked = marks.iter().filter(|mark| **mark).count();
        let mut collected = 0;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.value.is_some() && !marks[index] {
                slot.value = None;
                slot.generation = slot.generation.wrapping_add(1);
                self.free_list.push(index);
                collected += 1;
            }
        }
        self.live_len -= collected;

        CollectionReport {
            marked,
            collected,
            live_after: self.live_len,
        }
    }
}

#[derive(Debug, Clone)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

#[derive(Debug, Copy, Clone)]
struct SlotMarker {
    generation: u32,
    occupied: bool,
}
