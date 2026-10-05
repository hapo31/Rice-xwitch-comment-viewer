//! Bounded per-connection duplicate cache; reconnects retain the same instance.
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

pub(super) struct MessageDedupe {
    limit: usize,
    ttl: Duration,
    seen: HashSet<String>,
    order: VecDeque<(String, Instant)>,
}

impl MessageDedupe {
    pub(super) fn new(limit: usize, ttl: Duration) -> Self {
        Self {
            limit,
            ttl,
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn insert(&mut self, id: String) -> bool {
        self.insert_at(id, Instant::now())
    }

    pub(super) fn insert_at(&mut self, id: String, now: Instant) -> bool {
        self.remove_expired(now);

        if !self.seen.insert(id.clone()) {
            return false;
        }

        self.order.push_back((id, now));
        while self.order.len() > self.limit {
            if let Some((old_id, _)) = self.order.pop_front() {
                self.seen.remove(&old_id);
            }
        }
        true
    }

    fn remove_expired(&mut self, now: Instant) {
        while self
            .order
            .front()
            .is_some_and(|(_, seen_at)| now.saturating_duration_since(*seen_at) >= self.ttl)
        {
            if let Some((expired_id, _)) = self.order.pop_front() {
                self.seen.remove(&expired_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_receive_clock_observes_exact_ttl_and_capacity_boundaries() {
        let start = Instant::now();
        let mut cache = MessageDedupe::new(2, Duration::from_secs(60));
        assert!(cache.insert_at("a".into(), start));
        assert!(!cache.insert_at("a".into(), start + Duration::from_secs(59)));
        assert!(cache.insert_at("a".into(), start + Duration::from_secs(60)));
        assert_eq!(cache.seen.len(), 1);
        assert_eq!(cache.order.len(), 1);
        assert!(cache.insert_at("b".into(), start + Duration::from_secs(61)));
        assert!(cache.insert_at("c".into(), start + Duration::from_secs(62)));
        assert_eq!(cache.seen.len(), 2);
        assert_eq!(cache.order.len(), 2);
        assert!(!cache.seen.contains("a"));
    }
}
