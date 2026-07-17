use white_lotus::{Action, Config, Message, Node};

// A ForwardJoin whose walk has ended (ttl 0) => the node adopts the newcomer.
#[test]
fn forward_join_adopts_when_walk_ends() {
    let mut node: Node<u32> = Node::new(Config::new(1));
    let actions = node.handle::<u64>(Message::ForwardJoin {
        new_node: 9, sender: 2, ttl: 0,
    });
    assert!(actions.iter().any(|a|
        matches!(a, Action::Connect { peer } if *peer == 9)));
}
#[test]
fn forward_join_continues_the_walk() {
	let mut node: Node<u32> = Node::new(Config::new(1));
	node.handle::<u64>(Message::Join { new_node: 2 });
	node.handle::<u64>(Message::Join { new_node: 3 });
	let actions = node.handle::<u64>(Message::ForwardJoin {
        	new_node: 9, sender: 2, ttl: 5,
    });
    assert!(actions.iter().any(|a|
	matches!(a, Action::Send { msg: Message::ForwardJoin { ttl, .. }, .. } if *ttl == 4)));
}
//neighbor request node with room - accepted reply
#[test]
fn neighbor_accepted_when_there_is_room() {
    let mut node: Node<u32> = Node::new(Config::new(1));
    let actions = node.handle::<u64>(Message::Neighbor { sender: 7, accepted: false });
    assert!(actions.iter().any(|a|
        matches!(a, Action::Send { msg: Message::NeighborReply { accepted, .. }, .. } if *accepted)));
    assert!(actions.iter().any(|a|
        matches!(a, Action::Connect { peer } if *peer == 7)));
}

// A non-urgent Neighbor request at a FULL node - declined.
#[test]
fn neighbor_declined_when_full() {
    let mut node: Node<u32> = Node::new(Config::new(1));
    for peer in [2, 3, 4, 5] {
        node.handle::<u64>(Message::Join { new_node: peer });
    }
    let actions = node.handle::<u64>(Message::Neighbor { sender: 8, accepted: false });
    assert!(actions.iter().any(|a|
        matches!(a, Action::Send { msg: Message::NeighborReply { accepted, .. }, .. } if !*accepted)));
}
// A HIGH-PRIORITY Neighbor request cannot be refused, even at a full node these are nodes from your emergncy contact list
#[test]
fn high_priority_neighbor_cannot_be_refused() {
    let mut node: Node<u32> = Node::new(Config::new(1));
    for peer in [2, 3, 4, 5] {
        node.handle::<u64>(Message::Join { new_node: peer });
    }
    let actions = node.handle::<u64>(Message::Neighbor { sender: 8, accepted: true });
    assert!(actions.iter().any(|a|
        matches!(a, Action::Send { msg: Message::NeighborReply { accepted, .. }, .. } if *accepted)));
}

#[test]
fn shuffle_replies_to_origin() {
    let mut node: Node<u32> = Node::new(Config::new(1));
    node.handle::<u64>(Message::Join { new_node: 2 });
    let actions = node.handle::<u64>(Message::Shuffle {
        origin: 5, sender: 2, ttl: 0, peers: vec![10, 11],
    });
    assert!(actions.iter().any(|a|
        matches!(a, Action::Send { to, msg: Message::ShuffleReplay { .. } } if *to == 5)));
}

