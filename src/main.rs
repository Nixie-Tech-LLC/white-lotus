use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use white_lotus::{Action, Config, Message, Node};

// What we gossip: a file-hash announcement, as a simple String.
type Announcement = String;

fn main() {
	// usage: node <my_id> <my_port> <peer_id> <peer_port> [broadcast_text]
	let args: Vec<String> = env::args().collect();
	let my_id: u32 = args[1].parse().unwrap();
	let my_port: u16 = args[2].parse().unwrap();
	let peer_id: u32 = args[3].parse().unwrap();
	let peer_port: u16 = args[4].parse().unwrap();

	// address book: which address to reach each peer on
	let mut book: HashMap<u32, String> = HashMap::new();
	book.insert(peer_id, format!("127.0.0.1:{peer_port}"));

	// build the node and put the peer into its active view
	let mut node: Node<u32> = Node::new(Config::new(my_id));
	let _ = node.handle::<Announcement>(Message::Join { new_node: peer_id });

	// if a 6th argument was given, originate a broadcast of it
	if let Some(text) = args.get(5) {
		println!("[node {my_id}] broadcasting: {text}");
		let actions = node.broadcast::<Announcement>(text.clone());
		execute(&actions, &book);
	}

	// listen for incoming messages forever
	let listener = TcpListener::bind(format!("127.0.0.1:{my_port}")).unwrap();
	println!("[node {my_id}] listening on 127.0.0.1:{my_port}");
	for stream in listener.incoming() {
		let mut reader = BufReader::new(stream.unwrap());
		let mut line = String::new();
		if reader.read_line(&mut line).is_ok() && !line.trim().is_empty() {
			let msg: Message<u32, Announcement> = serde_json::from_str(line.trim()).unwrap();
			let actions = node.handle(msg);
			execute(&actions, &book);
		}
	}
}

// Carry out the Actions a node produced.
fn execute(actions: &[Action<u32, Announcement>], book: &HashMap<u32, String>) {
	for action in actions {
		match action {
			Action::Send { to, msg } => {
				if let Some(addr) = book.get(to) {
					if let Ok(mut stream) = TcpStream::connect(addr) {
						let line = serde_json::to_string(msg).unwrap();
						let _ = writeln!(stream, "{line}");
					}
				}
			}
			Action::Deliver { payload } => {
				println!(">>> DELIVERED announcement: {payload}");
			}
			Action::Connect { peer } => println!("[connect to node {peer}]"),
			Action::Disconnect { peer } => println!("[disconnect from node {peer}]"),
		}
	}
}
