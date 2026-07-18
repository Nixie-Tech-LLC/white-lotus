// The white-lotus tracker: a self-hosted rendezvous + relay.
//
// Run this on a machine with a public address. Every node makes ONE outbound
// connection here (outbound works through any NAT), registers its id, and from
// then on the tracker routes protocol messages between nodes BY ID. Nodes never
// need each other's IPs - which is what makes them work across different networks.
//
// usage:  tracker [port]        (default 7000)

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use white_lotus::transport::Ctrl;

// id -> a locked write handle to that node's socket
type Registry = Arc<Mutex<HashMap<u32, Arc<Mutex<TcpStream>>>>>;

fn main() {
	let port: u16 = std::env::args()
		.nth(1)
		.and_then(|s| s.parse().ok())
		.unwrap_or(7000);
	let registry: Registry = Arc::new(Mutex::new(HashMap::new()));
	let listener = TcpListener::bind(format!("0.0.0.0:{port}")).unwrap();
	println!("[tracker] listening on 0.0.0.0:{port}");
	for stream in listener.incoming() {
		let stream = match stream {
			Ok(s) => s,
			Err(_) => continue,
		};
		let registry = Arc::clone(&registry);
		thread::spawn(move || handle(stream, registry));
	}
}

fn handle(stream: TcpStream, registry: Registry) {
	let who = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
	let writer: Arc<Mutex<TcpStream>> = Arc::new(Mutex::new(stream.try_clone().unwrap()));
	let mut reader = BufReader::new(stream);
	let mut my_id: Option<u32> = None;
	let mut line = String::new();
	loop {
		line.clear();
		match reader.read_line(&mut line) {
			Ok(0) | Err(_) => break, // node disconnected
			Ok(_) => {}
		}
		let t = line.trim();
		if t.is_empty() {
			continue;
		}
		let ctrl: Ctrl = match serde_json::from_str(t) {
			Ok(c) => c,
			Err(_) => continue,
		};
		match ctrl {
			Ctrl::Register { id } => {
				// pick an already-connected node (if any) to introduce as a contact
				let contact = {
					let reg = registry.lock().unwrap();
					reg.keys().copied().find(|&k| k != id)
				};
				registry.lock().unwrap().insert(id, Arc::clone(&writer));
				my_id = Some(id);
				println!("[tracker] node {id} registered from {who}");
				if let Some(c) = contact {
					send(&writer, &Ctrl::Contact { id: c });
				}
			}
			Ctrl::Relay { to, line: inner } => {
				let target = registry.lock().unwrap().get(&to).cloned();
				if let Some(t) = target {
					send(&t, &Ctrl::Deliver { line: inner });
				}
			}
			_ => {} // Deliver/Contact are tracker -> node only
		}
	}
	if let Some(id) = my_id {
		registry.lock().unwrap().remove(&id);
		println!("[tracker] node {id} disconnected");
	}
}

fn send(conn: &Arc<Mutex<TcpStream>>, ctrl: &Ctrl) {
	let line = serde_json::to_string(ctrl).unwrap();
	let mut s = conn.lock().unwrap();
	let _ = writeln!(s, "{line}");
}
