use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Async token-bucket rate limiter — units are caller-defined.
///
/// A request only proceeds after `acquire(cost)` hands it `cost` worth of
/// tokens. Tokens refill at `rate_per_sec` (continuous, sub-second
/// precision) up to `burst` capacity. When not enough are available,
/// `acquire` computes the wait until refill and sleeps. The std mutex is
/// released **before** sleeping so this stays async-safe under contention.
///
/// Unit semantics are left to the caller:
/// * Etherscan-style providers throttle by **requests/sec** → cost = 1.0
///   per request, rate = requests/sec.
/// * Alchemy throttles by **compute units/sec** (CU/s) → cost = CU per
///   method (eth_getCode = 19, trace_filter = 75, …), rate = 500 (free
///   tier), burst = 5000 (Alchemy's 10s window). A batched JSON-RPC call
///   costs the **sum** of inner method CUs — a batch of 100 eth_getCode
///   is 1900 CU, which a req/sec-counting bucket would let through 100×
///   too fast.
///
/// Unlike a tokio Semaphore (which caps concurrency), this caps *rate*:
/// fast-completing requests cannot push throughput above `rate_per_sec`
/// even with concurrency = 1.
#[derive(Debug)]
pub struct RateLimiter {
    rate_per_sec: f64,
    burst: f64,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    last_refill: Instant,
    tokens: f64,
}

impl RateLimiter {
    /// `rate_per_sec`: steady-state requests-per-second the bucket allows.
    /// `burst`: max tokens accumulated when idle. Clamps both to ≥ a tiny
    /// positive number so misconfiguration (zero / negative) becomes
    /// unbounded-slow rather than divide-by-zero.
    pub fn new(rate_per_sec: f64, burst: f64) -> Self {
        let rate_per_sec = rate_per_sec.max(0.001);
        let burst = burst.max(1.0);
        Self {
            rate_per_sec,
            burst,
            state: Mutex::new(State {
                last_refill: Instant::now(),
                tokens: burst,
            }),
        }
    }

    pub fn rate_per_sec(&self) -> f64 {
        self.rate_per_sec
    }

    pub fn burst(&self) -> f64 {
        self.burst
    }

    /// Block until `cost` worth of tokens are available; consume them and
    /// return. `cost` is caller-defined: requests-per-second sources pass
    /// 1.0, CU-per-second sources pass the method's CU cost. `cost` is
    /// clamped to `burst` so a single oversized call can't deadlock the
    /// bucket (extra cost is forgiven; you'll see waits but not a hang).
    pub async fn acquire(&self, cost: f64) {
        let cost = cost.max(0.0).min(self.burst);
        loop {
            let wait = {
                let mut s = self.state.lock().expect("rate limiter mutex poisoned");
                let now = Instant::now();
                let elapsed = now.duration_since(s.last_refill).as_secs_f64();
                s.tokens = (s.tokens + elapsed * self.rate_per_sec).min(self.burst);
                s.last_refill = now;
                if s.tokens >= cost {
                    s.tokens -= cost;
                    return;
                }
                let need = cost - s.tokens;
                let secs = need / self.rate_per_sec;
                Duration::from_millis(((secs * 1000.0).ceil() as u64).max(1))
            };
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn first_burst_drains_without_waiting() {
        // Bucket starts full (burst tokens). With burst=3, the first 3
        // acquires of cost=1 must return without inducing a sleep.
        let rl = RateLimiter::new(1.0, 3.0);
        let start = Instant::now();
        for _ in 0..3 {
            rl.acquire(1.0).await;
        }
        assert!(
            start.elapsed() < Duration::from_millis(50),
            "first burst should be near-instant, took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn after_burst_we_wait_for_refill() {
        // burst=1, rate=10 tok/s → one token immediately, then ~100ms wait
        // for the next. Small absolute number → less wall-clock jitter.
        let rl = RateLimiter::new(10.0, 1.0);
        rl.acquire(1.0).await;

        let before = Instant::now();
        rl.acquire(1.0).await;
        let elapsed = before.elapsed();
        assert!(
            elapsed >= Duration::from_millis(80) && elapsed <= Duration::from_millis(250),
            "expected ~100ms wait, got {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn steady_state_matches_configured_rate() {
        // Drain burst, then measure 5 more tokens at 50 tok/s → ~100ms.
        let rl = RateLimiter::new(50.0, 1.0);
        rl.acquire(1.0).await; // drain burst

        let before = Instant::now();
        for _ in 0..5 {
            rl.acquire(1.0).await;
        }
        let elapsed = before.elapsed();
        assert!(
            elapsed >= Duration::from_millis(80) && elapsed <= Duration::from_millis(300),
            "expected ~100ms for 5 tokens at 50/s, got {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn cost_aware_call_waits_proportionally() {
        // burst=10, rate=100/s. A cost=10 call drains the bucket; a
        // subsequent cost=5 needs ~50ms refill before it can proceed.
        let rl = RateLimiter::new(100.0, 10.0);
        rl.acquire(10.0).await; // drain

        let before = Instant::now();
        rl.acquire(5.0).await;
        let elapsed = before.elapsed();
        assert!(
            elapsed >= Duration::from_millis(40) && elapsed <= Duration::from_millis(150),
            "expected ~50ms for cost=5 at 100/s, got {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn oversized_cost_is_clamped_to_burst_not_hung() {
        // cost > burst would never be satisfiable in a strict token bucket
        // (the bucket caps at burst). Clamping to burst means it just
        // drains everything — the test verifies it doesn't hang.
        let rl = RateLimiter::new(1000.0, 5.0);
        let before = Instant::now();
        rl.acquire(100.0).await; // way more than burst
        assert!(
            before.elapsed() < Duration::from_millis(50),
            "oversized cost on full bucket should return promptly"
        );
    }

    #[test]
    fn negative_or_zero_config_does_not_panic() {
        let rl = RateLimiter::new(0.0, 0.0);
        assert!(rl.rate_per_sec() > 0.0);
        assert!(rl.burst() >= 1.0);
    }
}
