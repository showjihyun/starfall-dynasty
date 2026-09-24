// F-27 (architect R10 판정 §1, 01_architect_decisions.md "## R10 판정" - team-lead R21 relay).
// Supersedes the R21-round-1 design (HasError-gated RebaseWithoutErrorTotal) - architect ruled
// that design would have merged two different things (see below), and replaced it with this
// budget: a comparison-less reconcile is not automatically a defect, because the render jump it
// produces can be entirely explained by elapsed time.
//
// The core fact (architect R10 §1.1): a 400 ms hitch makes the ship jump ~49 m on screen not
// because the two integrators disagree, but because the ship ACTUALLY travelled that far at
// 140 m/s while the client was not drawing. A perfectly correct client produces this jump too -
// hiding it would mean hiding the server's own truth (CLAUDE.md principle 1). So
// reconcile_hard_snap_total (which answers "did the two integrators disagree") must not count
// it, and neither should a plain "no comparison available" counter - both would conflate "client
// implementation drifted" with "client was asleep for N ticks", which is the exact defect qa r10
// found in reconcile_tick_drift_max (F-28).
//
// What DOES distinguish a real defect from an explained jump is whether elapsed time accounts
// for the distance: explained_m = (predicted speed just before rebasing) x behind_ticks x dt.
// behind_ticks is how many ticks the client's own local tick index (PredictedShipController.
// CurrentTickIndex) was sitting behind the confirmed snapshot's tick when this reconcile started -
// zero for the normal, healthy case (client had already predicted at or past the snapshot tick),
// positive whenever the client fell behind (a hitch, a forced rebase, an Editor domain reload).
//
// No new threshold: the "still too big after accounting for elapsed time" budget reuses
// reconcile_hard_snap_threshold_m (5.0 m) as its slack - when behind_ticks == 0 (explained_m ==
// 0) this collapses to EXACTLY the same gate reconcile_hard_snap_total already uses, so the two
// counters converge to one threshold's strength without sharing one counter's identity.

namespace Starfall.Flight
{
    public static class ReconcileRebaseJumpBudget
    {
        /// <summary>How many ticks the client's own local tick index was behind the snapshot it
        /// is rebasing on, clamped to zero (never negative - a client that had already predicted
        /// PAST the snapshot tick is not "behind" by any amount, it is ahead, which is the normal
        /// case).</summary>
        public static long BehindTicks(long snapshotTick, long currentTickIndexBeforeReconcile) =>
            snapshotTick > currentTickIndexBeforeReconcile ? snapshotTick - currentTickIndexBeforeReconcile : 0L;

        /// <summary>The portion of a render jump that elapsed, un-drawn time alone accounts for -
        /// distance a ship moving at <paramref name="speedMpsBeforeReconcile"/> covers over
        /// <paramref name="behindTicks"/> ticks. Zero when behindTicks is zero (the client was
        /// never behind), regardless of speed.</summary>
        public static double ExplainedM(double speedMpsBeforeReconcile, long behindTicks, double dt) =>
            speedMpsBeforeReconcile * behindTicks * dt;

        /// <summary>True when a rebase jump is bigger than elapsed time can account for, even
        /// after the same slack reconcile_hard_snap_total uses (<paramref name="hardSnapThresholdM"/>,
        /// 5.0 m - no new constant). At behindTicks == 0 (explainedM == 0) this is EXACTLY the
        /// hard-snap gate; the budget only widens as the client falls further behind.</summary>
        public static bool IsUnexplained(double rebaseJumpM, double explainedM, double hardSnapThresholdM) =>
            rebaseJumpM > explainedM + hardSnapThresholdM;
    }
}
