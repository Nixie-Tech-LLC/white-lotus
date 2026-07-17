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
}
