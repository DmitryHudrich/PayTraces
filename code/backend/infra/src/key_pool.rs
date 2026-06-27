use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Pool of upstream API keys with per-key cooldown. Round-robin selection,
/// with cooled keys (recently 429'd) skipped until their cooldown elapses.
///
/// `pick()` returns `None` only when *every* key is currently cooled — the
/// caller treats that as `DomainError::RateLimited` so a router upstream
/// can fail over to a different source.
#[derive(Clone)]
pub struct KeyPool {
    inner: Arc<KeyPoolInner>,
}

struct KeyPoolInner {
    keys: Vec<KeyHandle>,
    next_idx: AtomicUsize,
    cooldown: Duration,
}

struct KeyHandle {
    key: String,
    cooldown_until_ms: AtomicU64,
}

impl KeyPool {
    /// `keys` must be non-empty; the source layer is responsible for that
    /// invariant at construction time (config validation surfaces missing
    /// keys with a clearer message).
    pub fn new(keys: Vec<String>, cooldown: Duration) -> Self {
        assert!(!keys.is_empty(), "KeyPool requires at least one key");
        let keys = keys
            .into_iter()
            .map(|k| KeyHandle {
                key: k,
                cooldown_until_ms: AtomicU64::new(0),
            })
            .collect();
        Self {
            inner: Arc::new(KeyPoolInner {
                keys,
                next_idx: AtomicUsize::new(0),
                cooldown,
            }),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.keys.len()
    }

    /// Round-robin pick of a non-cooled key. Returns `None` when every key
    /// is currently cooling — caller bubbles up `RateLimited` so the router
    /// can fail over to the next source.
    pub fn pick(&self) -> Option<String> {
        let now_ms = now_unix_ms();
        let n = self.inner.keys.len();
        // Advance the round-robin cursor once per call; subsequent attempts
        // within this call sweep deterministically from there. This keeps
        // load spread even under contention without holding any lock.
        let start = self.inner.next_idx.fetch_add(1, Ordering::Relaxed) % n;
        for i in 0..n {
            let idx = (start + i) % n;
            let handle = &self.inner.keys[idx];
            let cooldown_until = handle.cooldown_until_ms.load(Ordering::Relaxed);
            if cooldown_until <= now_ms {
                return Some(handle.key.clone());
            }
        }
        None
    }

    /// Like `pick`, but when every key is cooling returns `Err(wait)` —
    /// the duration until the soonest key becomes available again. The
    /// caller can sleep for `wait` and retry (giving real retries with a
    /// single key) instead of immediately failing over to a sibling source.
    pub fn pick_or_wait(&self) -> Result<String, Duration> {
        if let Some(k) = self.pick() {
            return Ok(k);
        }
        let now_ms = now_unix_ms();
        let soonest = self
            .inner
            .keys
            .iter()
            .map(|h| h.cooldown_until_ms.load(Ordering::Relaxed))
            .min()
            .unwrap_or(now_ms);
        // .max(1) — never hand back a zero Duration; the caller would
        // spin tight if we did and the race resolved against them.
        Err(Duration::from_millis(
            soonest.saturating_sub(now_ms).max(1),
        ))
    }

    /// Mark `key` as cooling for the pool's configured cooldown. No-op if
    /// the key isn't part of this pool (defensive — shouldn't happen).
    pub fn cool(&self, key: &str) {
        let until_ms = now_unix_ms() + self.inner.cooldown.as_millis() as u64;
        for handle in &self.inner.keys {
            if handle.key == key {
                // Keep the longer of the two cooldowns — concurrent rate-limit
                // hits on the same key shouldn't shorten the penalty window.
                handle
                    .cooldown_until_ms
                    .fetch_max(until_ms, Ordering::Relaxed);
                return;
            }
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_key_always_returned() {
        let pool = KeyPool::new(vec!["A".into()], Duration::from_secs(1));
        assert_eq!(pool.pick().as_deref(), Some("A"));
        assert_eq!(pool.pick().as_deref(), Some("A"));
    }

    #[test]
    fn round_robin_spreads_load() {
        let pool = KeyPool::new(
            vec!["A".into(), "B".into(), "C".into()],
            Duration::from_secs(1),
        );
        let picks: Vec<_> = (0..3).map(|_| pool.pick().unwrap()).collect();
        let mut sorted = picks.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["A", "B", "C"]);
    }

    #[test]
    fn cooled_key_is_skipped_and_others_keep_serving() {
        let pool = KeyPool::new(
            vec!["A".into(), "B".into(), "C".into()],
            Duration::from_secs(60),
        );
        pool.cool("B");
        for _ in 0..20 {
            let k = pool.pick().unwrap();
            assert_ne!(k, "B", "cooled key must not be picked");
        }
    }

    #[test]
    fn all_cooled_returns_none() {
        let pool = KeyPool::new(
            vec!["A".into(), "B".into()],
            Duration::from_secs(60),
        );
        pool.cool("A");
        pool.cool("B");
        assert!(pool.pick().is_none());
    }

    #[test]
    fn cooldown_expires_and_key_becomes_available() {
        let pool = KeyPool::new(vec!["A".into()], Duration::from_millis(20));
        pool.cool("A");
        assert!(pool.pick().is_none());
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(pool.pick().as_deref(), Some("A"));
    }

    #[test]
    #[should_panic(expected = "at least one key")]
    fn empty_pool_panics() {
        let _ = KeyPool::new(vec![], Duration::from_secs(1));
    }

    #[test]
    fn pick_or_wait_returns_key_when_available() {
        let pool = KeyPool::new(vec!["A".into()], Duration::from_secs(1));
        assert_eq!(pool.pick_or_wait().unwrap(), "A");
    }

    #[test]
    fn pick_or_wait_returns_wait_when_all_cooled() {
        let pool = KeyPool::new(
            vec!["A".into(), "B".into()],
            Duration::from_secs(5),
        );
        pool.cool("A");
        pool.cool("B");
        let wait = pool.pick_or_wait().unwrap_err();
        // Cooldown was 5s — wait should be close to that, definitely > 0.
        assert!(
            wait <= Duration::from_secs(5) && wait >= Duration::from_millis(1),
            "wait outside expected range: {wait:?}"
        );
    }

    #[test]
    fn pick_or_wait_returns_soonest_not_latest() {
        // Cool A first (longer cooldown), then sleep, then cool B (shorter
        // remaining). The soonest expiry comes from A (it cooled first),
        // not B — pick_or_wait should report that one.
        let pool = KeyPool::new(
            vec!["A".into(), "B".into()],
            Duration::from_millis(200),
        );
        pool.cool("A");
        std::thread::sleep(Duration::from_millis(100));
        pool.cool("B");
        let wait = pool.pick_or_wait().unwrap_err();
        // A cooled ~100ms ago with 200ms cooldown → ~100ms left.
        // B just cooled → ~200ms left.
        // pick_or_wait must return A's remaining (the smaller one).
        assert!(
            wait <= Duration::from_millis(150),
            "should return shortest remaining, got {wait:?}"
        );
    }
}
