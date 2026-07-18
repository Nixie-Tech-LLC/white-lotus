#![allow(dead_code)]

// Test harness for driving a white-lotus overlay in one process and recording
// everything it does, so the run can be replayed in the visualizer.
//
// This lives in tests/common/ (a subdirectory) so cargo does not compile it as
// a test binary of its own - it is a module included by other integration tests.
//
// IMPORTANT - what this harness can and cannot see:
//
//   Tests only reach the public API (Config, Node, Message, Action). The
//   Membership type is private, so we cannot read a node's views directly.
//   Rather than reimplement membership logic here (which would drift from
//   src/ and then lie about the algorithm), we observe:
//
//     - messages and actions: ground truth, we own the queue
//     - active view:          ground truth, via probe_active() below
//     - eager/lazy split:     derived from observed Prune/Graft/Broadcast
//     - passive view:         NOT OBSERVABLE. add_to_passive emits no actions
//                             and has no accessor. We never guess at it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};
use white_lotus::{Action, Config, Message, Node, Payload};

// Reserved id used as the origin of a probe. Must stay outside the overlay's
// id space so no node ever stores it as a peer.
pub const PROBE_ID: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// Trace
// ---------------------------------------------------------------------------

// The recorded run: one JSON object per event, written out as JSONL.
pub struct Trace {
	events: Vec<Value>,
}

impl Trace {
	fn new() -> Self {
		Trace { events: Vec::new() }
	}

	fn push(&mut self, event: Value) {
		self.events.push(event);
	}

	pub fn len(&self) -> usize {
		self.events.len()
	}

	pub fn events(&self) -> &[Value] {
		&self.events
	}

	// How many `send` events carried a given Message variant, e.g. "IHave".
	pub fn count_msg(&self, variant: &str) -> usize {
		self.events
			.iter()
			.filter(|e| e["kind"] == "send" && e["msg"].get(variant).is_some())
			.count()
	}

	// Every event of a given kind, in order. Handy for assertions.
	pub fn of_kind(&self, kind: &str) -> Vec<&Value> {
		self.events
			.iter()
			.filter(|e| e["kind"] == kind)
			.collect()
	}

}

// ---------------------------------------------------------------------------
// Sim
// ---------------------------------------------------------------------------

pub struct Sim<P: Payload + Serialize> {
	nodes: HashMap<u32, Node<u32, P>>,
	ids: Vec<u32>,
	queue: VecDeque<(u32, Message<u32, P>)>,
	trace: Trace,
	step: u64,
	round: u64,
	// derived (tier B): node -> peers it has demoted to lazy
	lazy: HashMap<u32, HashSet<u32>>,
	// fault injection: Broadcast payloads addressed to these nodes are dropped
	// in flight, simulating eager-path loss so the lazy path has to repair it
	blackhole: HashSet<u32>,
	// running tallies
	deliveries: HashMap<u32, u32>,
	sends: u64,
	max_hop: u32,
}

impl<P: Payload + Serialize> Sim<P> {
	// Build n nodes with ids 0..n. `cfg` customises each node's Config before
	// the node is constructed.
	pub fn new(n: u32, cfg: impl Fn(&mut Config<u32>)) -> Self {
		let mut nodes = HashMap::new();
		let mut ids = Vec::new();
		let mut sample: Option<(usize, u32, u32, u32, u32)> = None;

		for i in 0..n {
			let mut c = Config::new(i);
			cfg(&mut c);
			if sample.is_none() {
				sample = Some((
					c.fanout,
					c.max_rounds,
					c.active_walk_length,
					c.passive_walk_length,
					c.passive_capacity as u32,
				));
			}
			nodes.insert(i, Node::new(c));
			ids.push(i);
		}

		let mut sim = Sim {
			nodes,
			ids,
			queue: VecDeque::new(),
			trace: Trace::new(),
			step: 0,
			round: 0,
			lazy: HashMap::new(),
			blackhole: HashSet::new(),
			deliveries: HashMap::new(),
			sends: 0,
			max_hop: 0,
		};

		let (fanout, max_rounds, arwl, prwl, passive_cap) = sample.unwrap();
		sim.record(json!({
			"kind": "config",
			"nodes": n,
			"fanout": fanout,
			"active_capacity": fanout + 1,
			"passive_capacity": passive_cap,
			"max_rounds": max_rounds,
			"arwl": arwl,
			"prwl": prwl,
		}));

		sim
	}

	fn record(&mut self, mut event: Value) {
		event["step"] = json!(self.step);
		event["round"] = json!(self.round);
		self.step += 1;
		self.trace.push(event);
	}

	// -----------------------------------------------------------------------
	// Wiring
	// -----------------------------------------------------------------------

	// Node i's active view = its next `links` peers: a ring with forward chords,
	// guaranteed connected. Mirrors tests/simulation.rs so traces are comparable.
	//
	// This is artificial setup, not organic membership: the ForwardJoin actions
	// each Join produces are discarded rather than routed. Use join() for the
	// real protocol path.
	pub fn wire_ring(&mut self, links: u32) {
		let n = self.ids.len() as u32;
		for i in 0..n {
			for j in 1..=links {
				let peer = (i + j) % n;
				if peer != i {
					let _ = self
						.nodes
						.get_mut(&i)
						.unwrap()
						.handle(Message::Join { new_node: peer });
				}
			}
		}
		self.record(json!({ "kind": "wired", "topology": "ring", "links": links }));
	}

	// Every node connected to a single hub.
	pub fn wire_star(&mut self, hub: u32) {
		let ids: Vec<u32> = self.ids.clone();
		for &i in &ids {
			if i == hub {
				continue;
			}
			let _ = self
				.nodes
				.get_mut(&hub)
				.unwrap()
				.handle(Message::Join { new_node: i });
			let _ = self
				.nodes
				.get_mut(&i)
				.unwrap()
				.handle(Message::Join { new_node: hub });
		}
		self.record(json!({ "kind": "wired", "topology": "star", "hub": hub }));
	}

	// Organic join: `node` knocks on `contact`, and the resulting ForwardJoin
	// walk is routed for real. This is the path main.rs takes on Ctrl::Contact.
	pub fn join(&mut self, node: u32, contact: u32) {
		self.record(json!({ "kind": "join", "node": node, "contact": contact }));
		let actions = self
			.nodes
			.get_mut(&contact)
			.unwrap()
			.handle(Message::Join { new_node: node });
		self.absorb(contact, actions);
	}

	// -----------------------------------------------------------------------
	// Driving
	// -----------------------------------------------------------------------

	pub fn broadcast(&mut self, from: u32, payload: P) {
		self.record(json!({ "kind": "origin", "node": from }));
		let actions = self.nodes.get_mut(&from).unwrap().broadcast(payload);
		self.absorb(from, actions);
	}

	// Fault injection -------------------------------------------------------

	// Drop Broadcast payloads addressed to these nodes while leaving IHave
	// announcements intact - i.e. lose the eager path but not the lazy one.
	// Heal with clear_blackhole() before ticking, or the GRAFT reply (itself a
	// Broadcast) is dropped too and the node can never recover.
	pub fn blackhole(&mut self, nodes: &[u32]) {
		self.blackhole = nodes.iter().copied().collect();
		self.record(json!({ "kind": "fault", "blackhole": nodes }));
	}

	pub fn clear_blackhole(&mut self) {
		self.blackhole.clear();
		self.record(json!({ "kind": "heal" }));
	}

	// Advance every node's clock. Drives the Plumtree GRAFT timers.
	pub fn tick(&mut self, now: u64) {
		self.record(json!({ "kind": "tick", "now": now }));
		let ids = self.ids.clone();
		for id in ids {
			let actions = self.nodes.get_mut(&id).unwrap().tick(now);
			self.absorb(id, actions);
		}
	}

	// Deliver one queued message. Returns false when the network is quiet.
	pub fn step(&mut self) -> bool {
		match self.queue.pop_front() {
			None => false,
			Some((to, msg)) => {
				self.deliver(to, msg);
				true
			}
		}
	}

	// Pure FIFO drain - identical semantics to tests/simulation.rs:47.
	pub fn run_fifo(&mut self) {
		while self.step() {}
	}

	// Drain everything currently in flight as one round before advancing. Every
	// message in flight at round r is rendered simultaneously, which is much
	// easier to follow than FIFO (where one branch races ahead of another).
	// Returns false when the network is quiet.
	pub fn round(&mut self) -> bool {
		if self.queue.is_empty() {
			return false;
		}
		self.round += 1;
		let batch: Vec<(u32, Message<u32, P>)> = self.queue.drain(..).collect();
		for (to, msg) in batch {
			self.deliver(to, msg);
		}
		true
	}

	pub fn run_rounds(&mut self) {
		while self.round() {}
	}

	// Round-based drain, snapshotting every node's active view between rounds.
	pub fn run_rounds_probing(&mut self) {
		self.probe_all();
		while self.round() {
			self.probe_all();
		}
	}

	// -----------------------------------------------------------------------
	// Internals
	// -----------------------------------------------------------------------

	// Hand a message to its target, record it, and queue whatever comes back.
	fn deliver(&mut self, to: u32, msg: Message<u32, P>) {
		// Message has no Clone (message.rs:7), but Serialize takes &self - so
		// serialize first, then move the message into handle().
		let payload = serde_json::to_value(&msg).unwrap();

		// Peek before the move: derive the eager/lazy split from observed
		// traffic, mirroring gossip.rs:143 / :193 / :211.
		let mut derived: Option<Value> = None;
		match &msg {
			Message::Prune { sender } => {
				self.lazy.entry(to).or_default().insert(*sender);
				derived = Some(json!({
					"kind": "prune", "node": to, "peer": sender, "derived": true
				}));
			}
			Message::Graft { sender, .. } => {
				self.lazy.entry(to).or_default().remove(sender);
				derived = Some(json!({
					"kind": "graft", "node": to, "peer": sender, "derived": true
				}));
			}
			Message::Broadcast { sender, hop, .. } => {
				self.lazy.entry(to).or_default().remove(sender);
				if *hop > self.max_hop {
					self.max_hop = *hop;
				}
			}
			_ => {}
		}

		self.record(json!({ "kind": "recv", "node": to, "msg": payload }));
		if let Some(d) = derived {
			self.record(d);
		}

		let actions = self.nodes.get_mut(&to).unwrap().handle(msg);
		self.absorb(to, actions);
	}

	// Record a node's emitted actions and queue any outbound messages.
	fn absorb(&mut self, node: u32, actions: Vec<Action<u32, P>>) {
		for action in actions {
			match action {
				Action::Send { to, msg } => {
					let payload = serde_json::to_value(&msg).unwrap();
					// Eager-path loss: drop the payload in flight but let the
					// IHave announcement through, so the receiver knows the
					// message exists and must GRAFT to get it.
					let lost = self.blackhole.contains(&to)
						&& matches!(msg, Message::Broadcast { .. });
					if lost {
						self.record(json!({
							"kind": "drop", "from": node, "to": to, "msg": payload
						}));
						continue;
					}
					self.record(json!({
						"kind": "send", "from": node, "to": to, "msg": payload
					}));
					self.sends += 1;
					self.queue.push_back((to, msg));
				}
				Action::Deliver { payload } => {
					*self.deliveries.entry(node).or_insert(0) += 1;
					self.record(json!({
						"kind": "deliver",
						"node": node,
						"payload": serde_json::to_value(&payload).unwrap(),
					}));
				}
				Action::Connect { peer } => {
					self.record(json!({ "kind": "connect", "node": node, "peer": peer }));
				}
				Action::Disconnect { peer } => {
					self.record(json!({ "kind": "disconnect", "node": node, "peer": peer }));
				}
			}
		}
	}

	// -----------------------------------------------------------------------
	// Observation
	// -----------------------------------------------------------------------

	// Read a node's exact active view without modifying it.
	//
	// membership.rs:176 - a Shuffle with ttl == 0 falls to the else branch,
	// which replies with `self.active` verbatim (:187) and absorbs the peers we
	// sent (:188). Sending an empty peer list means that absorb loop runs zero
	// times, so this is a pure read. Nothing in the gossip layer is touched.
	//
	// The reply is addressed to PROBE_ID and intercepted here - it is never
	// routed back into the network, and never recorded as a send.
	pub fn probe_active(&mut self, node: u32) -> Vec<u32> {
		let probe = Message::Shuffle {
			origin: PROBE_ID,
			sender: PROBE_ID,
			ttl: 0,
			peers: Vec::new(),
		};
		let actions = self.nodes.get_mut(&node).unwrap().handle(probe);

		let mut view = Vec::new();
		for action in actions {
			if let Action::Send {
				to: PROBE_ID,
				msg: Message::ShuffleReplay { peers },
			} = action
			{
				view = peers;
			}
		}
		view.sort_unstable();
		view
	}

	// Snapshot every node's active view (plus the derived eager/lazy split).
	pub fn probe_all(&mut self) {
		let ids = self.ids.clone();
		for id in ids {
			let active = self.probe_active(id);
			let lazy: Vec<u32> = {
				let l = self.lazy.entry(id).or_default();
				let mut v: Vec<u32> = active.iter().copied().filter(|p| l.contains(p)).collect();
				v.sort_unstable();
				v
			};
			self.record(json!({
				"kind": "probe",
				"node": id,
				"active": active,
				"lazy": lazy,
			}));
		}
	}

	// -----------------------------------------------------------------------
	// Assertions - useful in ordinary tests, not just visualization
	// -----------------------------------------------------------------------

	pub fn delivery_counts(&self) -> HashMap<u32, u32> {
		self.deliveries.clone()
	}

	// Messages sent per useful delivery. 1.0 would be a perfect tree.
	pub fn redundancy(&self) -> f64 {
		let delivered: u32 = self.deliveries.values().sum();
		if delivered == 0 {
			0.0
		} else {
			self.sends as f64 / delivered as f64
		}
	}

	pub fn max_hop(&self) -> u32 {
		self.max_hop
	}

	pub fn trace(&self) -> &Trace {
		&self.trace
	}

	// Write the trace to target/viz/<name>.jsonl and return the path.
	pub fn write_trace(&self, name: &str) -> String {
		let dir = Path::new("target").join("viz");
		fs::create_dir_all(&dir).expect("create target/viz");
		let path = dir.join(format!("{name}.jsonl"));
		let body: String = self
			.trace
			.events
			.iter()
			.map(|e| serde_json::to_string(e).unwrap())
			.collect::<Vec<_>>()
			.join("\n");
		fs::write(&path, body).expect("write trace");
		path.display().to_string()
	}
}
