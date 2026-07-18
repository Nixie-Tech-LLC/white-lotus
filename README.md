Welcome to White Lotus

for quick start:

cargo build          # compile
cargo test           # run all 18 tests (no hardware needed)

 run it live: start the relay, then self-assembling nodes
 
./target/debug/tracker 7000
./target/debug/white-lotus 1 127.0.0.1:7000
./target/debug/white-lotus 2 127.0.0.1:7000
etc etc for each device...

 …or the whole mesh in containers
docker compose up --build


This project was inspired by the work of Joao Leitao's 2007 master's thesis "Gossip-Based Broadcast Protocols" developed along with Luis Rodrigues. This project is also heavily inspired by the open source P2P service Iroh. This project takes the theory put forward by Joao and his associates and implements HyParView and Plumtree in Rust.

HyParView (Hybrid Partial View) in essence is how a node in a p2p network decides who it knows and their relation to that node. This is referred to in the code as membership.

Plumtree (Epidemic Broadcast Trees) is what determines how that message spreads, referred to as broadcast.

Since humans began to innovate on their life they have pulled inspiration from the world around them. Spears to enlongate their reach and efficieny in hunting clothes to protect themselves from the elements like this fur or scales. Similarly Gossip or Epidemic Protocols were first introduced in 1987 in a Xerox PARC paper "Epidemic Algorithms for Replicated Database Maintenance" (Demers et al.), which asserted that information can be spread through a network mimicking the way a rumor spreads through a people group, or similarly the way a virus or contagion is spread. Each node distributes information to a few others in its network, and in some number of rounds the information becomes fully disseminated. This differed from other information distribution protocols, which originated from a central coordinator. However, just like in a game of telephone, the larger the node network size the more difficult it became for nodes to disseminate information effectively. Each node knowing all other nodes (referred to as full membership) was not scalable. Each node needed to gossip to ln(n) random peers in the old modality. HyParView introduced a solution: rather than enforcing full membership for each node, HyParView instead stipulated partial membership. Each node in the network would maintain about 5 (4 plus 1) peers that it talks to (think of this like your emergency contact list, your family or a close knit friend group) and approximately 30 backup nodes which fill in and repair when one of your close friend nodes dies.

The HyParView and Plumtree pairing runs many of the popular p2p production systems today. In fact, Iroh uses the same basis for their own protocol.

Think of a gossip protocol like a tree. Each branch maintains leaves that fan out in successive layers (views), and some leaves are closer to the trunk (node), although this tree is a funny kind of tree where each leaf is a trunk of its own tree. Sorry, this metaphor is getting a little lost. Just like in many tree types, when one branch dies or falls off (a node goes dark) the tree (the gossip protocol) simply heals over that severed limb (node) and continues to send its messages and nutrients to the other living trees, growing new trees (establishing new node connections) when and where needed. And just like a real living tree, if too many branches fall off at once it causes serious problems for the life of the network. Not enough nutrients are passed around and maintained for the tree to keep growing, and eventually it can die. The same rings true with the nodes of a gossip protocol. If too many nodes die simultaneously then critical information can be lost, as there is no central server to back up information on. The strength of the network is determined by an algorithm determining how fast messages can travel between each other and the retention of the nodes in the network.

lib.rs

Lib.rs houses the module list. This list communicates to the compiler, notifying it of these 6 files: mod config, mod message, mod action, mod membership, mod broadcast, and mod gossip.

NodeId is a marker trait (a peer's identity) which stands in for its public key. In order to be a NodeId, one needs the ability to Copy, Eq, Ord, and Hash: pub trait NodeId: Copy + Eq + Ord + std::hash::Hash {}. Any type that happens to have those same four abilities is automatically a NodeId. Every peer must be able to hash itself in, for each node (id marker) to be stored in a Hashset.

Payload is the other trait. This is what nodes exchange and disseminate when they gossip (a file-hash announcement), and it is a clone of the message sent: pub trait Payload: Clone {} and impl<T: Clone> Payload for T {}.

The role of the lib.rs file is a table of contents which defines a shared vocabulary across all modules.

config.rs

The config.rs file is in charge of "who the node is," "timings," and "limits." This file covers all the knobs that a single node runs with, using a generic ID so this config runs with any identity type (refer back to the NodeId trait discussed in lib.rs above).

Time here refers to the time which the node ought to elapse (one second) before going to the next round. This is called pacing, and it is what tells your machine how often a node should wake up and engage a round of gossip.

Later we will deal with something called Logical Time. Logical time is used in ordering (the freshness of a message: how old is this message in relation to the other messages I have received?). For this freshness vector we cannot use physical time, since a gossip protocol is innately a horizontal distributed system, meaning that multiple nodes may be firing and receiving messages at the same time. If we were to use physical time here, things would get very messy very fast. Therefore Logical time was created, through the Lamport algorithm or vector clocks, to provide another way to measure the ordering of messages between nodes.

One thing that config does is help the node determine how many peers a given message is distributed to. You can review this for yourself in Leitao's master's thesis, section 3.2.2, page 32 (Joao Leitao, Gossip-Based Broadcast Protocols, MSc thesis, Faculdade de Ciencias da Universidade de Lisboa, 2007).

HyParView maintains active peers and passive peers. Think of these like your emergency contacts versus your school or work friends. The active peers are the ones a node maintains an open TCP connection to, and the ones they always forward messages to.

Direct quote: "to allow the use of a fanout of t without sending the gossip message back to the same node from which the message was received... partial views should have a size of t + 1." One slot is reserved for the peer you just heard it from (forwarding back to them is a guaranteed wasted message), and the other t slots are who you forward to. This means you always retain four plus one active nodes per node. These are your five emergency contacts.

As for the passive view, log(n): in the paper they suggest 30 passive nodes. These serve as a backup reservoir if your active nodes are not responding. Any active node that dies is immediately replaced with a passive node, and that passive node slot is in turn replaced and remade. In the words of the lovely authors, the rule is that it "must be larger than log(n)" to keep the network connected through many simultaneous failures. For n = 10,000, log2(n) is approximately 13, so 30 is a comfortable safety margin. They note the overhead is "minimal, as no connections are kept open."

Fanout: a fanout of four versus the classic gossip protocol of ln(n) + c. HyParView sticks with a bare minimum of 4 node fanout. However there are tradeoffs. Classic random gossip gives the network reliability with high bandwidth, since each node gossips to ln(n) peers and every message is received several times over per node. HyParView instead prioritizes reducing redundancy by sacrificing 100% churn for each node. However this is remedied through the implementation of passive view and shuffle (which we will get more into later).

Going back to our clock, timing, and sequencing questions from before: HyParView's core "how far / how long" parameters are not measured in seconds, they are hop counts. The Active Random Walk Length (ARWL) is 6, the max hops a join request travels. The Passive Random Walk Length (PRWL) is 3, the hop at which a node gets recorded in a passive view. The shuffle runs on a periodic cycle, and TTLs are decremented per hop, not per second.

For citation, see all parameters together (section 4.2, "Experimental Parameters"): network = 10,000 nodes, active view = 5, passive view = 30, ARWL = 6, PRWL = 3, shuffle exchanges 3 from active plus 4 from passive, fanout = 4.

message.rs

message.rs is the wire format: the exhaustive list of things one peer can send another. HyParView needs two families of messages, membership control (build and repair the overlay) and broadcast (actually disseminate your announcements). Plumtree adds a third small family: the tree-repair control messages IHave, Graft, and Prune. Each broadcast bears a unique id (the (origin, seq) pair) so that peers can identify a message they have already seen and avoid duplicating messages when disseminating them to other peers.

The enum Message is everything one node can say to another node. The node id and payload are what they gossip between each other. This reuses the vocabulary we defined in the lib.rs file at the beginning.

Membership

Membership starts by defining a node's HyParView membership state: the two views (active and passive) and their size limits (active is fanout plus 1, and passive is around 30). Membership also handles the self-healing. Join is a newcomer. ForwardJoin is the random walk that carries a join across the overlay. Neighbor and NeighborRelay handle promoting a passive peer to an active peer slot in the case of an active peer's demise. Shuffle and ShuffleReplay refresh the passive backups to ensure all replacements are live.

action.rs

Send carries a whole Message (reusing message.rs) plus who it goes to. Connect and Disconnect manage live links. Deliver hands a received payload up to your app.

Broadcast (Plumtree implementation)

Plumtree (thesis section 3.3 "Eager Push Strategy" and section 3.4 "Tree Strategy"; also Leitao, Pereira and Rodrigues, Epidemic Broadcast Trees, IEEE SRDS 2007): eager peers get the full payload, and these links form a spanning tree, which is efficient. Lazy peers get only a tiny IHave(id) announcement. PRUNE trims a redundant eager link to lazy, and GRAFT (after a short grace-period timer) heals a gap when a node dies.

gossip.rs

All together now. gossip.rs is the conductor that turns all your other files into one working node. It holds a Config (your settings), a Membership (the active/passive peer views), and the Plumtree state (a "seen" set, a payload set for answering GRAFTs, the eager/lazy split, and the missing message timers). It then exposes three doors: broadcast() to start spreading a Payload, tick(now) to fire the GRAFT timers, and handle() to react to an incoming Message. When a message arrives, gossip decides who does the work. Broadcast messages and the Plumtree protocol it handles itself (skip if already seen, Deliver to the app, then hand off to broadcast.rs to forward to the active peers), while membership messages (Join, Disconnect, and so on) get passed down to membership.rs. Everything it does comes back as a list of Actions, so gossip is the brain that ties config, membership, message, action, and broadcast together without ever touching the network itself.

Testing

As seen in example.rs, white-lotus is validated in four complementary layers, mirroring how the HyParView protocol itself was evaluated. First, an integration test suite (tests/simulation.rs) spins up entire networks of nodes, 5, 30, and 40 at a time, inside a single process, wires them into an overlay, broadcasts a message, and asserts that it reaches every node exactly once with no duplicate deliveries. Because the protocol logic is pure (each node returns a list of intended actions rather than performing network I/O), we can simulate networks of arbitrary size deterministically and instantly, exactly as Leitao's thesis simulated 10,000 nodes rather than deploying 10,000 machines. Second, a runnable example (examples/three_nodes.rs) exercises the same public API a real user would call and prints the protocol in action, serving simultaneously as living, compiler-checked documentation and as a continuous check that the public interface stays clean and ergonomic. Third, the code is deployed to real Raspberry Pi hardware to validate the connection and serialization layer under genuine network conditions, latency, failures, and churn that simulation cannot fully reproduce, scaling from a handful of devices up to a 40-node fleet. This separation lets simulation prove correctness at scale while hardware proves real-world plumbing, so each layer tests what it is best suited to catch.

Self-healing tree

ForwardJoin is the random walk (ARWL/PRWL) that lets a new node wire itself into the overlay from a single contact. Neighbor and NeighborReply are how a dead active peer gets replaced from the passive view, the actual healing over the severed limb. Shuffle keeps the passive backup fresh so replacements are live peers.

The key insight for testing (Pass 4)

Every new handler's behavior is observable in the Actions it returns. You don't need to peek inside the private views. So the setup is: feed the node one message, then check what Actions came back. It is deterministic, needs no new API, and has no fragile emergent dynamics.

For each new message type there is a clear observable outcome. Feeding a node ForwardJoin with ttl 0 should return a Connect action with the new node as peer, meaning the walk ended and the node was adopted. Feeding ForwardJoin with ttl 5 (with peers) should return a Send action carrying ForwardJoin with ttl 4, meaning the walk continues. Feeding Neighbor with accepted false (when there is room to spare) should return a Send action carrying NeighborReply with accepted true, meaning the promotion was accepted. Feeding Shuffle with ttl 0 should return a Send action carrying ShuffleReplay back to the origin, meaning the sample was absorbed and replied to.

Serialization

A Message is a Rust value in memory. To send it over TCP it has to become a stream of bytes, and become a Message again on the other end. The standard Rust tool for this is serde (serialize/deserialize), and we will use JSON as the format because it is human-readable and realitivley standard.

transport.rs

transport.rs is the little envelope spoken between a node and the tracker, one JSON per line. The tracker routes protocol messages between nodes by id, so nodes never need each other's addresses, which is what lets them work across any network or NAT: every node only makes an outbound connection to the tracker.

bin/tracker

The white-lotus tracker is a self-hosted rendezvous and relay. Run this on a machine with a public address. Every node makes one outbound connection here (outbound works through any NAT), registers its id, and from then on the tracker routes protocol messages between nodes by id. Nodes never need each other's IPs, which is what makes them work across different networks. Usage: tracker followed by an optional port (default 7000).
