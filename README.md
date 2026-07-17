Think of a gossip protocol like a tree - each branch maintains leaves that fan out in successive layers (views) some leaves are closer to the trunk (node) - although this tree is a funny kind of tree where each leaf is a trunk of its own tree - sorry this metaphore is getting a little lost... Just like in many tree types when one branch dies or falls off (ie a node goes dark) the tree (the gossip protocol) simply heals over that severed limb (node) and continues to send its messages and nutrients to the other living trees - growing new trees (establishing new node connections) when and where needed. And just like a real living tree if too many branches fall of at once it causes serious problems for the life of the network - not enough nutirents is passed around and maintained for the tree to keep growing and eventually it can die. The same rings true with the nodes of a gossip protocol - if too many nodes die simultantoeusly than critical information can be lost as there is no central server to back up information on - the strength of the network is determined by an algorthim determining how fast messages can travel between eachother and the retention of the nodes in the network. 

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

One thing that config does is helps the node to determine how many peers a given message is distributed to - you can review this for yourself in Leito's masters thesis §3.2.2, p.32 [add citation later]

HyParView maintains activve peers and passive peers. Think of these like your emergecy contacts vs. your school or work friends. The active peers are the ones who a node maintains an open TCP connection to and the ones they always forward messages to. 
Direct quote: "to allow the use of a fanout of t without sending the gossip message back to the same node from which the message was received... partial views should have a size of t + 1." One slot is reserved for "the peer I just heard it from" (forwarding back to them is a guaranteed-wasted message), the other t slots are who you forward to.
this means you always retain four plus one active nodes per node - these are your five emergency contacts

as for passive view: log(n) - in the paper they suggest 30 passive nodes - these serve as a backup resovior if your active nodes arent responding - any active node that dies is immeidnelty replaces with a pasive node and that passive node slot is in turn replaced/remade - in the word of the lovly authors Rule: it "must be larger than log(n)" to keep the network connected through many simultaneous failures. For n = 10,000, log₂(n) ≈ 13, so 30 is a comfortable safety margin. They note the overhead is "minimal, as no connections are kept open"

fanout - fanout of four vs the classic gossip protocol of ln(n) + c - HyPar View sticks with bare minimum of 4 node fanout becuase it gets the job done - the trade offs are 

going back to our clock, timing and sequencing questions from before: 
HyParView's core "how far / how long" parameters are not measured in seconds — they're hop counts:
- Active Random Walk Length (ARWL) = 6 — max hops a join request travels.
- Passive Random Walk Length (PRWL) = 3 — the hop at which a node gets recorded in a passive view.
- The shuffle runs on a periodic cycle, and TTLs are decremented per hop, not per second.

for citation see: All parameters together (§4.2, "Experimental Parameters"): network = 10,000 nodes · active view = 5 · passive view = 30 · ARWL = 6 · PRWL = 3 · shuffle exchanges 3 from active + 4 from passive · fanout = 4

message.rs

message.rs is the wire format: the exhaustive list of things one peer can send another. HyParView needs two families of messages — membership control (build & repair the overlay) and broadcast (actually disseminate your announcements).
each broadcase bears a unique id sot that peers can identify a message theyve already seen and avoid duplicating messages when dissiminating them to other peers.

the enum message is everything one node can say to another node - the node id and payload are what they gossip between eachother - this resuses the vocabulary we defined in the lib.rs file at the beguinning
  
Membership

Membership starts by defining a nodes hyparview membership state: the two and their size limits -  
 
