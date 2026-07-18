use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

use white_lotus::transport::Ctrl;
use white_lotus::{Action, Config, Message, Node};

// What we gossip: a file-hash announcement, as a simple String.
type Announcement = String;
// A shared, locked write handle to our single connection to the tracker.
type Conn = Arc<Mutex<TcpStream>>;

fn main() {
	// usage: white-lotus <my_id> <tracker_host:port>
	//   local:   white-lotus 1 127.0.0.1:7000
	//   docker:  white-lotus 1 tracker:7000     (reach the tracker by name)
	let args: Vec<String> = std::env::args().collect();
	let my_id: u32 = args[1].parse().expect("need a numeric node id");
	let tracker_addr = args[2].clone();

	// ONE outbound connection to the tracker - outbound works through any NAT,
	// and the tracker routes to every other node by id, so we need no peer list.
	let stream = TcpStream::connect(&tracker_addr)
		.unwrap_or_else(|e| panic!("could not reach tracker {tracker_addr}: {e}"));
	let conn: Conn = Arc::new(Mutex::new(stream.try_clone().unwrap()));

	// tell the tracker who we are
	send_ctrl(&conn, &Ctrl::Register { id: my_id });
	println!("[node {my_id}] connected to tracker {tracker_addr}  (type a message + Enter)");

	let node: Arc<Mutex<Node<u32, Announcement>>> =
		Arc::new(Mutex::new(Node::new(Config::new(my_id))));

	// --- keyboard thread: type a line -> broadcast it ---
	{
		let node = Arc::clone(&node);
		let conn = Arc::clone(&conn);
		thread::spawn(move || {
			let stdin = std::io::stdin();
			for line in stdin.lock().lines() {
				let text = match line {
					Ok(t) => t,
					Err(_) => break,
				};
				if text.trim().is_empty() {
					continue;
				}
				let actions = node.lock().unwrap().broadcast(text);
				execute(&actions, &conn);
			}
		});
	}

	// --- ticker thread: drive the Plumtree GRAFT timers ---
	{
		let node = Arc::clone(&node);
		let conn = Arc::clone(&conn);
		thread::spawn(move || {
			let start = std::time::Instant::now();
			loop {
				thread::sleep(std::time::Duration::from_millis(50));
				let now = start.elapsed().as_millis() as u64;
				let actions = node.lock().unwrap().tick(now);
				execute(&actions, &conn);
			}
		});
	}

	// --- main thread: read from the tracker and react ---
	let mut reader = BufReader::new(stream);
	let mut line = String::new();
	loop {
		line.clear();
		match reader.read_line(&mut line) {
			Ok(0) | Err(_) => {
				println!("[node {my_id}] tracker connection closed");
				break;
			}
			Ok(_) => {}
		}
		let trimmed = line.trim();
		if trimmed.is_empty() {
			continue;
		}
		let ctrl: Ctrl = match serde_json::from_str(trimmed) {
			Ok(c) => c,
			Err(_) => continue,
		};
		match ctrl {
			// a message forwarded to us from another node
			Ctrl::Deliver { line: inner } => {
				if let Ok(msg) = serde_json::from_str::<Message<u32, Announcement>>(&inner) {
					let actions = node.lock().unwrap().handle(msg);
					execute(&actions, &conn);
				}
			}
			// the tracker introduced us to an existing node: join through it
			Ctrl::Contact { id } => {
				println!("[node {my_id}] joining the overlay via node {id}");
				let actions = node.lock().unwrap().handle(Message::Join { new_node: id });
				execute(&actions, &conn);
				// announce ourselves so that node adds us back and spreads the join
				let join = Message::<u32, Announcement>::Join { new_node: my_id };
				send_ctrl(&conn, &Ctrl::Relay {
					to: id,
					line: serde_json::to_string(&join).unwrap(),
				});
			}
			_ => {}
		}
	}
}

// Carry out the Actions a node produced, by relaying them through the tracker.
fn execute(actions: &[Action<u32, Announcement>], conn: &Conn) {
	for action in actions {
		match action {
			Action::Send { to, msg } => {
				let line = serde_json::to_string(msg).unwrap();
				send_ctrl(conn, &Ctrl::Relay { to: *to, line });
			}
			Action::Deliver { payload } => println!(">>> DELIVERED announcement: {payload}"),
			// Connect/Disconnect are no-ops here: the tracker handles reachability.
			Action::Connect { .. } | Action::Disconnect { .. } => {}
		}
	}
}

fn send_ctrl(conn: &Conn, ctrl: &Ctrl) {
	let line = serde_json::to_string(ctrl).unwrap();
	let mut s = conn.lock().unwrap();
	let _ = writeln!(s, "{line}");
}
