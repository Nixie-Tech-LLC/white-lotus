Lib.rs houses the module list - this list communicate to the compiler notifying it of these 6 files:
mod config;
mod message;
mod action;
mod membership;
mod broadcast;
mod gossip;

peers name/NodeId marker trait - this is the peers public key placeholder
pub trait NodeId: Copy + Eq + Ord + std::hash::Hash {}

to be a node you must have four abilityies
impl<T: Clone> Payload for T {}

any type that happend to have those same four abilities is automatically a N$
every peer must be able to hash itself in for each node (id marker) to be st$

the role of the lib.rs file is a table of contents which defines a share vocabulary accross all modules

the config.rs file is in charge of "who the node is", "timings" and "limits"

this file covers all the knobs that a single node runs with using a generic 'ID' so this config runs with any idenity type (refer back to the NodeId trait we just discussed in lib.rs above^^)

time here refers to the time which the node ought to elapse (one second) before going to the next round - this is called pacing and it is what tells your machine how often a node should wake up and engae a round of gossip

later we will deal with something called Locical Time- Logical time used in ordering (ie the freshness of a message - how old is this message in realtion to the other messages i have received?) - for this freshness vector we cannot use physical time since a gossip protocol is inatley a horizontal distributed system- meaning that multiple nodes may be firing receiving messages at the same time - if we were to use phycisal time here things would get very messy very fast - ergo Logical time was created through the Lamport algorithm or vector clocks to provide another way to measure the ordering of messages between nodes. 

Thanks for entertaining my tangent - moving on back to the config at hand 



 
