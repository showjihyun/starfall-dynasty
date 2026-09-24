// Hand-written. Appends one predicted tick to the retained-input history. Pure: takes an
// immutable history and returns a new one, never mutates the caller's list - the same
// discipline Reconciliation.cs uses, and for the same reason (SC-53: reconciliation, and
// everything that feeds it, must be provably a pure function).

using System.Collections.Generic;
using Starfall.Sim;

namespace Starfall.Flight
{
    public static class PredictionHistory
    {
        /// <summary>Advances <paramref name="currentState"/> by exactly one tick with
        /// <paramref name="input"/> (ADR-0012 section 6: one input sent = one tick predicted,
        /// never tied to render frame rate) and appends the result to a NEW list.</summary>
        /// <param name="derivedFromSeq">H-15: null when <paramref name="input"/>.InputSeq was
        /// actually sent this tick. Set to the source command's seq when this tick is a
        /// carry-forward/dormant prediction that was never sent (TickCatchUp rules 4/5).</param>
        public static (ShipSimState NewState, List<InputRecord> NewHistory) ApplyInput(
            IReadOnlyList<InputRecord> history,
            ShipSimState currentState,
            ShipControlInputD input,
            ShipClassStats ship,
            ShipIntegrator.Boundary boundary,
            double dt,
            uint? derivedFromSeq = null)
        {
            ShipSimState next = ShipIntegrator.Step(currentState, input, ship, boundary, dt).State;

            var newHistory = new List<InputRecord>(history.Count + 1);
            newHistory.AddRange(history);
            newHistory.Add(new InputRecord(input.InputSeq, input, next, derivedFromSeq));

            return (next, newHistory);
        }

        /// <summary>Drops one input from the history without reconciling against a snapshot -
        /// the COMMAND_RESULT{REJECTED, *} path (ADR-0012 section 3): the server never applied
        /// this input, so replaying it would keep the client running ahead of the server by
        /// exactly the input this removes. H-15: also drops every entry CARRY-FORWARD-DERIVED
        /// from the rejected seq (DerivedFromSeq == rejectedInputSeq) - those predicted with an
        /// input the server never saw either, for the same reason.</summary>
        public static List<InputRecord> DropRejected(IReadOnlyList<InputRecord> history, uint rejectedInputSeq)
        {
            var kept = new List<InputRecord>(history.Count);
            foreach (InputRecord record in history)
                if (record.InputSeq != rejectedInputSeq && record.DerivedFromSeq != rejectedInputSeq)
                    kept.Add(record);
            return kept;
        }

        /// <summary>H-16 (architect K-1 (4)): the retained history must never exceed
        /// carry_forward_max_ticks + M (10 + 20 = 30 in production) entries - the worst case of
        /// the two bounded exit paths combined (an ack, or the rebase-hold forced timeout).
        /// Trims the OLDEST entries first. Returns the trimmed list and how many were dropped
        /// (the caller counts prediction_history_overflow_total - this must be 0 on any normal
        /// path, so a caller-visible count is the whole point).</summary>
        public static (List<InputRecord> Trimmed, int Dropped) EnforceCap(IReadOnlyList<InputRecord> history, int maxEntries)
        {
            int excess = history.Count - maxEntries;
            if (excess <= 0) return (new List<InputRecord>(history), 0);

            var trimmed = new List<InputRecord>(maxEntries);
            for (int i = excess; i < history.Count; i++) trimmed.Add(history[i]);
            return (trimmed, excess);
        }
    }
}
