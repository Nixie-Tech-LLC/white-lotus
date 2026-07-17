use crate::{NodeId, Payload};
use serde::{Serialize, Deserialize};

// unique message id for broadcas
pub type MessageId = u64;

#[derive(Debug, Serialize, Deserialize)]
pub enum Message<Id: NodeId, P:Payload> {
	// new node - knock knock let me in to the overlay
	Join { new_node: Id },
	// spread word of a join accross a random walk - ttl starts the ARWL and drops by 1 node each hop in the network 
	ForwardJoin { new_node: Id, sender: Id, ttl: u32 },
	//keeps links symetric
	Disconnect { sender: Id }, 
	//This sets a boolean that gives a sender with a nearly empty active view meaning the message cannot be refused placing the receiver into an active view slot - this prevents nodes being isolated in the network with no no one to communicate with 
	Neighbor { sender : Id, accepted: bool },
	//accepts or rejects neighbor request
	NeighborReply { sender: Id, accepted: bool }, 
	//periodically passivley view refresh which alsl carries ttl
	Shuffle { origin: Id, sender: Id, ttl: u32, peers: Vec<Id> },
	//replay shuffle to peers
	ShuffleReplay { peers: Vec<Id> }, 
	// this is the disseminiation - file hash announcment plus id and hop counter bounded by Confiug max rounds
	Broadcast { origin: Id, seq: MessageId, sender: Id, hop: u32, payload: P },
}
