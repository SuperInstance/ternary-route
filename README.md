# ternary-route

Ternary routing engine with **three-tier health classification** for load-balanced request distribution. Every destination is classified as `+1` (healthy), `0` (degraded), or `-1` (down), and routing decisions cascade through tiers: healthy first, degraded as fallback, queue when all else fails.

## Why It Matters

Traditional routers use binary health (up/down), which causes thundering-herd problems when a "down" node recovers. Ternary routing introduces a **degraded** middle state that absorbs partial traffic during recovery, smoothing the transition:

| Health | Value | Traffic Share |
|--------|-------|---------------|
| Healthy | `+1` | Full (least-loaded selection) |
| Degraded | `0` | Fallback (when all healthy are saturated) |
| Down | `-1` | None |

The router also implements **weighted round-robin**, **queue overflow protection**, and **failover** to route around failures in real time.

## How It Works

### Cascading Route Selection

The `route(request_id)` method follows a strict priority cascade:

```
1. Select min-load destination where health = +1 and load < 0.9
2. If none, select min-load destination where health = 0 and load < 0.8
3. If none, queue the request (if queue not full)
4. If queue full, reject
```

This is equivalent to a three-tier weighted selection where tier weights are `(+1) ≫ (0) ≫ (queue) ≫ (reject)`.

**Complexity:** O(n) per route decision, where n = destination count.

### Weighted Round-Robin

For healthy destinations, weighted selection distributes traffic proportionally:

```
total_weight = Σ wᵢ for healthy destinations
target = (queue_len mod total_weight)
for each dest: target -= wᵢ; if target ≤ 0: select dest
```

This is a streaming variant of **interleaved weighted round-robin** — O(n) per selection.

### Health Updates

Health transitions follow ternary escalation:

```
success → health = +1, load -= 0.1 (min 0.0)
failure → health -= 1 (min -1), load += 0.2 (max 1.0)
```

Each failure degrades the health by one step. Two consecutive failures move a node from `+1` to `-1`.

### Queue and Drain

Requests that cannot be routed immediately are queued (bounded by `max_queue`). When capacity returns, `drain_queue()` re-attempts routing for all queued requests.

### Rebalancing

The `rebalance()` method redistributes load across all healthy destinations to the average load, simulating the effect of a load-aware balancer.

## Quick Start

```rust
use ternary_route::{TernaryRouter, Destination, RouteDecision};

let dests = vec![
    Destination { id: 0, health: 1, load: 0.3, weight: 1.0 },
    Destination { id: 1, health: 1, load: 0.7, weight: 1.0 },
    Destination { id: 2, health: -1, load: 1.0, weight: 1.0 }, // down
];

let mut router = TernaryRouter::new(dests, max_queue: 10);

// Routes to least-loaded healthy destination
assert_eq!(router.route(1), RouteDecision::Accept(0));

// Failover: mark node 0 as down, traffic shifts
let available = router.failover(0);
assert_eq!(available, vec![1]);
```

## API

### `TernaryRouter`

| Method | Returns | Description |
|--------|---------|-------------|
| `new(destinations, max_queue)` | `Self` | Initialize router |
| `route(request_id)` | `RouteDecision` | Cascade-select destination |
| `weighted_route()` | `Option<usize>` | Weighted selection among healthy |
| `update_health(dest_id, success)` | `()` | Adjust health + load |
| `failover(failed_id)` | `Vec<usize>` | Mark down, return available |
| `drain_queue()` | `Vec<RouteDecision>` | Re-route queued requests |
| `rebalance()` | `()` | Equalize load across healthy |

### `RouteDecision`

```rust
pub enum RouteDecision {
    Accept(usize),  // route to destination
    Queue,          // queued for later
    Reject,         // queue full, request rejected
}
```

### `Destination`

```rust
pub struct Destination {
    pub id: usize,
    pub health: i8,   // -1, 0, +1
    pub load: f64,    // [0.0, 1.0]
    pub weight: f64,  // routing weight
}
```

## Architecture Notes

The **γ + η = C** invariant manifests as follows: the *generation* (γ) is the set of routing decisions producing traffic flow, the *entropy* (η) is the health/load diversity across destinations, and *conservation* (C) is the invariant that no destination exceeds capacity while queue bounds are maintained. The cascading selection is the conservation law — it guarantees that traffic distribution remains within safe operating bounds regardless of individual node health states.

## References

- **Load balancing algorithms:** Patterson, D. & Hennessy, J. *Computer Architecture* (2017), Appendix D
- **Circuit breakers and bulkheads:** Nygard, M. *Release It!* (2007)
- **Weighted round-robin:** Katevenis, M. *Fast Switching and Fair Control* (1987)
- **Health-checked routing:** RFC 7234, §4.3 (cache health semantics)

## License

MIT
