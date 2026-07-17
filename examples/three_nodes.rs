// A 10-second tour of white-lotus, using only the public API.
// Run it with:  cargo run --example three_nodes

use white_lotus::{Action, Config, Message, Node};

fn main() {
    // --- Node 1: learns about two peers, then originates a broadcast ---
    let mut node1 = Node::new(Config::new(1u32));
    node1.handle::<u64>(Message::Join { new_node: 2 });
    node1.handle::<u64>(Message::Join { new_node: 3 });

    println!("Node 1 broadcasts a file-hash announcement (payload = 42):");
    for action in node1.broadcast(42u64) {
        match action {
            Action::Send { to, .. } => println!("  -> forward announcement to peer {to}"),
            Action::Deliver { payload } => println!("  -> deliver payload {payload} locally"),
            other => println!("  -> {other:?}"),
        }
    }

    // --- Node 2: receives that broadcast and reacts ---
    let mut node2 = Node::new(Config::new(2u32));
    node2.handle::<u64>(Message::Join { new_node: 4 }); // node 2 knows peer 4

    println!("\nPeer 2 receives the announcement and reacts:");
    let incoming = Message::Broadcast { origin: 1, seq: 0, sender: 1, hop: 0, payload: 42u64 };
    for action in node2.handle(incoming) {
        match action {
            Action::Deliver { payload } => println!("  -> node 2 delivers payload {payload} to its app"),
            Action::Send { to, .. } => println!("  -> node 2 forwards it onward to peer {to}"),
            other => println!("  -> {other:?}"),
        }
    }

    // --- Dedup: the same announcement a second time does nothing ---
    println!("\nPeer 2 sees the SAME announcement again (id 0):");
    let duplicate = Message::Broadcast { origin: 1, seq: 0, sender: 1, hop: 0, payload: 42u64 };
    let actions = node2.handle(duplicate);
    if actions.is_empty() {
        println!("  -> ignored (already seen) - dedup works");
    }
}
