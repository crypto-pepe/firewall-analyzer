use crate::model::Request;
use crate::validator::generic_validator::{BanRule, BaseCostCounter, RequestCoster};
use chrono::{DateTime, Utc};
use circular_queue::CircularQueue;

pub struct CostCount {}

impl RequestCoster for CostCount {
    fn cost(&self, _r: &Request) -> u64 {
        1
    }
}

#[derive(Debug)]
pub struct CountStateHolder {
    request_time: CircularQueue<DateTime<Utc>>,
}

impl BaseCostCounter for CountStateHolder {
    fn new(r: &BanRule) -> Self {
        CountStateHolder {
            request_time: CircularQueue::with_capacity(r.limit as usize),
        }
    }

    fn add(&mut self, _cost: u64, time: DateTime<Utc>) {
        self.request_time.push(time);
    }

    fn latest_value_added_at(&self) -> Option<DateTime<Utc>> {
        self.request_time.iter().last().copied()
    }

    fn is_above_limit(&self, time: &DateTime<Utc>) -> bool {
        if !self.request_time.is_full() {
            return false;
        }
        self.request_time
            .iter()
            .last()
            .expect("requests queue is empty")
            > time
    }

    fn clear(&mut self) {
        self.request_time.clear();
    }
}
