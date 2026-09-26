// F-23 (team-lead R18/R19 assignment, "재촬영 선행 조건"). Deterministic, on-demand hitch
// injection for evidence collection - a human alt-tabbing/window-switching cannot reliably land
// inside the window that makes SC-56 (c2) ((c1)∧(c2)∧(c3), F-20's corrected condition -
// pending architect F-22) observable at all:
//
//   snapshot_interval_ticks = 2 (server boot log) = 100 ms floor - a hitch shorter than this
//     resolves BETWEEN two snapshots and the client never sees a drifted ack/tick pair at all
//     (qa r9: 32 server-side carry-forward/supersede branches fired, client saw 1 drift event).
//   carry_forward_max_ticks = 10 (ADR-0011 section 6.1) = 500 ms ceiling - a hitch at or past
//     this switches BOTH sides to the dormant input (zero thrust), so the ship has already
//     decelerated by the time drift is visible - the same trap R18's re-measurement fell into
//     (drift fired only in the session's dormant opening window, never at >=100 m/s).
//
// So the only window that produces a VISIBLE, still-fast drift is (100 ms, 500 ms) exclusive -
// StallMs below is its midpoint, comfortably inside both bounds.
//
// SAFE FOR SHIPPING (why this is not a footgun): GreyboxSession is itself the vertical-slice
// diagnostic harness (its own header: "grey box: no hand-authored scene ... this slice's
// visuals are simple shapes"), not gameplay code - it already reads the keyboard directly for
// flight input the same way (ShipInputSampler.cs's own header: "diagnostic greybox controller,
// not rebindable game-facing input"). This key lives in exactly that file, alongside that same
// caveat: if/when a real input action map or gameplay HUD replaces this harness, this key must
// NOT carry over - it exists to make a specific bug's evidence collectible, not as a feature.
//
// Thread.Sleep on the main thread is not a general-purpose testing primitive here - it is the
// SAME mechanism a real hitch (Editor GC pause, domain reload, shader compile stall) already
// produces (TickCatchUp.cs's own header documents this precisely: "a single hitch ... drained
// and SENT every backlogged tick"), just deterministic and on-demand instead of accidental.
//
// Honest limit (documented once here, not repeated at every call site): an injected hitch has a
// DIFFERENT CAUSE than a real network/frame stall - it proves "does reconciliation hold up once
// drift happens", not "does drift happen this way in real play". The latter is what SC-89's own
// background-window vectors (domain reload, heavy scene load, GC loop) are for; this file does
// not attempt to answer it.

using System.Globalization;

namespace Starfall.Greybox
{
    public static class HitchInjection
    {
        /// <summary>250 ms - the midpoint of the (100 ms, 500 ms) window derived in this file's
        /// header. Not a tuning value: move it and either bound (snapshot cadence, carry-forward
        /// ceiling) can swallow the injection again.</summary>
        // F-25 (qa r9 §H): was 250. qa swept ten injection phases against the snapshot cadence
        // and found 250 ms lands the (drift != 0 AND HasError AND speed >= 100 m/s) triple in
        // the SAME snapshot in only 5 of 10 phases - a coin flip, because a 5-tick blackout can
        // straddle the 2-tick snapshot boundary either way. 400 ms (8 ticks) hits 10 of 10 and
        // still leaves 100 ms of headroom under the 500 ms carry-forward ceiling, past which the
        // server switches to dormant input and the ship decelerates at 7.0 m/s^2 - which would
        // drop it below the 100 m/s visibility threshold and reproduce exactly the vacuous pass
        // this whole round exists to avoid.
        //
        // WHY 400 AND NOT 250 (F-31, architect R10 판정 §3 - closed, record only).
        //
        // 400 ms (8 ticks) was chosen because the R8 live session produced a drift pair on all 8
        // injections at >= 100 m/s (qa r10 §A.1). Two idealised harnesses (qa r9, qa r10)
        // disagreed about WHY - r9 reported a parity law, r10's independent sweep did not
        // reproduce it, and neither model predicted the session's observed 4/8 front-edge, 8/8
        // trailing-edge split. The live observation is the reason; the parity law is not, and
        // must not be restored as a justification for going back to 250.
        //
        // architect's judgement (R10 §3.1): this is a tuning value for an EVIDENCE-COLLECTION
        // tool, not product behaviour - wrong, the game plays identically either way; the only
        // cost of a wrong value is evidence that fails to land. Fixing which idealised model is
        // right would not change 400's justification (it already has a live-session pair, 8/8),
        // so that investigation is out of scope for this slice (CLAUDE.md principles 8, 10).
        // Reopens only if a future round proposes going back to <= 250 ms - that reopening is
        // settled by two live sessions (250 and 400), not by another idealised sweep.
        //
        // The leader's stated floor of 100 ms was also wrong - drift is observable at 50 ms
        // (1 tick) in 5 of 10 phases, so the real floor is "at least one tick must be missed".
        // And the reason small hitches usually produce no drift is not that snapshots "cancel"
        // them: ADR-0012 section 6.2 burns an input_seq for every carried-forward tick, so
        // delta_ack == delta_tick holds by construction. What is left is send/snapshot phase.
        //
        // The 500 ms ceiling does stand, from carry_forward_max_ticks = 10 in
        // data/movement/sync-tuning.json: past it the server switches to dormant input and the
        // ship decelerates. Note qa's model does not enforce that ceiling (it holds 140 m/s even
        // at 600 ms), so the ceiling rests on the arithmetic, not on the sweep table.
        public const int DefaultStallMs = 400;

        /// <summary>One grep-able line: `hitch_injected stall_ms=... tick=... speed_mps=...`.
        /// Captured from the state BEFORE the stall (the tick/speed the ship was actually at
        /// when the injection started) - a judge correlating this against F-21's
        /// reconcile_tick_drift_event lines needs to know what the ship was doing going into the
        /// stall, not coming out of it. No module prefix baked in (same convention as
        /// MarkerHudLine.Format()/ReconcileTickDriftEvent.Format()) - the caller prepends its own
        /// "starfall.xxx:" tag.</summary>
        public static string Format(int stallMs, long tick, double speedMps)
        {
            return "hitch_injected stall_ms=" + stallMs.ToString(CultureInfo.InvariantCulture) +
                   " tick=" + tick.ToString(CultureInfo.InvariantCulture) +
                   " speed_mps=" + speedMps.ToString("F1", CultureInfo.InvariantCulture);
        }
    }
}
