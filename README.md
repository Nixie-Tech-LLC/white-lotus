Welcome to White Lotus

This project was inspired by the work of João Leitão's 2007 master's thesis "Gossip-Based Broadcast Protocols" developed along with Luis Rodriguiz. This Project is also heavily inspired by the open source P2P service Iroh. This project takes the theory put forward by Joao and his associates and implements HyParView and Plumtree in Rust. 
HyParView (Hybrid Partial View) in essance is how a node in a p2p network decides who it knows and their relation to that node this is referred to in the code as membership. 
Plumbtree (Epidemic Broadcast Trees) is what determines how that message spreads referred to as broadcast. 

Gossip or Epidemic Protocols were firsr introduced in 1987 in a ZEROX PARC paper "Epidemic Algorithms for Replicated Database Maintinence" (Demers et al.) which asserted that information can be spread through a network mimicing the way a rumor spreads through a people group or simularly a virus or contagion is spread. Each node distributes information to a few others in its network and in x number of rounds the information becomes fully disseminated. This differed from other information distribution protocols which origionated from a central coordinator. However just like in a game of telephone the larger the node network size the made it difficult for nodes to disseminate information effectlively. Each node knowing all other nodes (referred to as full memebership) wasnt scaleable. Each node needed to gossip to ln(n) random peers in the old modality. HyParView introduced a solution, rather than enforcing full membership for each node, HyParView instead stipulated patial membership. Each node in the network would maintain ~5 (4 +1) pers that it talks to (think of this like your emergency contact list, your family or a close knit friend group) and approximatley 30 backup nodes which fill in and repair when one of your close friend nodes dies. On top of that Plumtree 

The HyParView and Plumtree pairing run many of the popular p2p production systems today, infact Iroh uses the same basis for their own protocol. 


Think of a gossip protocol like a tree - each branch maintains leaves that fan out in successive layers (views) some leaves are closer to the trunk (node) - although this tree is a funny kind of tree where each leaf is a trunk of its own tree - sorry this metaphore is getting a little lost... Just like in many tree types when one branch dies or falls off (ie a node goes dark) the tree (the gossip protocol) simply heals over that severed limb (node) and continues to send its messages and nutrients to the other living trees - growing new trees (establishing new node connections) when and where needed. And just like a real living tree if too many branches fall of at once it causes serious problems for the life of the network - not enough nutirents is passed around and maintained for the tree to keep growing and eventually it can die. The same rings true with the nodes of a gossip protocol - if too many nodes die simultantoeusly than critical information can be lost as there is no central server to back up information on - the strength of the network is determined by an algorthim determining how fast messages can travel between eachother and the retention of the nodes in the network. 
lib.rs
Lib.rs houses the module list: this list communicate to the compiler notifying it of these 6 files:
mod config;
mod message;
mod action;
mod membership;
mod broadcast;
mod gossip;

NodeId is a marker train (a peers identity) which stands in for its public key. In order to be a NodeId one needs the ability to Copy, Eq, Ord, and Hash:

pub trait NodeId: Copy + Eq + Ord + std::hash::Hash {}
impl<T: Clone> Payload for T {}

any type that happend to have those same four abilities is automatically a NodeId
every peer must be able to hash itself in for each node (id marker) to be stored in a Hashset.

Paload is the other trait this is what nodes exchange/dissemniate when they gossip (a file-hash announcment) this is a clone of the message sent.
pub trait Payload: Clone {}
impl<T: Clone> Payload for T {}

the role of the lib.rs file is a table of contents which defines a share vocabulary accross all modules

config.rs
the config.rs file is in charge of "who the node is", "timings" and "limits." This file covers all the knobs that a single node runs with using a generic 'ID' so this config runs with any idenity type (refer back to the NodeId trait we just discussed in lib.rs above^^)

Time here refers to the time which the node ought to elapse (one second) before going to the next round - this is called pacing and it is what tells your machine how often a node should wake up and engae a round of gossip

Later we will deal with something called Locical Time- Logical time used in ordering (ie the freshness of a message - how old is this message in realtion to the other messages i have received?) - for this freshness vector we cannot use physical time since a gossip protocol is inatley a horizontal distributed system- meaning that multiple nodes may be firing receiving messages at the same time - if we were to use phycisal time here things would get very messy very fast - ergo Logical time was created through the Lamport algorithm or vector clocks to provide another way to measure the ordering of messages between nodes.  

One thing that config does is helps the node to determine how many peers a given message is distributed to - you can review this for yourself in Leito's masters thesis §3.2.2, p.32 (João Leitão, Gossip-Based Broadcast Protocols, MSc thesis, Faculdade de Ciências da Universidade de Lisboa, 2007) 

HyParView maintains activve peers and passive peers. Think of these like your emergecy contacts vs. your school or work friends. The active peers are the ones who a node maintains an open TCP connection to and the ones they always forward messages to. 
Direct quote: "to allow the use of a fanout of t without sending the gossip message back to the same node from which the message was received... partial views should have a size of t + 1." One slot is reserved for "the peer I just heard it from" (forwarding back to them is a guaranteed-wasted message), the other t slots are who you forward to.
this means you always retain four plus one active nodes per node - these are your five emergency contacts

as for passive view: log(n) - in the paper they suggest 30 passive nodes - these serve as a backup resovior if your active nodes arent responding - any active node that dies is immeidnelty replaces with a pasive node and that passive node slot is in turn replaced/remade - in the word of the lovly authors Rule: it "must be larger than log(n)" to keep the network connected through many simultaneous failures. For n = 10,000, log₂(n) ≈ 13, so 30 is a comfortable safety margin. They note the overhead is "minimal, as no connections are kept open"

fanout - fanout of four vs the classic gossip protocol of ln(n) + c - HyPar View sticks with bare minimum of 4 node fanout becuase it gets the job done. However there are tradeoffs: classic random gossip gives he network reliability with high bandwidth - each node gossips to ln(n) peers and ever message is received several times over per node. HyParView instead prioritizes the reducing redundancy by sacrificing 100% churn for each node. However this is remedied through the implementation of passive view and shuffle (which we will get more into later).  

going back to our clock, timing and sequencing questions from before: 
HyParView's core "how far / how long" parameters are not measured in seconds — they're hop counts:
- Active Random Walk Length (ARWL) = 6 — max hops a join request travels.
- Passive Random Walk Length (PRWL) = 3 — the hop at which a node gets recorded in a passive view.
- The shuffle runs on a periodic cycle, and TTLs are decremented per hop, not per second.

for citation see: All parameters together (§4.2, "Experimental Parameters"): network = 10,000 nodes · active view = 5 · passive view = 30 · ARWL = 6 · PRWL = 3 · shuffle exchanges 3 from active + 4 from passive · fanout = 4

message.rs

message.rs is the wire format: the exhaustive list of things one peer can send another. HyParView needs two families of messages — membership control (build & repair the overlay) and broadcast (actually disseminate your announcements).Plumtree adds a third small family: the tree-repair control messages IHave, Graft, and Prune. Each broadcast bears a unique id (the(origin, seq) pair) so that peers can identify a message theyve already seen and avoid duplicating messages when dissiminating them to other peers.

the enum message is everything one node can say to another node - the node id and payload are what they gossip between eachother - this resuses the vocabulary we defined in the lib.rs file at the beguinning
  
Membership

Membership starts by defining a nodes hyparview membership state: the two views (active and passive) and their size limits (active: fanout + 1 and passive at ~30). Memebership also handles the self healing: 
- Join: a newcommer
- ForwardJoin: the random walk that carries a join accross the overlay 
- Neighbor/NeighborRelay: promoting a passive peer to an active peer slot in the case of an active peer demise
- Shuffle/ShuffleReplay: refreshes the passive backups to ensure all replacemtns are live 
 

Action.rs

 Send carries a whole Message (reusing message.rs) plus who it goes to. Connect/Disconnect manage live links. Deliver hands a received payload up to your app.

Broadcast (Plumtree implememntation)
Plumtree (thesis §3.3 "Eager Push Strategy" and §3.4 "Tree Strategy"; also Leitão, Pereira & Rodrigues, Epidemic Broadcast Trees, IEEE SRDS 2007):
- eager peers get the FULL payload (these links form a spanning tree — efficient)
- lazy peers get only a tiny IHave(id) announcement
- PRUNE trims a redundant eager link to lazy; GRAFT (after a short grace-period timer) heals a gap when a node dies.

gossip.rs
All together now!  
 
gossip.rs is the conductor that turns all your other files into one working node: it holds a Config (your settings), a Membership (the active/passive peer views),the plumbtree state (a "seen" set, a payload set for answering GRATs, the eager/lazy split, and the missing message timers), then exposes three doors: broadcast() to start spreading a Payload, tixk(now) to fire the GAFT timers, and handle() to react to an incoming Message. When a message arrives, gossip decides who does the work: broadcast messages and plumtree prorocol it handles itself (skip if already seen, Deliver to the app, then hand off to broadcast.rs to forward to the active peers), while membership messages (Join, Disconnect, etc.) get passed down to membership.rs. Everything it does comes back as a list of Actions, so gossip is the brain that ties config, membership, message, action, and broadcast together without ever touching the network itself.

Testing
as seen in example.rs

white-lotus is validated in fur  complementary layers, mirroring how the HyParView protocol itself was evaluated. First, an integration test suite (tests/simulation.rs) spins up entire networks of nodes — 5, 30, and 40 at a time — inside a single process, wires them into an overlay, broadcasts a message, and asserts that it reaches every node exactly once with no duplicate deliveries; because the protocol logic is pure (each node returns a list of intended actions rather than performing network I/O), we can simulate networks of arbitrary size deterministically and instantly, exactly as Leitão's thesis simulated 10,000 nodes rather than deploying 10,000 machines. Second, a runnable example (examples/three_nodes.rs) exercises the same public API a real user would call and prints the protocol in action, serving simultaneously as living, compiler-checked documentation and as a continuous check that the public interface stays clean and ergonomic. Third, the code is deployed to real Raspberry Pi hardware to validate the connection and serialization layer under genuine network conditions — latency, failures, and churn that simulation cannot fully reproduce — scaling from a handful of devices up to a 40-node fleet. This separation lets simulation prove correctness at scale while hardware proves real-world plumbing, so each layer tests what it is best suited to catch.

self healing tree 
  - ForwardJoin — the random walk (ARWL/PRWL) that lets a new node wire itself into the overlay from a single contact
  - Neighbor/NeighborReply — how a dead active peer gets replaced from the passive view (the actual "heals over the severed limb")
  - Shuffle — keeping the passive backup fresh so replacements are live peers


The key insight for testing Pass 4

Every new handler's behavior is observable in the Actions it returns — you don't need to peek inside the private views. So the setup is: feed the node one message, then check what Actions came back. Deterministic, no new API needed, no fragile emergent dynamics.

For each new message type, there's a clear observable outcome:

┌────────────────────────┬────────────────────────────────┐
│      feed it this      │    expect this Action back     │
├────────────────────────┼────────────────────────────────┤
│ ForwardJoin { ttl: 0,  │ Connect { peer: new_node } —   │
│ .. }                   │ walk ended, node adopted       │
├────────────────────────┼────────────────────────────────┤
│ ForwardJoin { ttl: 5,  │ Send { ForwardJoin { ttl: 4 }  │
│ .. } (with peers)      │ } — walk continues             │
├────────────────────────┼────────────────────────────────┤
│ Neighbor { accepted:   │ Send { NeighborReply {         │
│ false } (room to       │ accepted: true } } — promotion │
│ spare)                 │  accepted                      │
├────────────────────────┼────────────────────────────────┤
│                        │ Send { ShuffleReplay } back to │
│ Shuffle { ttl: 0, .. } │  origin — sample absorbed &    │
│                        │ replied                        │
└────────────────────────┴────────────────────────────────┘


A Message is a Rust value in memory; to send it over TCP it has to become a stream of bytes, and become a Message again on the other end. The standard Rust tool for this is serde (serialize/deserialize), and we'll use JSON as the format at first because it's human-readable

an example:
How to read the command (so it's clear which is which)

./target/debug/white-lotus  <my-id>  <my-port>  <peer-id>  <peer-address>
                    Mac:        1        9001        2       10.7.23.32:9002  (the Pi)
                    Pi:         2        9002        1       10.7.23.33:9001  (the Mac)

transport.rs
// The little envelope spoken between a node and the tracker - one JSON per line.
// The tracker routes protocol messages between nodes by id, so nodes never need
// each other's addresses (which is what lets them work across any network/NAT:
// every node only makes an OUTBOUND connection to the tracker).

bin/tracker
The white-lotus tracker: a self-hosted rendezvous + relay.
//
// Run this on a machine with a public address. Every node makes ONE outbound
// connection here (outbound works through any NAT), registers its id, and from
// then on the tracker routes protocol messages between nodes BY ID. Nodes never
// need each other's IPs - which is what makes them work across different networks.
//
// usage:  tracker [port]        (default 7000)

the archetecture of white lotus
              ┌───────────────────────────────────────────────────┐
              │   DOCKER bridge network  ·  172.20.0.x  ·  no NAT   │
              │                                                     │
              │   ┌────────┐  ┌────────┐  ┌────────┐                │
              │   │ node1  │  │ node2  │  │ node3  │  … (scale N)    │
              │   └───┬────┘  └───┬────┘  └───┬────┘      │
              │       │ outbound  │  outbound │  outbound      │
              │       └───────────┼───────────┘      │
              │                   ▼      │
              │             ┌───────────┐      │
              │             │  TRACKER  │  routes messages BYID     │
              │             │  :7000    │  introducesnewcomers      │
              │             └───────────┘      │
└───────────────────────────────────────────────────┘
     Every node dials `tracker:7000`. No IPs, no peer lists —the tracker
     knows where each id lives, so nodes reference each otherby id alone.

2. Inside one node (the two layers that matter)

   ┌─────────────────────────────────────────────────────────────┐
   │  RUNTIME  (src/main.rs)  — the ONLY part that touches I/O     │
   │                                                               │
   │   keyboard thread ─► node.broadcast(text) ─┐                  │
   │   ticker thread   ─► node.tick(now)        ├─► Vec<Action>    │
   │   network thread  ─► node.handle(msg)      ┘        │         │
   │                                                     ▼         │
   │                        execute(actions):                      │
   │                          Send    → Ctrl::Relay → tracker       │
   │                          Deliver → println!                    │
   │                          Connect/Disconnect → (no-op)          │
   └───────────────────────────────┬─────────────────────────────┘
                                    │ returns Actions, never does I/O
   ┌───────────────────────────────▼─────────────────────────────┐
   │  PURE CORE  (the library)  — no sockets, fully testable       │
   │                                                               │
   │   Node<Id, P>            (gossip.rs)   ── the conductor        │
   │    ├─ Config<Id>         (config.rs)   who am I, fanout, TTLs  │
   │    ├─ Membership<Id>     (membership.rs)  HyParView views      │
   │    │      active view (fanout+1) · passive view (~30)          │
   │    │      Join / ForwardJoin / Neighbor / Shuffle (self-heal)  │
   │    ├─ broadcast fns      (broadcast.rs)   Plumtree             │
   │    │      eager_push (full payload) · lazy_push (IHave)        │
   │    ├─ seen  · cache · lazy · missing   (dedup + GRAFT timers)  │
   │    └─ speaks Message (message.rs) · emits Action (action.rs)   │
   └───────────────────────────────────────────────────────────────┘i
