use std::collections::HashSet;
use crate::{NodeId, Payload};
use crate::action::Action;
use crate::message::Message;

//node membership - views and size mimits
pub struct Membership<Id: NodeId> {
	// nodes personal identity
	me:Id,
	//small set of peers kept alive for broadcasting - capasity fanout +1
	active: HashSet<Id>,
	//larger back up set this is the 30 peers - when a peer in the inner circle fails one of these peers is called in - capasity is greater than log(n)
	passive: HashSet<Id>,
	//max size active view
	active_capasity: usize,
	//max pastive view
	passive_capasity: usize,
	//ARWL - how far a ForwardJoin walks before a node adopts it
	active_walk_length: u32,
	//PRWL - hop at which a joining node is stashed in the passive view
	passive_walk_length: u32,
}

impl<Id: NodeId> Membership<Id> {
	//start with empty views (sized per specs in essay)
	pub fn new(me: Id, fanout: usize, passive_capasity: usize, active_walk_length: u32, passive_walk_length: u32) -> Self
{
 Membership {
            me,
            active: HashSet::new(),
            passive: HashSet::new(),
            active_capasity: fanout + 1,
            passive_capasity,
            active_walk_length,
            passive_walk_length,
        }
    }
//is active view full of the appropriate number of nodes?
    pub fn active_is_full(&self) -> bool {
        self.active.len() >= self.active_capasity
    }
//peers currently broadcasting
    pub fn active_peers(&self) -> impl Iterator<Item = &Id> {
        self.active.iter()
    }
//is this peer known?
    pub fn contains(&self, peer: Id) -> bool {
        self.active.contains(&peer) || self.passive.contains(&peer)
    }

    pub fn add_to_passive(&mut self, peer: Id) {
        if peer == self.me
            || self.active.contains(&peer)
            || self.passive.contains(&peer)
        {
            return; // never store ourselves or a peer we already know
        }
        if self.passive.len() >= self.passive_capasity {
            // TODO: HyParView drops a *random* peer; grab any one for now.
            if let Some(&victim) = self.passive.iter().next() {
                self.passive.remove(&victim);
            }
        }
        self.passive.insert(peer);
    }

    /// Promote a peer into the active view. If the view is full, one existing
    /// active peer is demoted to the passive view and returned, so the caller
    /// can send it a Disconnect.
    pub fn add_to_active(&mut self, peer: Id) -> Option<Id> {
        if peer == self.me || self.active.contains(&peer) {
            return None;
        }
        self.passive.remove(&peer); // it's moving up from passive
        let mut evicted = None;
        if self.active_is_full() {
            if let Some(&victim) = self.active.iter().next() {
                self.active.remove(&victim);
                self.add_to_passive(victim); // demote, don't lose them
                evicted = Some(victim);
            }
        }
        self.active.insert(peer);
        evicted
    }

    /// Remove a failed or leaving peer from the active view.
    pub fn drop_from_active(&mut self, peer: Id) {
        self.active.remove(&peer);
    }

    pub fn handle<P: Payload>(&mut self, msg: Message<Id, P>) -> Vec<Action<Id, P>> {
        let mut actions = Vec::new();
        match msg {
            Message::Join { new_node } => {
                // Accept the newcomer into our active view.
                let evicted = self.add_to_active(new_node);
                actions.push(Action::Connect { peer: new_node });
                if let Some(old) = evicted {
                    actions.push(Action::Send { to: old, msg: Message::Disconnect { sender: self.me } });
                    actions.push(Action::Disconnect { peer: old });
                }
                // Tell our other active peers, kicking off the random walk.
                for &peer in self.active.iter() {
                    if peer != new_node {
                        actions.push(Action::Send {
                            to: peer,
                            msg: Message::ForwardJoin { new_node, sender: self.me, ttl: self.active_walk_length },
                        });
                    }
                }
            }
            Message::ForwardJoin { new_node, sender, ttl } => {
                // End of the walk, or we're nearly isolated: adopt into active view.
                if ttl == 0 || self.active.len() <= 1 {
                    let evicted = self.add_to_active(new_node);
                    actions.push(Action::Connect { peer: new_node });
                    if let Some(old) = evicted {
                        actions.push(Action::Send { to: old, msg: Message::Disconnect { sender: self.me } });
                        actions.push(Action::Disconnect { peer: old });
                    }
                } else {
                    // At the passive-walk point, stash the node in the backup set.
                    if ttl == self.passive_walk_length {
                        self.add_to_passive(new_node);
                    }
                    // Keep the walk going: pass it to a neighbor other than the sender.
                    if let Some(&next) = self.active.iter().find(|&&p| p != sender) {
                        actions.push(Action::Send {
                            to: next,
                            msg: Message::ForwardJoin { new_node, sender: self.me, ttl: ttl - 1 },
                        });
                    }
                }
            }
            Message::Disconnect { sender } => {
                // Peer dropped us: remove from active, keep as a backup.
                self.drop_from_active(sender);
                self.add_to_passive(sender);
                actions.push(Action::Disconnect { peer: sender });
            }
            Message::Neighbor { sender, accepted } => {
                // `accepted` flags a high-priority request that can't be refused
                // (the sender's active view is nearly empty).
                let high_priority = accepted;
                if high_priority || !self.active_is_full() {
                    let evicted = self.add_to_active(sender);
                    actions.push(Action::Connect { peer: sender });
                    actions.push(Action::Send {
                        to: sender,
                        msg: Message::NeighborReply { sender: self.me, accepted: true },
                    });
                    if let Some(old) = evicted {
                        actions.push(Action::Send { to: old, msg: Message::Disconnect { sender: self.me } });
                        actions.push(Action::Disconnect { peer: old });
                    }
                } else {
                    // Full and not urgent: politely decline.
                    actions.push(Action::Send {
                        to: sender,
                        msg: Message::NeighborReply { sender: self.me, accepted: false },
                    });
                }
            }
            Message::NeighborReply { sender, accepted } => {
                if accepted {
                    self.add_to_active(sender);
                    actions.push(Action::Connect { peer: sender });
                } else {
                    // Rejected: keep them as a backup for later.
                    self.add_to_passive(sender);
                }
            }
            Message::Shuffle { origin, sender, ttl, peers } => {
                if ttl > 0 && self.active.len() > 1 {
                    // Keep the shuffle walking to a neighbor other than the sender.
                    if let Some(&next) = self.active.iter().find(|&&p| p != sender) {
                        actions.push(Action::Send {
                            to: next,
                            msg: Message::Shuffle { origin, sender: self.me, ttl: ttl - 1, peers: peers.clone() },
                        });
                    }
                } else {
                    // Walk done: absorb their sample, reply with some of ours.
                    let reply: Vec<Id> = self.active.iter().copied().collect();
                    for p in &peers {
                        self.add_to_passive(*p);
                    }
                    actions.push(Action::Send { to: origin, msg: Message::ShuffleReplay { peers: reply } });
                }
            }
            Message::ShuffleReplay { peers } => {
                for p in peers {
                    self.add_to_passive(p);
                }
            }
            // Broadcast dissemination is handled by the gossip layer, not membership.
            Message::Broadcast { .. } => {}
        }
        actions
    }
}
