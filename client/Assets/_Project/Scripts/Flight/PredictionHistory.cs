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
        public static (ShipSimState NewState, List<InputRecord> NewHistory) ApplyInput(
            IReadOnlyList<InputRecord> history,
            ShipSimState currentState,
            ShipControlInputD input,
            ShipClassStats ship,
            ShipIntegrator.Boundary boundary,
            double dt)
        {
            ShipSimState next = ShipIntegrator.Step(currentState, input, ship, boundary, dt).State;

            var newHistory = new List<InputRecord>(history.Count + 1);
            newHistory.AddRange(history);
            newHistory.Add(new InputRecord(input.InputSeq, input, next));

            return (next, newHistory);
        }

        /// <summary>Drops one input from the history without reconciling against a snapshot -
        /// the COMMAND_RESULT{REJECTED, *} path (ADR-0012 section 3): the server never applied
        /// this input, so replaying it would keep the client running ahead of the server by
        /// exactly the input this removes.</summary>
        public static List<InputRecord> DropRejected(IReadOnlyList<InputRecord> history, uint rejectedInputSeq)
        {
            var kept = new List<InputRecord>(history.Count);
            foreach (InputRecord record in history)
                if (record.InputSeq != rejectedInputSeq)
                    kept.Add(record);
            return kept;
        }
    }
}
