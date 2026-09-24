// C-1 (R8 판정 D-3 / ADR-0012 section 6.4 point 6). Pure formula tests for
// Starfall.Flight.ReconcileTickDrift.Compute - see that file's header for why this exists and
// why it must survive the D-1/D-2 tick-alignment fix unchanged.

using NUnit.Framework;
using Starfall.Flight;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconcileTickDriftTests
    {
        [Test]
        public void Compute_FirstSnapshot_PrevAckNull_ReturnsNull()
        {
            // No baseline yet - nothing to compare (same discipline as Reconciliation.Result.HasError).
            Assert.That(ReconcileTickDrift.Compute(prevAck: null, prevTick: 100, ack: 5, tick: 105), Is.Null);
        }

        [Test]
        public void Compute_CurrentSnapshotAckNull_ReturnsNull()
        {
            // Server has not applied anything from this session yet.
            Assert.That(ReconcileTickDrift.Compute(prevAck: 5, prevTick: 100, ack: null, tick: 105), Is.Null);
        }

        [Test]
        public void Compute_OneToOne_IsZero()
        {
            // Normal path: 100 ticks passed, 100 commands acked - the "1 seq = 1 server tick"
            // assumption R8 판정 A-2 showed is only sometimes true.
            Assert.That(ReconcileTickDrift.Compute(prevAck: 200, prevTick: 598652, ack: 300, tick: 598752),
                Is.EqualTo(0L));
        }

        [Test]
        public void Compute_ServerCarriedForward_IsNegative()
        {
            // R6 row5 (08_qa_report_r8.md §2.3): dTick=100, dAck=99 - one tick where the server
            // carried the last input forward instead of acking a new one. drift = 99 - 100 = -1.
            Assert.That(ReconcileTickDrift.Compute(prevAck: 310, prevTick: 607624, ack: 409, tick: 607724),
                Is.EqualTo(-1L));
        }

        [Test]
        public void Compute_ServerSuperseded_IsPositive()
        {
            // Two commands arrived in the same server tick; ack advances by 2 for 1 tick of
            // elapsed time. drift = +1.
            Assert.That(ReconcileTickDrift.Compute(prevAck: 10, prevTick: 1000, ack: 12, tick: 1001),
                Is.EqualTo(1L));
        }

        [Test]
        public void Compute_ClientTruncate_IsLargeAndNegative()
        {
            // A 6-second Editor freeze (R8 판정 §B-1): ~125 ticks pass server-side while the
            // client predicts and sends almost nothing during the truncated window.
            Assert.That(ReconcileTickDrift.Compute(prevAck: 4780, prevTick: 500000, ack: 4785, tick: 500125),
                Is.EqualTo(-120L));
        }

        [Test]
        public void Compute_IsSymmetricAroundZero_AbsoluteValueIsWhatCallerTracks()
        {
            long? negative = ReconcileTickDrift.Compute(prevAck: 0, prevTick: 0, ack: 5, tick: 10);
            long? positive = ReconcileTickDrift.Compute(prevAck: 0, prevTick: 0, ack: 15, tick: 10);

            Assert.That(negative, Is.EqualTo(-5L));
            Assert.That(positive, Is.EqualTo(5L));
        }
    }
}
