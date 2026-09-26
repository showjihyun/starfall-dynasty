// C-1 (R8 판정 D-3 / ADR-0012 section 6.4 point 6). Pure. Measures how far the two axes that
// Reconciliation.Reconcile used to treat as interchangeable (ack_input_seq and server tick)
// actually move apart between two consecutive APPLIED snapshots (snapshots that made it past
// RebaseHold, not ones that were held).
//
// drift = (ack - prev_ack) - (tick - prev_tick). Normal value is 0: for every server tick that
// passed, exactly one command was newly acknowledged. Non-zero means one of R8 판정's three
// divergence paths fired between the two snapshots - server carry-forward (ack moved less than
// tick), server supersede (ack moved more than tick), or client truncate (ack moved much less
// than tick) - ADR-0012 section 6.4.
//
// This instrumentation goes in BEFORE the D-1/D-2 tick-alignment fix (architect's explicit
// ordering, 01_architect_decisions.md "R8 판정" C-1) and stays in AFTER it: pre-fix it is the
// thing that proves the divergence this whole round chases is real and nonzero in the running
// client, not just in the R5/R6 evidence logs; post-fix it is the only way to tell
// "reconcile_hard_snap_total == 0" apart from "this session never went fast enough to see the
// bug" (R8 판정 관측 3 - the 5.0m threshold divided by 0.05s dt is a 100 m/s visibility floor).
// The tick-alignment fix changes WHAT Reconciliation.Reconcile keys off; it does not change
// whether ack and tick drift apart on the wire, so this counter's meaning survives the fix
// unchanged.

namespace Starfall.Flight
{
    public static class ReconcileTickDrift
    {
        /// <summary>Null when either endpoint has no ack yet - nothing to compare, the same
        /// "no error to report, not zero error" discipline Reconciliation.Result.HasError uses
        /// for the first snapshot after a connect/reconnect. Otherwise the signed drift for this
        /// interval; the caller (GreyboxSession) counts non-zero occurrences
        /// (reconcile_tick_drift_total) and tracks the running max |drift| (reconcile_tick_drift_max).</summary>
        public static long? Compute(uint? prevAck, long prevTick, uint? ack, long tick)
        {
            if (!prevAck.HasValue || !ack.HasValue) return null;

            long deltaAck = (long)ack.Value - (long)prevAck.Value;
            long deltaTick = tick - prevTick;
            return deltaAck - deltaTick;
        }
    }
}
