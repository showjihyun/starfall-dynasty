// H-15/H-16 (architect R4 보충 판정 §D / K-1(4)). Covers the two additions to
// PredictionHistory: DropRejected now also drops carry-forward-derived entries, and EnforceCap
// bounds the retained history.

using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Flight;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class PredictionHistoryTests
    {
        static readonly ShipSimState Dummy = ShipSimState.Zero;
        static ShipControlInputD Input(uint seq) => new ShipControlInputD(seq, 0, 0, 1000, 0, 0, 0, 0, 1_000_000, false, true);
        static InputRecord Sent(uint seq) => new InputRecord(seq, Input(seq), Dummy, derivedFromSeq: null);
        static InputRecord CarriedForward(uint seq, uint fromSeq) => new InputRecord(seq, Input(seq), Dummy, derivedFromSeq: fromSeq);

        [Test]
        public void DropRejected_AlsoDropsEntriesDerivedFromTheRejectedSeq()
        {
            var history = new List<InputRecord> { Sent(1), CarriedForward(2, 1), CarriedForward(3, 1), Sent(4) };

            List<InputRecord> kept = PredictionHistory.DropRejected(history, rejectedInputSeq: 1);

            Assert.That(kept.Count, Is.EqualTo(1));
            Assert.That(kept[0].InputSeq, Is.EqualTo(4u),
                "seq 1 and everything carried forward from it (2, 3) must all be gone");
        }

        [Test]
        public void DropRejected_UnrelatedCarryForwardEntries_AreNotTouched()
        {
            var history = new List<InputRecord> { Sent(1), CarriedForward(2, 1), Sent(5), CarriedForward(6, 5) };

            List<InputRecord> kept = PredictionHistory.DropRejected(history, rejectedInputSeq: 1);

            Assert.That(kept.Count, Is.EqualTo(2));
            Assert.That(kept[0].InputSeq, Is.EqualTo(5u));
            Assert.That(kept[1].InputSeq, Is.EqualTo(6u));
        }

        [Test]
        public void EnforceCap_UnderTheLimit_ReturnsAnUnchangedCopyAndDropsZero()
        {
            var history = new List<InputRecord> { Sent(1), Sent(2) };
            (List<InputRecord> trimmed, int dropped) = PredictionHistory.EnforceCap(history, maxEntries: 30);

            Assert.That(dropped, Is.EqualTo(0));
            Assert.That(trimmed.Count, Is.EqualTo(2));
            Assert.That(trimmed, Is.Not.SameAs(history), "must return a copy, not alias the caller's list (SC-53 discipline)");
        }

        [Test]
        public void EnforceCap_OverTheLimit_DropsTheOldestEntriesFirst()
        {
            var history = new List<InputRecord>();
            for (uint seq = 1; seq <= 35; seq++) history.Add(Sent(seq));

            (List<InputRecord> trimmed, int dropped) = PredictionHistory.EnforceCap(history, maxEntries: 30);

            Assert.That(dropped, Is.EqualTo(5));
            Assert.That(trimmed.Count, Is.EqualTo(30));
            Assert.That(trimmed[0].InputSeq, Is.EqualTo(6u), "the oldest 5 (seq 1-5) must be the ones dropped");
            Assert.That(trimmed[29].InputSeq, Is.EqualTo(35u));
        }

        [Test]
        public void EnforceCap_ExactlyAtTheLimit_DropsNothing()
        {
            var history = new List<InputRecord>();
            for (uint seq = 1; seq <= 30; seq++) history.Add(Sent(seq));

            (List<InputRecord> trimmed, int dropped) = PredictionHistory.EnforceCap(history, maxEntries: 30);

            Assert.That(dropped, Is.EqualTo(0));
            Assert.That(trimmed.Count, Is.EqualTo(30));
        }
    }
}
