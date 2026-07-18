use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Ctrl {
	// node to tracker: "I am node <id>"
	Register { id: u32 },
	// node  tracker: (line = a serialized Message)
	Relay { to: u32, line: String },
	// trackerto node: a message forwarded from another node
	Deliver { line: String },
	// tracker to node: joining through exsisting node id

	Contact { id: u32 },
}
