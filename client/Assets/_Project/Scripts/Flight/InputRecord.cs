// Hand-written. One retained, unconfirmed input: what the client sent, and the predicted
// state that resulted immediately after applying it (ADR-0012 section 3's
// "(input_seq, command_id, quantised 11 fields, predicted state after applying it)").

using Starfall.Sim;

namespace Starfall.Flight
{
    public readonly struct InputRecord
    {
        public readonly uint InputSeq;
        public readonly ShipControlInputD Input;

        /// <summary>Predicted ship state immediately after this input was applied. Recomputed
        /// on every reconciliation (Reconciliation.Reconcile), never stale.</summary>
        public readonly ShipSimState StateAfter;

        public InputRecord(uint inputSeq, ShipControlInputD input, ShipSimState stateAfter)
        {
            InputSeq = inputSeq;
            Input = input;
            StateAfter = stateAfter;
        }
    }
}
