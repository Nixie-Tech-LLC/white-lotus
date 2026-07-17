use crate::{NodeId, Payload};
use crate::action::Action;
use crate::message::{Message, MessageId};

//forward to every peer except sender
//hop is incoming - sent are the hops plus 1
//origin + seq travel unchanged so the (origin, seq) id stays globally unique
pub fn forward<Id: NodeId, P: Payload>(
	me: Id,
	active_peers: &[Id],
	exclude: Option<Id>,
	origin: Id,
	seq: MessageId,
	hop: u32,
	payload: &P,
) -> Vec<Action<Id, P>> {
	let mut actions = Vec::new();
	for &peer in active_peers {
		if Some(peer) == exclude {
			continue; // never echo back to the sender
		}
		actions.push(Action::Send {
			to: peer,
			msg: Message::Broadcast {
				origin,
				seq,
				sender: me,
				hop: hop + 1,
				payload: payload.clone(),
			},
		});
	}
	actions
}
