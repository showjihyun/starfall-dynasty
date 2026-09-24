// H-2 (T-2, architect R4 판정 §5 + R4 보충 판정 K-1). Tests RebaseHold.Evaluate directly - pure,
// no ShipIntegrator/PredictedShipController needed since the predicate only looks at
// InputRecord.InputSeq/DerivedFromSeq and ackInputSeq.

using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Flight;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RebaseHoldTests
    {
        static readonly ShipSimState Dummy = ShipSimState.Zero;

        static ShipControlInputD Input(uint seq) => new ShipControlInputD(seq, 0, 0, 1000, 0, 0, 0, 0, 1_000_000, false, true);

        static InputRecord Sent(uint seq) => new InputRecord(seq, Input(seq), Dummy, derivedFromSeq: null);
        static InputRecord CarriedForward(uint seq, uint fromSeq) => new InputRecord(seq, Input(seq), Dummy, derivedFromSeq: fromSeq);

        [Test]
        public void Evaluate_NoUnsentEntries_RebasesNormally()
        {
            var history = new List<InputRecord> { Sent(1), Sent(2), Sent(3) };
            Assert.That(RebaseHold.Evaluate(history, ackInputSeq: 2, heldSecondsSoFar: 0.0),
                Is.EqualTo(RebaseHold.Action.RebaseNormally));
        }

        [Test]
        public void Evaluate_UnsentEntryAheadOfAck_HoldsBeforeTimeout()
        {
            // seq 5-9 carried forward from the last real send (seq 4), seq 10 the next real
            // send - ack hasn't reached the carry-forward run yet.
            var history = new List<InputRecord> { Sent(4), CarriedForward(5, 4), CarriedForward(6, 4), Sent(10) };
            Assert.That(RebaseHold.Evaluate(history, ackInputSeq: 4, heldSecondsSoFar: 0.1),
                Is.EqualTo(RebaseHold.Action.HoldAndKeepPredicting));
        }

        [Test]
        public void Evaluate_AckHasAdvancedPastTheUnsentEntries_RebasesNormally()
        {
            // The carry-forward run's source (seq 4) is now acked, and every carried-forward
            // seq (5,6) is <= ack - nothing left unresolved.
            var history = new List<InputRecord> { CarriedForward(5, 4), CarriedForward(6, 4), Sent(10) };
            Assert.That(RebaseHold.Evaluate(history, ackInputSeq: 6, heldSecondsSoFar: 0.0),
                Is.EqualTo(RebaseHold.Action.RebaseNormally));
        }

        [Test]
        public void Evaluate_UnsentEntryAheadOfAck_ButAckIsNull_Holds()
        {
            var history = new List<InputRecord> { CarriedForward(1, 1) };
            Assert.That(RebaseHold.Evaluate(history, ackInputSeq: null, heldSecondsSoFar: 0.0),
                Is.EqualTo(RebaseHold.Action.HoldAndKeepPredicting));
        }

        [Test]
        public void Evaluate_HeldPastFiveHundredMs_ForcesRebase()
        {
            var history = new List<InputRecord> { CarriedForward(5, 4) };
            Assert.That(RebaseHold.Evaluate(history, ackInputSeq: 4, heldSecondsSoFar: 0.5),
                Is.EqualTo(RebaseHold.Action.ForceRebaseDiscardingUnsent));
            Assert.That(RebaseHold.Evaluate(history, ackInputSeq: 4, heldSecondsSoFar: 0.4999),
                Is.EqualTo(RebaseHold.Action.HoldAndKeepPredicting),
                "the 500ms ceiling is carry_forward_max_ticks - must not fire early");
        }

        [Test]
        public void DiscardUnsentEntries_KeepsOnlyActuallySentEntries()
        {
            var history = new List<InputRecord> { Sent(1), CarriedForward(2, 1), Sent(3), CarriedForward(4, 3) };
            List<InputRecord> kept = RebaseHold.DiscardUnsentEntries(history);

            Assert.That(kept.Count, Is.EqualTo(2));
            Assert.That(kept[0].InputSeq, Is.EqualTo(1u));
            Assert.That(kept[1].InputSeq, Is.EqualTo(3u));
        }
    }
}
