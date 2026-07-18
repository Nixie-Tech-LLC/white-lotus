mod config;
mod message;
mod action;
mod membership;
mod broadcast;
mod gossip;
pub mod transport;

// The public API a user of this library works with. Internal modules stay
// private; these re-exports are the clean surface (e.g. `use white_lotus::Node;`).
pub use config::Config;
pub use gossip::Node;
pub use message::{Message, MessageId};
pub use action::Action;

// A peer's name / id. Placeholder for the node's public key.
pub trait NodeId: Copy + Eq + Ord + std::hash::Hash {}
impl<T: Copy + Eq + Ord + std::hash::Hash> NodeId for T {}

// What we gossip: an opaque payload (the file-hash announcement).
pub trait Payload: Clone {}
impl<T: Clone> Payload for T {}
