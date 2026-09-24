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

        /// <summary>H-15 (architect R4 보충 판정 §D / K-1): null when this entry's own
        /// <see cref="InputSeq"/> was actually sent to the server. Set to the source command's
        /// input_seq when this tick was NOT sent and instead predicted with a carried-forward
        /// or dormant input (TickCatchUp rules 4/5, K-1) - so <see cref="PredictionHistory.DropRejected"/>
        /// can drop entries derived from a command the server rejected, not just that command's
        /// own entry.</summary>
        public readonly uint? DerivedFromSeq;

        public InputRecord(uint inputSeq, ShipControlInputD input, ShipSimState stateAfter, uint? derivedFromSeq = null)
        {
            InputSeq = inputSeq;
            Input = input;
            StateAfter = stateAfter;
            DerivedFromSeq = derivedFromSeq;
        }

        /// <summary>True when this tick was never actually sent to the server (carry-forward or
        /// dormant-input prediction) - the seq that carries the server's actual acknowledgement
        /// is <see cref="DerivedFromSeq"/>, not this entry's own <see cref="InputSeq"/>.</summary>
        public bool WasNotSent => DerivedFromSeq.HasValue;
    }
}
