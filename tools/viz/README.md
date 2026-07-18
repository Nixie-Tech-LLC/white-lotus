# Overlay visualizer

Watch HyParView membership and Plumtree broadcast run, step by step, on a real
overlay driven by the actual `src/` code.

## Use it

```sh
cargo test --test visualize -- --ignored --nocapture   # writes target/viz/*.jsonl
```

Open `tools/viz/index.html` in a browser and drop a trace onto it. No build step,
no server, no dependencies — it is one self-contained file.

Controls: space plays/pauses, arrow keys step, the scrubber seeks. The filter
drops down to membership-only or Plumtree-only traffic.

## Scenarios

| Test | Shows |
|---|---|
| `viz_tree_formation` | 30-node ring, one broadcast. The eager spanning tree emerging as duplicates trigger PRUNEs. Ends at exactly 29 eager edges — a spanning tree over 30 nodes. |
| `viz_organic_join` | Nodes joining one at a time through the real `Join`/`ForwardJoin` walk. |
| `viz_lazy_repair` | IHave → GRAFT recovery after the eager payload is dropped in flight. |
| `viz_eviction` | Active-view overflow on a star hub, forcing demotions. |

## What you are looking at

Trust levels differ by layer, and the UI distinguishes them:

- **Messages, actions, active views — ground truth.** The harness owns the
  message queue, and reads active views directly from each node.
- **Eager/lazy edge split — derived** from observed PRUNE/GRAFT/GOSSIP traffic,
  mirroring `gossip.rs:143`/`:193`/`:211`. Solid = eager, dashed = lazy.
- **Passive view — not observable.** Drawn as an empty grey ring, never guessed
  at. See below.

### How active views are read

`tests/` can only reach the public API, and `Membership` is private — so there is
no accessor for a node's views. Rather than reimplement membership logic in the
harness (which would drift from `src/` and then misrepresent the algorithm), the
harness uses a probe built from the protocol itself.

`membership.rs:176-192`: a `Shuffle` with `ttl == 0` falls to the else branch,
which replies `ShuffleReplay { peers: <the active view verbatim> }` and absorbs
whatever peers you sent. Send an empty peer list and the absorb loop runs zero
times — so it is a pure read that touches nothing, in membership or in gossip.

`tests/visualize.rs` has two non-ignored tests guarding this. If that branch ever
changes shape, they fail rather than letting every trace quietly become fiction.

### Why the passive view is blank

`add_to_passive` (`membership.rs:53`) emits no actions at any of its six call
sites, and there is no accessor. It genuinely cannot be observed from a test.

Since the passive view is where most of the unimplemented HyParView behaviour
lives (no shuffle initiator, no Neighbor-based recovery), it is probably worth
seeing eventually — that needs one line in `src/`:

```rust
#[cfg(test)] pub fn passive_peers(&self) -> impl Iterator<Item = &Id> { self.passive.iter() }
```

plus a `#[cfg(test)]` re-export. Left undone deliberately; this tooling is
additive only.

## Two things the traces expose

**Runs are not reproducible.** The hub's active view in `viz_eviction` differs
between runs. Every "random" choice in `membership.rs` is
`HashSet::iter().next()` or `.find()` (`:62`, `:79`, `:130`, `:179`), and Rust's
default hasher is randomly seeded per process — so the overlay is
nondeterministic without being uniformly random, which is the worst of both.
HyParView's guarantees assume uniform random selection. The `TODO` at
`membership.rs:61` already flags this.

**Redundancy is high.** `viz_tree_formation` reports ~7.3× sends per delivery.
The tree does converge to 29 edges, but the flood to get there is expensive on a
4-chord ring. Worth watching if fanout or wiring changes.

## Reusing the harness

`tests/common/mod.rs` is a general-purpose simulator, not just a trace recorder.
It has `delivery_counts()`, `redundancy()`, and `max_hop()`, plus `blackhole()`
for dropping payloads in flight — useful for ordinary tests too. `run_fifo()`
keeps exact parity with `tests/simulation.rs`; `run_rounds()` batches by round,
which is easier to follow when animated.
