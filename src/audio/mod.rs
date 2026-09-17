// Audio plumbing: a lock-free SPSC ring carries samples from the emulation
// thread to the output stream callback, with pluggable backends.

mod backend;

pub use backend::{start, AudioOutput};

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Ring capacity in samples (power of two): ~90ms at 44.1kHz — enough to
/// ride out scheduler hiccups while keeping latency low. The usable capacity
/// is one slot less (the classic head==tail rule).
const RING_CAPACITY: usize = 4096;

struct Ring {
    /// Shared sample storage. Sound because each slot is written by the
    /// producer only while it owns the head slot, and read by the consumer
    /// only after the Acquire of head publishes that write; the producer
    /// reclaims a slot only after the Acquire of tail publishes the read.
    buf: UnsafeCell<Box<[f32]>>,
    mask: usize,
    /// Written by the producer only.
    head: AtomicUsize,
    /// Written by the consumer only.
    tail: AtomicUsize,
}

// Sound: UnsafeCell is safe here because the SPSC index protocol keeps all
// slot access disjoint (see the comment on `buf`).
unsafe impl Sync for Ring {}

pub struct AudioProducer(Arc<Ring>);

pub struct AudioConsumer(Arc<Ring>);

/// Creates the producer/consumer pair for one live audio stream.
pub fn audio_ring() -> (AudioProducer, AudioConsumer) {
    let ring = Ring {
        buf: UnsafeCell::new(vec![0.0; RING_CAPACITY].into_boxed_slice()),
        mask: RING_CAPACITY - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    };
    let arc = Arc::new(ring);
    (AudioProducer(Arc::clone(&arc)), AudioConsumer(arc))
}

impl AudioProducer {
    /// Approximate number of samples currently buffered (for diagnostics).
    pub fn len(&self) -> usize {
        let ring = &self.0;
        ring.head
            .load(Ordering::Relaxed)
            .wrapping_sub(ring.tail.load(Ordering::Acquire))
            & ring.mask
    }

    /// Total ring slots; one is unusable (the head==tail rule).
    pub fn capacity(&self) -> usize {
        RING_CAPACITY
    }

    /// Pushes samples into the ring. If the consumer falls behind the ring
    /// fills and the remaining samples of the chunk are dropped: the
    /// emulation thread must never block, and dropping the newest audio
    /// keeps the SPSC invariants intact.
    pub fn push(&self, samples: &[f32]) {
        let ring = &self.0;
        for &sample in samples {
            let head = ring.head.load(Ordering::Relaxed);
            let next = (head + 1) & ring.mask;
            if next == ring.tail.load(Ordering::Acquire) {
                break;
            }
            unsafe {
                (*ring.buf.get())[head] = sample;
            }
            ring.head.store(next, Ordering::Release);
        }
    }
}

impl AudioConsumer {
    /// Pops up to `out.len()` samples; returns how many were available. The
    /// callback tops up any shortfall with the last value to avoid clicks.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let ring = &self.0;
        let mut count = 0;
        while count < out.len() {
            let tail = ring.tail.load(Ordering::Relaxed);
            if tail == ring.head.load(Ordering::Acquire) {
                break;
            }
            out[count] = unsafe { (*ring.buf.get())[tail] };
            ring.tail.store((tail + 1) & ring.mask, Ordering::Release);
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::{audio_ring, RING_CAPACITY};

    #[test]
    fn test_push_pop_round_trip() {
        let (producer, consumer) = audio_ring();
        let samples: Vec<f32> = (0..1000u32).map(|i| i as f32 * 0.5).collect();
        producer.push(&samples);
        let mut out = vec![0.0f32; 1000];
        assert_eq!(consumer.pop(&mut out), 1000);
        assert_eq!(out, samples);
    }

    #[test]
    fn test_pop_partial() {
        let (producer, consumer) = audio_ring();
        producer.push(&[1.0, 2.0, 3.0]);
        let mut out = [0.0f32; 10];
        assert_eq!(consumer.pop(&mut out), 3);
        assert_eq!(&out[..3], &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_pop_empty() {
        let (_producer, consumer) = audio_ring();
        let mut out = [0.0f32; 10];
        assert_eq!(consumer.pop(&mut out), 0);
    }

    #[test]
    fn test_overflow_keeps_oldest() {
        let (producer, consumer) = audio_ring();
        let total = RING_CAPACITY + 10;
        let samples: Vec<f32> = (0..total as u32).map(|i| i as f32).collect();
        producer.push(&samples);
        let mut out = vec![0.0f32; total];
        let got = consumer.pop(&mut out);
        assert_eq!(got, RING_CAPACITY - 1);
        // The oldest samples survive; the tail of the chunk was dropped.
        assert_eq!(out[..got], samples[..got]);
    }

    #[test]
    fn test_ring_spsc_ordering() {
        use std::thread;
        let (producer, consumer) = audio_ring();
        // 4000 samples fit the ring, so no drops are possible regardless of
        // scheduling; the consumer must see the exact sequence.
        let handle = thread::spawn(move || {
            let mut buf = [0.0f32; 512];
            let mut expected = 0u32;
            while expected < 4000 {
                let got = consumer.pop(&mut buf);
                for &value in &buf[..got] {
                    assert_eq!(value as u32, expected);
                    expected += 1;
                }
            }
        });
        for chunk in (0..4000u32).collect::<Vec<_>>().chunks(256) {
            let samples: Vec<f32> = chunk.iter().map(|&i| i as f32).collect();
            producer.push(&samples);
        }
        handle.join().unwrap();
    }
}
