//! Ternary routing: route requests with {-1=reject, 0=queue, +1=accept} decisions.

use std::collections::{HashMap, VecDeque};

/// Route destination
#[derive(Clone, Debug)]
pub struct Destination {
    pub id: usize,
    pub health: i8,   // -1=down, 0=degraded, +1=healthy
    pub load: f64,     // 0-1 current load
    pub weight: f64,   // routing weight
}

/// Routing decision
#[derive(Clone, Debug, PartialEq)]
pub enum RouteDecision {
    Accept(usize),   // route to destination
    Queue,           // queue for later
    Reject,          // reject request
}

/// Router with ternary health awareness
pub struct TernaryRouter {
    pub destinations: Vec<Destination>,
    pub queue: VecDeque<usize>, // queued request indices
    pub max_queue: usize,
}

impl TernaryRouter {
    pub fn new(destinations: Vec<Destination>, max_queue: usize) -> Self {
        Self { destinations, queue: VecDeque::new(), max_queue }
    }

    /// Route based on health and load
    pub fn route(&mut self, request_id: usize) -> RouteDecision {
        // Find best healthy destination
        let best = self.destinations.iter()
            .filter(|d| d.health > 0 && d.load < 0.9)
            .min_by(|a, b| a.load.partial_cmp(&b.load).unwrap_or(std::cmp::Ordering::Equal));

        match best {
            Some(dest) => RouteDecision::Accept(dest.id),
            None => {
                // Try degraded
                let degraded = self.destinations.iter()
                    .filter(|d| d.health == 0 && d.load < 0.8)
                    .min_by(|a, b| a.load.partial_cmp(&b.load).unwrap_or(std::cmp::Ordering::Equal));
                match degraded {
                    Some(dest) => RouteDecision::Accept(dest.id),
                    None => {
                        if self.queue.len() < self.max_queue {
                            self.queue.push_back(request_id);
                            RouteDecision::Queue
                        } else {
                            RouteDecision::Reject
                        }
                    }
                }
            }
        }
    }

    /// Weighted round-robin among healthy destinations
    pub fn weighted_route(&self) -> Option<usize> {
        let healthy: Vec<&Destination> = self.destinations.iter().filter(|d| d.health > 0).collect();
        if healthy.is_empty() { return None; }
        let total_weight: f64 = healthy.iter().map(|d| d.weight).sum();
        let mut target = (self.queue.len() as f64 % total_weight);
        for dest in &healthy {
            target -= dest.weight;
            if target <= 0.0 { return Some(dest.id); }
        }
        healthy.last().map(|d| d.id)
    }

    /// Update destination health based on response
    pub fn update_health(&mut self, dest_id: usize, success: bool) {
        if let Some(dest) = self.destinations.iter_mut().find(|d| d.id == dest_id) {
            if success {
                dest.health = 1;
                dest.load = (dest.load - 0.1).max(0.0);
            } else {
                dest.health = (dest.health - 1).max(-1);
                dest.load = (dest.load + 0.2).min(1.0);
            }
        }
    }

    /// Drain queue and attempt routing
    pub fn drain_queue(&mut self) -> Vec<RouteDecision> {
        let mut results = Vec::new();
        while let Some(req_id) = self.queue.pop_front() {
            results.push(self.route(req_id));
        }
        results
    }

    /// Load balance: redistribute load across destinations
    pub fn rebalance(&mut self) {
        let avg_load: f64 = self.destinations.iter().map(|d| d.load).sum::<f64>() / self.destinations.len().max(1) as f64;
        for dest in &mut self.destinations {
            if dest.health > 0 {
                dest.load = avg_load; // simulate redistribution
            }
        }
    }

    /// Failover: route around failed destinations
    pub fn failover(&mut self, failed_id: usize) -> Vec<usize> {
        let mut redirected = Vec::new();
        if let Some(dest) = self.destinations.iter_mut().find(|d| d.id == failed_id) {
            dest.health = -1;
            dest.load = 1.0;
        }
        for dest in &self.destinations {
            if dest.health > 0 { redirected.push(dest.id); }
        }
        redirected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_to_healthy() {
        let dests = vec![
            Destination { id: 0, health: 1, load: 0.3, weight: 1.0 },
            Destination { id: 1, health: 1, load: 0.7, weight: 1.0 },
        ];
        let mut router = TernaryRouter::new(dests, 10);
        let decision = router.route(1);
        assert_eq!(decision, RouteDecision::Accept(0)); // least loaded
    }

    #[test]
    fn test_route_to_degraded() {
        let dests = vec![
            Destination { id: 0, health: -1, load: 1.0, weight: 1.0 },
            Destination { id: 1, health: 0, load: 0.5, weight: 1.0 },
        ];
        let mut router = TernaryRouter::new(dests, 10);
        let decision = router.route(1);
        assert_eq!(decision, RouteDecision::Accept(1));
    }

    #[test]
    fn test_queue_when_all_down() {
        let dests = vec![
            Destination { id: 0, health: -1, load: 1.0, weight: 1.0 },
        ];
        let mut router = TernaryRouter::new(dests, 5);
        let decision = router.route(1);
        assert_eq!(decision, RouteDecision::Queue);
    }

    #[test]
    fn test_reject_when_queue_full() {
        let dests = vec![
            Destination { id: 0, health: -1, load: 1.0, weight: 1.0 },
        ];
        let mut router = TernaryRouter::new(dests, 2);
        router.route(1);
        router.route(2);
        let decision = router.route(3);
        assert_eq!(decision, RouteDecision::Reject);
    }

    #[test]
    fn test_weighted_route() {
        let dests = vec![
            Destination { id: 0, health: 1, load: 0.5, weight: 2.0 },
            Destination { id: 1, health: 1, load: 0.5, weight: 1.0 },
        ];
        let router = TernaryRouter::new(dests, 10);
        let dest = router.weighted_route();
        assert!(dest.is_some());
    }

    #[test]
    fn test_update_health() {
        let dests = vec![Destination { id: 0, health: 1, load: 0.5, weight: 1.0 }];
        let mut router = TernaryRouter::new(dests, 10);
        router.update_health(0, false);
        assert_eq!(router.destinations[0].health, 0);
    }

    #[test]
    fn test_failover() {
        let dests = vec![
            Destination { id: 0, health: 1, load: 0.3, weight: 1.0 },
            Destination { id: 1, health: 1, load: 0.5, weight: 1.0 },
        ];
        let mut router = TernaryRouter::new(dests, 10);
        let available = router.failover(0);
        assert_eq!(available, vec![1]);
        assert_eq!(router.destinations[0].health, -1);
    }

    #[test]
    fn test_drain_queue() {
        let dests = vec![Destination { id: 0, health: -1, load: 1.0, weight: 1.0 }];
        let mut router = TernaryRouter::new(dests, 5);
        router.route(1);
        router.route(2);
        // Make destination healthy
        router.destinations[0].health = 1;
        router.destinations[0].load = 0.0;
        let results = router.drain_queue();
        assert_eq!(results.len(), 2);
    }
}
