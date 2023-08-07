use crate::multiplex::pipe_pool::*;
use std::{
    collections::{btree_map::Entry, BTreeMap},
    time::{Duration, Instant},
};

use super::calc::{BwCalculator, RttCalculator};

#[derive(Debug, Clone)]
/// An element of Inflight.
pub struct InflightEntry {
    seqno: Seqno,
    send_time: Instant,
    retrans: u64,
    payload: Message,

    retrans_time: Instant,
    delivered: u64,
    delivered_time: Instant,

    known_lost: bool,
}

/// A data structure that tracks in-flight packets.
pub struct Inflight {
    segments: BTreeMap<Seqno, InflightEntry>,
    rtos: BTreeMap<Instant, Vec<Seqno>>,

    rtt: RttCalculator,
    bw: BwCalculator,

    sent: u64,
    retrans: u64,
    // max_inversion: Duration,
    // max_acked_sendtime: Instant,
}

impl Inflight {
    /// Creates a new Inflight.
    pub fn new() -> Self {
        Inflight {
            segments: Default::default(),
            rtos: Default::default(),
            rtt: Default::default(),
            bw: Default::default(),

            sent: 0,
            retrans: 0,
            // max_inversion: Duration::from_millis(1),
            // max_acked_sendtime: Instant::now(),
        }
    }

    pub fn unacked(&self) -> usize {
        self.segments.len()
    }

    pub fn inflight(&self) -> usize {
        // all segments that are still in flight
        self.segments.len()
    }

    pub fn last_minus_first(&self) -> usize {
        (self
            .segments
            .iter()
            .next_back()
            .map(|f| *f.0)
            .unwrap_or_default()
            - self
                .segments
                .iter()
                .next()
                .map(|f| *f.0)
                .unwrap_or_default()) as usize
    }

    /// Mark all inflight packets less than a certain sequence number as acknowledged.
    pub fn mark_acked_lt(&mut self, seqno: Seqno) -> usize {
        let mut to_remove = vec![];
        for (k, _) in self.segments.iter() {
            if *k < seqno {
                to_remove.push(*k);
            } else {
                // we can rely on iteration order
                break;
            }
        }
        let mut sum = 0;
        for seqno in to_remove {
            sum += usize::from(self.mark_acked(seqno));
        }
        sum
    }

    /// Marks a particular inflight packet as acknowledged. Returns whether or not there was actually such an inflight packet.
    pub fn mark_acked(&mut self, acked_seqno: Seqno) -> bool {
        let now = Instant::now();

        if let Some(acked_seg) = self.segments.remove(&acked_seqno) {
            // record RTT
            if acked_seg.retrans == 0 {
                self.rtt
                    .record_sample(now.saturating_duration_since(acked_seg.send_time));
            }
            // record bandwidth
            self.bw.on_ack(acked_seg.delivered, acked_seg.send_time);
            // remove from rtos
            self.remove_rto(acked_seg.retrans_time, acked_seqno);

            true
        } else {
            false
        }
    }

    /// Inserts a packet to the inflight.
    pub fn insert(&mut self, msg: Message) {
        let seqno = msg.seqno();
        let now = Instant::now();
        let rto_duration = self.rtt.rto();
        let rto = now + rto_duration;
        let prev = self.segments.insert(
            seqno,
            InflightEntry {
                seqno,
                send_time: now,
                payload: msg,
                retrans: 0,
                retrans_time: rto,
                known_lost: false,
                delivered: self.bw.delivered(),
                delivered_time: self.bw.delivered_time(),
            },
        );
        assert!(prev.is_none());
        // we insert into RTOs.
        self.rtos.entry(rto).or_default().push(seqno);
        self.sent += 1;
    }

    /// Returns the retransmission time of the first possibly retransmitted packet, as well as its seqno. This skips all known-lost packets.
    pub fn first_rto(&self) -> Option<(Seqno, Instant)> {
        self.rtos
            .iter()
            .next()
            .map(|(instant, seqno)| (seqno[0], *instant))
    }

    /// Retransmits a particular seqno, clearing the "known lost" flag on the way.
    pub fn retransmit(&mut self, seqno: Seqno) -> Option<Message> {
        let rto = self.rtt.rto();
        let (payload, old_retrans, new_retrans) = {
            let entry = self.segments.get_mut(&seqno);
            entry.map(|entry| {
                let old_retrans = entry.retrans_time;
                entry.retrans += 1;

                entry.retrans_time =
                    Instant::now() + rto.mul_f64(2.0f64.powi(entry.retrans as i32).min(60.0));
                entry.known_lost = false;
                (entry.payload.clone(), old_retrans, entry.retrans_time)
            })?
        };
        // eprintln!("retransmit {}", seqno);
        self.remove_rto(old_retrans, seqno);
        self.rtos.entry(new_retrans).or_default().push(seqno);
        self.sent += 1;
        self.retrans += 1;
        log::debug!(
            "retransmission {:.2}% ({}/{})",
            (self.retrans as f64) / (self.sent as f64) * 100.0,
            self.retrans,
            self.sent
        );
        Some(payload)
    }

    /// Marks a particular packet as known to be lost. Does not immediately retransmit it yet!
    pub fn mark_lost(&mut self, seqno: Seqno) -> bool {
        if let Some(seg) = self.segments.get_mut(&seqno) {
            seg.known_lost = true;
            let retrans_time = seg.retrans_time;
            self.remove_rto(retrans_time, seqno);

            true
        } else {
            false
        }
    }

    fn remove_rto(&mut self, retrans_time: Instant, seqno: Seqno) {
        let rto_entry = self.rtos.entry(retrans_time);
        if let Entry::Occupied(mut o) = rto_entry {
            o.get_mut().retain(|v| *v != seqno);
            if o.get().is_empty() {
                o.remove();
            }
        }
    }

    pub fn min_rtt(&self) -> Duration {
        self.rtt.min_rtt()
    }

    /// The total bdp of the link, in packets
    pub fn bdp(&self) -> usize {
        (self.bw.delivery_rate() * self.rtt.min_rtt().as_secs_f64()) as usize
    }

    /// The bandwidth of the link, in RTT/sec
    pub fn delivery_rate(&self) -> f64 {
        self.bw.delivery_rate()
    }
}
