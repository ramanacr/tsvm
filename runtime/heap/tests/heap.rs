use tsvm_heap::{GcHeap, HeapHandle, Trace, Tracer};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    name: &'static str,
    edges: Vec<HeapHandle>,
}

impl Trace for Node {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for handle in &self.edges {
            tracer.mark(*handle);
        }
    }
}

#[test]
fn allocation_returns_a_live_handle() {
    let mut heap = GcHeap::new();
    let handle = heap.allocate(Node {
        name: "root",
        edges: Vec::new(),
    });

    assert_eq!(heap.live_len(), 1);
    assert_eq!(
        heap.get(handle).expect("handle should resolve").name,
        "root"
    );
}

#[test]
fn tracing_keeps_reachable_objects_and_collects_the_rest() {
    let mut heap = GcHeap::new();
    let leaf = heap.allocate(Node {
        name: "leaf",
        edges: Vec::new(),
    });
    let root = heap.allocate(Node {
        name: "root",
        edges: vec![leaf],
    });
    let orphan = heap.allocate(Node {
        name: "orphan",
        edges: Vec::new(),
    });

    let report = heap.collect([root]);

    assert_eq!(report.marked, 2);
    assert_eq!(report.collected, 1);
    assert_eq!(heap.live_len(), 2);
    assert!(heap.contains(root));
    assert!(heap.contains(leaf));
    assert!(!heap.contains(orphan));
}

#[test]
fn stale_handles_do_not_resolve_after_slot_reuse() {
    let mut heap = GcHeap::new();
    let stale = heap.allocate(Node {
        name: "old",
        edges: Vec::new(),
    });
    heap.collect([]);

    let fresh = heap.allocate(Node {
        name: "new",
        edges: Vec::new(),
    });

    assert_eq!(fresh.index(), stale.index());
    assert_ne!(fresh.generation(), stale.generation());
    assert!(heap.get(stale).is_none());
    assert_eq!(
        heap.get(fresh).expect("fresh handle should resolve").name,
        "new"
    );
}

#[test]
fn stress_collects_many_unreachable_allocations() {
    let mut heap = GcHeap::new();
    let root = heap.allocate(Node {
        name: "root",
        edges: Vec::new(),
    });
    for index in 0..10_000 {
        let name = if index % 2 == 0 { "even" } else { "odd" };
        heap.allocate(Node {
            name,
            edges: Vec::new(),
        });
    }

    let report = heap.collect([root]);

    assert_eq!(report.marked, 1);
    assert_eq!(report.collected, 10_000);
    assert_eq!(heap.live_len(), 1);
    assert!(heap.contains(root));
}
