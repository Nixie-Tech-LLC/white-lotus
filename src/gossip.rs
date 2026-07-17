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
	// message ids we've already seen, so we never deliver or forward twice
	seen: HashSet<MessageId>,
	// counter for minting fresh broadcast ids
	next_id: MessageId,
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
			next_id: 0,
		}
	}

	// Start a brand-new broadcast of `payload` from this node.
	pub fn broadcast<P: Payload>(&mut self, payload: P) -> Vec<Action<Id, P>> {
		let id = self.next_id;
		self.next_id += 1;
		self.seen.insert(id);
		let peers: Vec<Id> = self.membership.active_peers().copied().collect();
		broadcast::forward(self.config.me, &peers, None, id, 0, &payload)
	}

	// React to any incoming message.
	pub fn handle<P: Payload>(&mut self, msg: Message<Id, P>) -> Vec<Action<Id, P>> {
		match msg {
			Message::Broadcast { id, sender, hop, payload } => {
				let mut actions = Vec::new();
				// Dedup: ignore anything we've already seen.
				if !self.seen.insert(id) {
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
					self.config.me, &peers, Some(sender), id, hop, &payload,
				));
				actions
			}
			// Everything else is membership control - hand it to that layer.
			other => self.membership.handle(other),
		}
	}
}
