mod config;
mod message;
mod action;
mod membership;
mod broadcast;
mod gossip;

// A peer's name / id. Placeholder for the node's public key.
pub trait NodeId: Copy + Eq + Ord + std::hash::Hash {}
impl<T: Copy + Eq + Ord + std::hash::Hash> NodeId for T {}

// What we gossip: an opaque payload (the file-hash announcement).
pub trait Payload: Clone {}
impl<T: Clone> Payload for T {}
