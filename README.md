# ternary-route

Routing where every destination carries a trivalent health signal — down, degraded, or healthy — and the decision is reject, queue, or accept.

## Why This Exists

Load balancers typically treat backends as binary: alive or dead. Reality is messier. A server can be "technically up but slow" (degraded). It can handle traffic but shouldn't get more. It can be returning errors but might recover. Binary health checks force you to either route traffic to a struggling server or remove it entirely — both of which are wrong.

Ternary routing gives destinations three health states: `{−1 = down, 0 = degraded, +1 = healthy}` and produces three routing decisions: `Reject`, `Queue`, `Accept`. The routing priority is explicit: route to healthy first, fall back to degraded, queue if nothing's available, reject if the queue is full. This degrades gracefully instead of catastrophically.

Weighted round-robin, health-based failover, and load rebalancing all respect the ternary model. Degraded nodes get traffic only when healthy nodes are overloaded. Failed nodes are routed around immediately.

## Architecture

```
Destination {id, health: i8, load: f64, weight: f64}
    │
    ▼
TernaryRouter
    ├── destinations: Vec<Destination>
    ├── queue: VecDeque<usize>      (queued request ids)
    │
    ├── route(request_id) ──► RouteDecision
    │   ├── Find healthy + load < 0.9 → Accept(least loaded)
    │   ├── Find degraded + load < 0.8 → Accept(least loaded)
    │   └── Queue if space, Reject if full
    │
    ├── weighted_route() ──► Option<usize>   (weighted round-robin)
    ├── update_health(id, success) ──► adjust health ±1, load ±0.1/0.2
    ├── failover(failed_id) ──► mark down, return available backends
    ├── rebalance() ──► equalize load across healthy destinations
    └── drain_queue() ──► retry queued requests
```

**Key types:**

- **`Destination`** — a backend with id, health `{-1, 0, +1}`, current load `[0, 1]`, and routing weight.
- **`RouteDecision`** — `Accept(usize)` (route to destination), `Queue` (hold for later), `Reject` (drop request).
- **`TernaryRouter`** — the routing engine. Holds destinations and a bounded request queue.

## Usage

```rust
use ternary_route::{TernaryRouter, Destination, RouteDecision};

let dests = vec![
    Destination { id: 0, health: 1,  load: 0.3, weight: 2.0 },  // healthy, light load
    Destination { id: 1, health: 1,  load: 0.7, weight: 1.0 },  // healthy, heavy load
    Destination { id: 2, health: 0,  load: 0.5, weight: 1.0 },  // degraded
    Destination { id: 3, health: -1, load: 1.0, weight: 1.0 },  // down
];
let mut router = TernaryRouter::new(dests, 100); // max 100 queued requests

// Route to least-loaded healthy destination
let decision = router.route(1);
assert_eq!(decision, RouteDecision::Accept(0)); // dest 0 has load 0.3

// Update health based on response
router.update_health(0, false); // request failed
assert_eq!(router.destinations[0].health, 0); // healthy → degraded

// Failover: mark a destination as down, get list of alternatives
let available = router.failover(3); // dest 3 already down, but marks it formally
// available = [0, 1, 2] (all non-down)

// Weighted round-robin among healthy destinations
let dest = router.weighted_route(); // respects weights 2:1

// When all destinations are down or overloaded, requests queue
router.destinations[0].health = -1;
router.destinations[1].health = -1;
router.destinations[2].health = -1;
let decision = router.route(42);
assert_eq!(decision, RouteDecision::Queue); // queued, not rejected

// Drain queue when capacity returns
router.destinations[0].health = 1;
router.destinations[0].load = 0.0;
let results = router.drain_queue(); // retry all queued requests
```

## API Reference

### `Destination`

Fields: `id: usize`, `health: i8` `{-1, 0, +1}`, `load: f64` `[0, 1]`, `weight: f64`

### `RouteDecision`

| Variant | Description |
|---------|-------------|
| `Accept(usize)` | Route to destination with given id |
| `Queue` | Hold request in queue (all backends unavailable) |
| `Reject` | Drop request (queue full) |

### `TernaryRouter`

| Method | Description |
|--------|-------------|
| `TernaryRouter::new(destinations, max_queue)` | Create router with bounded queue |
| `.route(request_id)` | Route to least-loaded healthy → degraded → queue → reject |
| `.weighted_route()` | Weighted round-robin among healthy destinations |
| `.update_health(dest_id, success)` | Adjust health: success → +1 and reduce load; failure → health −1 and increase load |
| `.failover(failed_id)` | Mark destination as down (`−1`), return ids of healthy alternatives |
| `.rebalance()` | Equalize load across healthy destinations to average |
| `.drain_queue()` | Retry all queued requests, return decisions |

## The Deeper Idea

Ternary routing implements **circuit breaker** semantics without explicit state machines. The health value `{-1, 0, +1}` *is* the circuit breaker state: closed (+1), half-open (0), open (-1). The `update_health()` method transitions between these states based on success/failure signals, exactly like Netflix's Hystrix or resilience4j.

The load-aware routing priority (healthy before degraded) is a form of **traffic shaping** that prevents cascading failures. When a server starts degrading, it receives less traffic automatically — not because it was removed from the pool, but because the router prefers healthier alternatives. This is gentler than binary circuit breaking, which abruptly cuts all traffic and can cause thundering herds when the server recovers.

The bounded queue is a deliberate backpressure mechanism. When all backends are unhealthy, requests accumulate up to `max_queue`, then get rejected. This is better than unbounded queuing (OOM) or immediate rejection (no recovery window). The queue gives the system a grace period — if a backend recovers within that window, queued requests get served.

## Related Crates

- **`ternary-scheduler`** — task scheduling with the same priority model, complementary to routing
- **`ternary-proof`** — verification with ternary verdicts, useful for health check verification
- **`ternary-negotiate`** — multi-agent negotiation, structurally similar to weighted routing
