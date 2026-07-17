use crate::{NodeId, Payload};
use crate::message::Message;

#[derive(Debug)]
pub enum Action<Id: NodeId, P: Payload> {
	//send a message to a specific peer
	Send { to: Id, msg: Message<Id, P> },
	//open a live connection to a new peer
	Connect { peer: Id },
	//tear down the connection to a peer we removed
	Disconnect { peer: Id },
	//give recived payload to application
	Deliver { payload: P },
}
