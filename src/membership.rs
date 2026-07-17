use std::collections::HashSet;
use crate::NodeId;

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
}

impl<Id: NodeId> Membership<Id> {
	//start with empty views (sized per specs in essay)
	pub fn new(me: Id, fanout: usize, passive_capasity: usize) -> Self
{
 Membership {
            me,
            active: HashSet::new(),
            passive: HashSet::new(),
            active_capasity: fanout + 1,
            passive_capasity,
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
}
