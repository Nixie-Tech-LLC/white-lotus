// Scenario runners that emit visualizer traces to target/viz/*.jsonl.
//
//   cargo test --test visualize -- --ignored --nocapture
//
// then open tools/viz/index.html and load the trace.
//
// Every scenario is #[ignore]d so a normal `cargo test` run does not write
// files. The two tests that are NOT ignored guard the harness itself.

mod common;

use common::Sim;

// The payload we gossip in these scenarios - stands in for a file hash.
const PAYLOAD: u64 = 777;

// ---------------------------------------------------------------------------
// Harness guards (these DO run on a normal `cargo test`)
// ---------------------------------------------------------------------------

// probe_active() leans on membership.rs:176-192 - a Shuffle with ttl == 0 and
// an empty peer list replies with the exact active view and mutates nothing.
// If that branch ever changes shape, every trace silently becomes fiction, so
// pin the behaviour here against a wiring whose result we know.
#[test]
fn probe_reports_the_real_active_view() {
	let mut sim = Sim::<u64>::new(6, |c| c.fanout = 4);
	sim.wire_ring(2);

	// Node 0 was joined with peers 1 and 2; a ring with 2 forward chords also
	// makes 4 and 5 join node 0. Active capacity is fanout + 1 = 5.
	let view = sim.probe_active(0);
	assert!(view.contains(&1), "expected 1 in {view:?}");
	assert!(view.contains(&2), "expected 2 in {view:?}");
	assert!(!view.contains(&0), "node must never hold itself: {view:?}");
	assert!(view.len() <= 5, "active view over capacity: {view:?}");
}

// A probe must be a pure read: probing twice, and probing between broadcasts,
// must not perturb the run.
#[test]
fn probe_does_not_disturb_the_node() {
	let mut sim = Sim::<u64>::new(10, |c| {
		c.fanout = 4;
		c.max_rounds = 10;
	});
	sim.wire_ring(3);

	let before = sim.probe_active(4);
	let again = sim.probe_active(4);
	assert_eq!(before, again, "probe is not idempotent");

	sim.broadcast(0, PAYLOAD);
	sim.run_fifo();

	// Delivery is unaffected by the probes above.
	let delivered = sim.delivery_counts();
	assert_eq!(delivered.len(), 9);
	assert!(delivered.values().all(|&c| c == 1));
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

// 1. The core Plumtree behaviour: a 30-node ring carries one broadcast, and the
//    eager spanning tree emerges as duplicate Broadcasts trigger PRUNEs
//    (gossip.rs:144-152). Watch the solid edges thin out round by round.
#[test]
#[ignore]
fn viz_tree_formation() {
	let mut sim = Sim::<u64>::new(30, |c| {
		c.fanout = 4;
		c.max_rounds = 30;
	});
	sim.wire_ring(4);
	sim.probe_all();
	sim.broadcast(0, PAYLOAD);
	sim.run_rounds_probing();

	let path = sim.write_trace("tree_formation");
	println!("wrote {path}");
	println!(
		"  delivered to {} nodes, redundancy {:.2}x, max hop {}",
		sim.delivery_counts().len(),
		sim.redundancy(),
		sim.max_hop()
	);
}

// 2. Organic membership: nodes join one at a time through the real Join /
//    ForwardJoin path, with a snapshot after each.
//
//    This is where the missing randomness becomes visible. Every "random"
//    choice in membership.rs is HashSet::iter().next() or .find() - the
//    eviction victim (:62, :79), the ForwardJoin next hop (:130), the Shuffle
//    next hop (:179). HyParView's guarantees assume uniform random selection,
//    so the overlay this produces should look conspicuously lopsided next to
//    the paper's figures. That is the implementation, not the renderer.
#[test]
#[ignore]
fn viz_organic_join() {
	let mut sim = Sim::<u64>::new(16, |c| {
		c.fanout = 4;
		c.max_rounds = 16;
	});

	// Seed a small connected core, then let the rest walk in.
	sim.wire_ring(1);
	sim.probe_all();

	for node in 1..16u32 {
		sim.join(node, 0);
		sim.run_rounds();
		sim.probe_all();
	}

	let path = sim.write_trace("organic_join");
	println!("wrote {path}");
}

// 3. Lazy-path repair. This needs staging, because neither half of the
//    mechanism exists at the start of a run:
//
//    a) `lazy` starts empty (gossip.rs:54), so the FIRST broadcast pushes
//       eagerly to everyone and emits no IHave at all. Lazy links only exist
//       once duplicates have triggered PRUNEs. So broadcast twice - the second
//       one rides the tree the first one built, and announces via IHave.
//    b) GRAFT only fires if the eager payload never turns up, so we drop the
//       payload in flight to a few nodes while letting their IHave through.
//
//    Then heal, tick past graft_timeout, and the starved nodes should GRAFT
//    their announcer and recover (gossip.rs:97-135).
#[test]
#[ignore]
fn viz_lazy_repair() {
	let mut sim = Sim::<u64>::new(12, |c| {
		c.fanout = 3;
		c.max_rounds = 12;
		c.graft_timeout = 100;
		c.graft_retry_timeout = 50;
	});
	sim.wire_ring(3);
	sim.probe_all();

	// Pass 1: builds the eager tree, leaves lazy links behind.
	sim.broadcast(0, PAYLOAD);
	sim.run_rounds();
	sim.probe_all();

	// Pass 2: starve nodes 6 and 7 of the payload, but let their IHave land.
	sim.blackhole(&[6, 7]);
	sim.broadcast(0, PAYLOAD + 1);
	sim.run_rounds();
	sim.probe_all();

	// Heal the link, then advance past the GRAFT deadline. If the repair works,
	// 6 and 7 GRAFT their announcer and the payload arrives late.
	sim.clear_blackhole();
	sim.tick(150);
	sim.run_rounds();
	sim.probe_all();
	sim.tick(250);
	sim.run_rounds();
	sim.probe_all();

	let path = sim.write_trace("lazy_repair");
	println!("wrote {path}");
	println!(
		"  IHave: {}, GRAFT: {}, dropped: {}",
		sim.trace().count_msg("IHave"),
		sim.trace().count_msg("Graft"),
		sim.trace().of_kind("drop").len()
	);
	println!("  deliveries per node: {:?}", {
		let mut v: Vec<_> = sim.delivery_counts().into_iter().collect();
		v.sort();
		v
	});
}

// 4. Active-view eviction under pressure: a star topology forces the hub well
//    past its capacity of fanout + 1, so it must demote peers.
//
//    Also the scenario where the NeighborReply bug would show. membership.rs:169
//    calls add_to_active and discards the evicted peer, unlike the three other
//    call sites (:101, :120, :155) which send it a Disconnect. If Neighbor
//    traffic is ever driven here, the evicted peer keeps the link in its own
//    view and the overlay renders a one-way edge.
#[test]
#[ignore]
fn viz_eviction() {
	let mut sim = Sim::<u64>::new(14, |c| {
		c.fanout = 3; // active capacity 4, so a 13-peer star overflows hard
		c.max_rounds = 14;
	});
	sim.wire_star(0);
	sim.probe_all();

	sim.broadcast(0, PAYLOAD);
	sim.run_rounds_probing();

	let path = sim.write_trace("eviction");
	println!("wrote {path}");
	println!("  hub active view: {:?}", sim.probe_active(0));
}
