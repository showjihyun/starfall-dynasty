// F-27 (architect R10 판정 §1, 01_architect_decisions.md "## R10 판정" - team-lead R21 relay).
// Supersedes the R21-round-1 ReconcileRebaseJumpEvent (gated on Reconciliation.Result.HasError)
// - architect's model gates on behind_ticks instead: whether the client's own local tick index
// was behind the snapshot it just rebased on (Flight.ReconcileRebaseJumpBudget.BehindTicks), not
// on whether step 1 happened to find a comparison. The two conditions usually coincide in
// practice but are not the same thing, and the budget model is defined purely in terms of tick
// arithmetic - this event follows that, not HasError.
//
// One line per behind-reconcile, carrying enough for a reader to check the SC-56 (c4) decision
// themselves: the jump, how far behind the client was, how much of the jump elapsed time alone
// explains, and whether that leaves an unexplained remainder (Flight.ReconcileRebaseJumpBudget.
// IsUnexplained - the exact boolean that drives PredictedShipController.UnexplainedJumpTotal).
//
// §7b(1) discipline: the gate (behindTicks > 0) lives in TryCreate, not at the Debug.Log call
// site - "always emit, filter with an if() outside" would make the negative half untestable
// without a live scene (ReconcileClientBehindEventTests.cs's negative-contrast case).

using System.Globalization;

namespace Starfall.Greybox
{
    public readonly struct ReconcileClientBehindEvent
    {
        public readonly double RebaseJumpM;
        public readonly long SnapshotTick;
        public readonly long BehindTicks;
        public readonly long DeltaTick;
        public readonly double SpeedMps;
        public readonly double ExplainedM;

        /// <summary>Flight.ReconcileRebaseJumpBudget.IsUnexplained's own verdict for this event -
        /// the same boolean that drove PredictedShipController.UnexplainedJumpTotal for this
        /// reconcile, carried verbatim (never re-derived here) so a reader never has to redo the
        /// arithmetic to know whether this line is "explained, reported for context" or
        /// "the SC-56 (c4) gate just tripped".</summary>
        public readonly bool Unexplained;

        ReconcileClientBehindEvent(double rebaseJumpM, long snapshotTick, long behindTicks, long deltaTick,
            double speedMps, double explainedM, bool unexplained)
        {
            RebaseJumpM = rebaseJumpM;
            SnapshotTick = snapshotTick;
            BehindTicks = behindTicks;
            DeltaTick = deltaTick;
            SpeedMps = speedMps;
            ExplainedM = explainedM;
            Unexplained = unexplained;
        }

        /// <summary>Null whenever <paramref name="behindTicks"/> is not positive - the client had
        /// already predicted at or past the snapshot's tick (the normal, healthy case). Non-null
        /// for EVERY behind-reconcile regardless of <paramref name="unexplained"/> - qa r10's
        /// instruction was to make the condition readable, not to pre-filter by which ones turn
        /// out to be a problem (that judgement belongs to reconcile_client_behind_total's report
        /// requirement, which does not gate on == 0).</summary>
        public static ReconcileClientBehindEvent? TryCreate(
            long behindTicks, double rebaseJumpM, long snapshotTick, long deltaTick, double speedMps,
            double explainedM, bool unexplained)
        {
            if (behindTicks <= 0) return null;
            return new ReconcileClientBehindEvent(rebaseJumpM, snapshotTick, behindTicks, deltaTick, speedMps, explainedM, unexplained);
        }

        /// <summary>One key=value line, grep-able by `reconcile_client_behind_event`. No module
        /// prefix baked in (same convention as ReconcileTickDriftEvent.Format()) - each caller
        /// (GreyboxSession, ObserverSession) prepends its own "starfall.xxx:" tag.</summary>
        public string Format()
        {
            return "reconcile_client_behind_event rebase_jump_m=" + RebaseJumpM.ToString("F4", CultureInfo.InvariantCulture) +
                   " snapshot_tick=" + SnapshotTick.ToString(CultureInfo.InvariantCulture) +
                   " behind_ticks=" + BehindTicks.ToString(CultureInfo.InvariantCulture) +
                   " delta_tick=" + DeltaTick.ToString(CultureInfo.InvariantCulture) +
                   " speed_mps=" + SpeedMps.ToString("F1", CultureInfo.InvariantCulture) +
                   " explained_m=" + ExplainedM.ToString("F4", CultureInfo.InvariantCulture) +
                   " unexplained=" + (Unexplained ? "true" : "false");
        }
    }
}
