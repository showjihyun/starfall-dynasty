//! Text profiler (docs/HARNESS.md, "텍스트 프로파일러"): nested timing spans reported as a text tree,
//! so performance work is read and compared as text by people and agents alike.
//!
//! ```ignore
//! shared::prof::enable(true);
//! { let _s = shared::prof::span("tick"); { let _w = shared::prof::span("weapons"); /* … */ } }
//! print!("{}", shared::prof::take().report(400, "tick"));
//! ```
//!
//! Spans nest per thread (each thread has its own stack; their roots are listed side by side). When
//! profiling is off, `span` costs one atomic load and records nothing. Timing never feeds back into
//! game state, so it cannot affect determinism.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static ENABLED: AtomicBool = AtomicBool::new(false);
static STATS: Mutex<BTreeMap<Vec<&'static str>, Stat>> = Mutex::new(BTreeMap::new());

thread_local! {
    static STACK: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stat {
    pub calls: u64,
    pub ns: u128,
}

pub fn enable(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Also turns profiling on when `STARFALL_PROF` is set (any value but `0`).
pub fn enable_from_env() {
    if std::env::var("STARFALL_PROF").is_ok_and(|v| v != "0") {
        enable(true);
    }
}

/// Times the enclosing scope as `name`, nested under the spans open on this thread.
#[must_use = "the span measures until it is dropped"]
pub struct Span {
    start: Option<Instant>,
}

pub fn span(name: &'static str) -> Span {
    if !enabled() {
        return Span { start: None };
    }
    STACK.with(|s| s.borrow_mut().push(name));
    Span { start: Some(Instant::now()) }
}

impl Drop for Span {
    fn drop(&mut self) {
        let Some(start) = self.start else { return };
        let ns = start.elapsed().as_nanos();
        let path = STACK.with(|s| {
            let mut s = s.borrow_mut();
            let path = s.clone();
            s.pop();
            path
        });
        let mut stats = STATS.lock().unwrap();
        let st = stats.entry(path).or_default();
        st.calls += 1;
        st.ns += ns;
    }
}

/// Everything recorded since the last `take`, which starts a fresh measurement.
pub fn take() -> Profile {
    Profile { stats: std::mem::take(&mut *STATS.lock().unwrap()) }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    /// Span path (root first) → calls and total nanoseconds.
    pub stats: BTreeMap<Vec<&'static str>, Stat>,
}

impl Profile {
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Total of a path, if recorded.
    pub fn get(&self, path: &[&str]) -> Option<Stat> {
        self.stats.iter().find(|(p, _)| p.as_slice() == path).map(|(_, s)| *s)
    }

    /// Time inside a span that none of its child spans account for.
    pub fn self_ns(&self, path: &[&'static str]) -> u128 {
        let total = self.get(path).map(|s| s.ns).unwrap_or(0);
        let children: u128 = self.stats.iter().filter(|(p, _)| p.len() == path.len() + 1 && p.starts_with(path)).map(|(_, s)| s.ns).sum();
        total.saturating_sub(children)
    }

    /// Text report: the span tree (children by time, largest first) with calls, total and self time,
    /// time per `unit` (frames or ticks, `per` of them) and share of all root time, then the spans
    /// with the most self time.
    pub fn report(&self, per: u64, unit: &str) -> String {
        let per = per.max(1) as f64;
        let roots: u128 = self.stats.iter().filter(|(p, _)| p.len() == 1).map(|(_, s)| s.ns).sum();
        let pct = |ns: u128| if roots > 0 { ns as f64 * 100.0 / roots as f64 } else { 0.0 };
        let mut out = format!("{:<44} {:>9} {:>11} {:>10} {:>12} {:>6}\n", "span", "calls", "total ms", "self ms", format!("ms/{unit}"), "%");
        fn walk(p: &Profile, path: &mut Vec<&'static str>, out: &mut String, per: f64, pct: &dyn Fn(u128) -> f64) {
            let mut kids: Vec<(&Vec<&'static str>, &Stat)> = p.stats.iter().filter(|(k, _)| k.len() == path.len() + 1 && k.starts_with(path)).collect();
            kids.sort_by(|a, b| b.1.ns.cmp(&a.1.ns).then(a.0.cmp(b.0)));
            for (k, s) in kids {
                let name = format!("{}{}", "  ".repeat(path.len()), k.last().unwrap());
                let self_ns = p.self_ns(k);
                out.push_str(&format!("{:<44} {:>9} {:>11.2} {:>10.2} {:>12.3} {:>5.1}%\n", name, s.calls, s.ns as f64 / 1e6, self_ns as f64 / 1e6, s.ns as f64 / 1e6 / per, pct(s.ns)));
                path.push(k.last().unwrap());
                walk(p, path, out, per, pct);
                path.pop();
            }
        }
        walk(self, &mut vec![], &mut out, per, &pct);
        let mut hot: Vec<(String, u128)> = self.stats.keys().map(|k| (k.join("/"), self.self_ns(k))).filter(|(_, ns)| *ns > 0).collect();
        hot.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.push_str("hottest (self time):\n");
        for (k, ns) in hot.iter().take(8) {
            out.push_str(&format!("  {:>6.1}%  {:>9.3} ms/{unit}  {k}\n", pct(*ns), *ns as f64 / 1e6 / per));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy(us: u64) {
        let t = Instant::now();
        while t.elapsed().as_micros() < us as u128 {
            std::hint::black_box(0);
        }
    }

    #[test]
    fn spans_nest_and_report_self_time() {
        // One test owns the global profiler (others in this crate never open spans).
        let _ = take();
        enable(true);
        for _ in 0..3 {
            let _t = span("tick");
            busy(200);
            {
                let _w = span("weapons");
                busy(600);
            }
            let _a = span("ai");
            busy(100);
        }
        enable(false);
        {
            let _off = span("ignored");
        }
        let p = take();
        assert!(take().is_empty(), "take resets");
        assert_eq!(p.get(&["tick"]).unwrap().calls, 3);
        assert_eq!(p.get(&["tick", "weapons"]).unwrap().calls, 3);
        assert!(p.get(&["ignored"]).is_none(), "nothing recorded while off");
        let total = p.get(&["tick"]).unwrap().ns;
        let weapons = p.get(&["tick", "weapons"]).unwrap().ns;
        assert!(weapons >= 1_800_000 && weapons < total);
        assert_eq!(p.self_ns(&["tick"]), total - weapons - p.get(&["tick", "ai"]).unwrap().ns);
        let r = p.report(3, "tick");
        let lines: Vec<&str> = r.lines().collect();
        assert!(lines[1].starts_with("tick "), "{r}");
        assert!(lines[2].starts_with("  weapons "), "children sorted by time: {r}");
        assert!(lines[3].starts_with("  ai "), "{r}");
        assert!(r.contains("100.0%"), "{r}");
        assert!(r.contains("hottest (self time):\n") && r.contains("tick/weapons"), "{r}");
    }
}
