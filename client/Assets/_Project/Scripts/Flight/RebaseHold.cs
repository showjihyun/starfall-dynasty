// H-2 (T-2, architect R4 판정 §5 + R4 보충 판정 §D). A separate pure predicate from
// Reconciliation.Reconcile itself - Reconcile's purity (SC-53) is not touched by this file.
//
// Why this exists: Reconcile assumes "unconfirmed history count == server ticks between the
// confirmed tick and the client's current predicted tick". That holds when 1 sent seq == 1
// server tick. During a carry-forward run the server keeps advancing ticks WITHOUT ack_input_seq
// moving (nothing new arrived to ack) - the client predicted extra ticks the confirmed snapshot
// cannot yet "explain" in terms of ack_input_seq. Rebasing against that snapshot either
// over-replays (carry-forward entries counted twice) or under-replays (they're dropped and the
// distance is lost) - both exceed the 5m hard-snap threshold (R4 판정 §5).
//
// The fix: don't rebase on a snapshot that can't yet resolve the un-sent entries. Wait for the
// first snapshot whose ack_input_seq has advanced past them (their source command was sent and
// acknowledged), then rebase normally - replay count and server tick count line up exactly. A
// hold has a hard 500ms (carry_forward_max_ticks) ceiling: past that, force the rebase anyway,
// discarding every un-sent (carry-forward-derived) entry outright rather than guessing.

using System.Collections.Generic;

namespace Starfall.Flight
{
    public static class RebaseHold
    {
        /// <summary>carry_forward_max_ticks in seconds (500ms) - same ceiling the server uses
        /// for carry-forward expiry (ADR-0011 section 6.1), reused here as the hold timeout.</summary>
        public const double ForcedRebaseAfterSeconds = 0.5;

        public enum Action
        {
            /// <summary>No un-sent (carry-forward-derived) entry is unresolved by this
            /// snapshot's ack_input_seq - rebase normally.</summary>
            RebaseNormally,

            /// <summary>An un-sent entry exists with seq > ack_input_seq and the hold has not
            /// hit its timeout yet - do not call Reconcile this snapshot. Keep predicting.</summary>
            HoldAndKeepPredicting,

            /// <summary>The hold exceeded ForcedRebaseAfterSeconds - rebase now, but first
            /// discard every un-sent entry (DerivedFromSeq != null) from the history handed to
            /// Reconcile, and the caller counts reconcile_forced_after_hitch_total.</summary>
            ForceRebaseDiscardingUnsent,
        }

        /// <summary>Pure. <paramref name="heldSecondsSoFar"/> is the caller's own running total
        /// of real time spent holding (0 if this is the first snapshot being evaluated for a
        /// hold) - passed in rather than read from a clock here, so this stays a pure function
        /// of its arguments like Reconciliation.Reconcile.</summary>
        public static Action Evaluate(IReadOnlyList<InputRecord> history, uint? ackInputSeq, double heldSecondsSoFar)
        {
            bool hasUnresolvedUnsentEntry = HasUnresolvedUnsentEntry(history, ackInputSeq);
            if (!hasUnresolvedUnsentEntry) return Action.RebaseNormally;

            return heldSecondsSoFar >= ForcedRebaseAfterSeconds
                ? Action.ForceRebaseDiscardingUnsent
                : Action.HoldAndKeepPredicting;
        }

        static bool HasUnresolvedUnsentEntry(IReadOnlyList<InputRecord> history, uint? ackInputSeq)
        {
            for (int i = 0; i < history.Count; i++)
            {
                InputRecord record = history[i];
                if (!record.WasNotSent) continue;
                if (!ackInputSeq.HasValue || record.InputSeq > ackInputSeq.Value) return true;
            }
            return false;
        }

        /// <summary>For <see cref="Action.ForceRebaseDiscardingUnsent"/>: strips every
        /// carry-forward/dormant (never-sent) entry before handing the history to
        /// Reconciliation.Reconcile, so only entries the server could plausibly have seen are
        /// replayed.</summary>
        public static List<InputRecord> DiscardUnsentEntries(IReadOnlyList<InputRecord> history)
        {
            var kept = new List<InputRecord>(history.Count);
            foreach (InputRecord record in history)
                if (!record.WasNotSent) kept.Add(record);
            return kept;
        }
    }
}
