// Proves a Message can be turned into bytes (JSON) and rebuilt intact -
// the foundation of sending messages between machines over a network.

use white_lotus::Message;

#[test]
fn broadcast_message_round_trips_through_json() {
	// Build a message, like one a node would send.
	let original: Message<u32, String> = Message::Broadcast {
		origin: 1,
		seq: 7,
		sender: 1,
		hop: 2,
		payload: String::from("filehash-abc123"),
	};

	// Serialize it to a JSON string (this is what would go on the wire)...
	let wire: String = serde_json::to_string(&original).unwrap();

	// ...and deserialize it back into a Message on the "other end".
	let received: Message<u32, String> = serde_json::from_str(&wire).unwrap();

	// Message has no PartialEq, so compare their debug representations.
	assert_eq!(format!("{:?}", original), format!("{:?}", received));
}

#[test]
fn a_membership_message_round_trips_too() {
	let original: Message<u32, String> = Message::Shuffle {
		origin: 5,
		sender: 2,
		ttl: 3,
		peers: vec![10, 11, 12],
	};

	let wire = serde_json::to_string(&original).unwrap();
	let received: Message<u32, String> = serde_json::from_str(&wire).unwrap();

	assert_eq!(format!("{:?}", original), format!("{:?}", received));
}
