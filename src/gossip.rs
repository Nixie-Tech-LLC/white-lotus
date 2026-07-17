use std::collections::HashSet;
use crate::{NodeId, Payload};
use crate::config::Config;
use crate::membership::Membership;
use crate::message::{Message, MessageId};
use crate::action::Action;
use crate::broadcast;

// The whole node: config + membership state + broadcast dedup, tied together.
pub struct Node<Id: NodeId> {
	config: Config<Id>,
	membership: Membership<Id>,
	// (origin, seq) ids we've already seen, so we never deliver or forward twice
	seen: HashSet<(Id, MessageId)>,
	// per-node counter for minting fresh broadcast sequence numbers
	next_seq: MessageId,
}

impl<Id: NodeId> Node<Id> {
	// Build a node from its config, setting up an empty membership.
	pub fn new(config: Config<Id>) -> Self {
		let membership = Membership::new(
			config.me,
			config.fanout,
			config.passive_capacity,
			config.active_walk_length,
			config.passive_walk_length,
		);
		Node {
			config,
			membership,
			seen: HashSet::new(),
			next_seq: 0,
		}
	}

	// Start a brand-new broadcast of `payload` from this node.
	pub fn broadcast<P: Payload>(&mut self, payload: P) -> Vec<Action<Id, P>> {
		let seq = self.next_seq;
		self.next_seq += 1;
		self.seen.insert((self.config.me, seq));
		let peers: Vec<Id> = self.membership.active_peers().copied().collect();
		broadcast::forward(self.config.me, &peers, None, self.config.me, seq, 0, &payload)
	}

	// React to any incoming message.
	pub fn handle<P: Payload>(&mut self, msg: Message<Id, P>) -> Vec<Action<Id, P>> {
		match msg {
			Message::Broadcast { origin, seq, sender, hop, payload } => {
				let mut actions = Vec::new();
				// Dedup on the globally-unique (origin, seq) id.
				if !self.seen.insert((origin, seq)) {
					return actions;
				}
				// Deliver it locally.
				actions.push(Action::Deliver { payload: payload.clone() });
				// Stop if it has reached the hop limit.
				if hop >= self.config.max_rounds {
					return actions;
				}
				// Otherwise keep it moving - forward to the rest of the active view.
				let peers: Vec<Id> = self.membership.active_peers().copied().collect();
				actions.extend(broadcast::forward(
					self.config.me, &peers, Some(sender), origin, seq, hop, &payload,
				));
				actions
			}
			// Everything else is membership control - hand it to that layer.
			other => self.membership.handle(other),
		}
	}
}
