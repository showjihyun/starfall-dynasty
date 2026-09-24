// F-27 (architect R10 판정 §1, team-lead R21). Tests for Starfall.Greybox.ReconcileClientBehindEvent
// - see that file's header for why it exists and why it gates on behind_ticks rather than
// Reconciliation.Result.HasError (the R21-round-1 design this supersedes).

using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconcileClientBehindEventTests
    {
        // ------------------------------------------------------------------ §7b(1): the negative half comes first

        [Test]
        public void TryCreate_BehindTicksIsZero_ReturnsNull_EvenWithALargeJump()
        {
            // The normal, healthy case (client already caught up) - must produce nothing, even
            // if the jump itself happens to be huge (that is HardSnapTotal/UnexplainedJumpTotal's
            // business, not this event's).
            Assert.That(ReconcileClientBehindEvent.TryCreate(
                behindTicks: 0, rebaseJumpM: 391.56, snapshotTick: 1050790, deltaTick: 0, speedMps: 140.0,
                explainedM: 0.0, unexplained: true), Is.Null);
        }

        [Test]
        public void TryCreate_BehindTicksIsNegative_ReturnsNull()
        {
            // BehindTicks() itself never returns negative, but TryCreate's own gate must not
            // assume that - defend the boundary directly.
            Assert.That(ReconcileClientBehindEvent.TryCreate(
                behindTicks: -1, rebaseJumpM: 10.0, snapshotTick: 5, deltaTick: 0, speedMps: 10.0,
                explainedM: 0.0, unexplained: false), Is.Null);
        }

        // ------------------------------------------------------------------ the positive half

        [Test]
        public void TryCreate_BehindTicksPositive_ReturnsAnEvent_RegardlessOfUnexplained()
        {
            // qa r10's instruction was "make the condition readable", not "only when it's a
            // problem" - both explained and unexplained behind-reconciles must produce a line.
            Assert.That(ReconcileClientBehindEvent.TryCreate(
                behindTicks: 7, rebaseJumpM: 49.0, snapshotTick: 1050346, deltaTick: 8, speedMps: 140.0,
                explainedM: 49.0, unexplained: false), Is.Not.Null);
            Assert.That(ReconcileClientBehindEvent.TryCreate(
                behindTicks: 7, rebaseJumpM: 200.0, snapshotTick: 1050346, deltaTick: 8, speedMps: 140.0,
                explainedM: 49.0, unexplained: true), Is.Not.Null);
        }

        [Test]
        public void TryCreate_CarriesTheMeasuredValuesVerbatim()
        {
            ReconcileClientBehindEvent? evt = ReconcileClientBehindEvent.TryCreate(
                behindTicks: 7, rebaseJumpM: 49.0, snapshotTick: 1050346, deltaTick: 8, speedMps: 140.0,
                explainedM: 49.0, unexplained: false);

            Assert.That(evt, Is.Not.Null);
            Assert.That(evt.Value.BehindTicks, Is.EqualTo(7L));
            Assert.That(evt.Value.RebaseJumpM, Is.EqualTo(49.0));
            Assert.That(evt.Value.SnapshotTick, Is.EqualTo(1050346L));
            Assert.That(evt.Value.DeltaTick, Is.EqualTo(8L));
            Assert.That(evt.Value.SpeedMps, Is.EqualTo(140.0));
            Assert.That(evt.Value.ExplainedM, Is.EqualTo(49.0));
            Assert.That(evt.Value.Unexplained, Is.False);
        }

        // ------------------------------------------------------------------ Format()

        [Test]
        public void Format_ContainsEveryField()
        {
            ReconcileClientBehindEvent evt = ReconcileClientBehindEvent.TryCreate(
                behindTicks: 90, rebaseJumpM: 391.56, snapshotTick: 1050790, deltaTick: 91, speedMps: 122.9,
                explainedM: 391.5, unexplained: true).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("reconcile_client_behind_event"));
            Assert.That(line, Does.Contain("rebase_jump_m=391.5600"));
            Assert.That(line, Does.Contain("snapshot_tick=1050790"));
            Assert.That(line, Does.Contain("behind_ticks=90"));
            Assert.That(line, Does.Contain("delta_tick=91"));
            Assert.That(line, Does.Contain("speed_mps=122.9"));
            Assert.That(line, Does.Contain("explained_m=391.5000"));
            Assert.That(line, Does.Contain("unexplained=true"));
            Assert.That(line, Does.Not.Contain("\n"), "must be one grep-able line");
        }

        [Test]
        public void Format_UnexplainedFalse_PrintsFalse_NotTrue()
        {
            // §7b(1) pairing for the assertion above - a Format() that hard-coded "true" would
            // pass the positive test alone.
            ReconcileClientBehindEvent evt = ReconcileClientBehindEvent.TryCreate(
                behindTicks: 7, rebaseJumpM: 49.0, snapshotTick: 7, deltaTick: 8, speedMps: 140.0,
                explainedM: 49.0, unexplained: false).Value;

            Assert.That(evt.Format(), Does.Contain("unexplained=false"));
        }

        [Test]
        public void Format_SnapshotTickAndBehindTicks_DoNotAlias()
        {
            ReconcileClientBehindEvent evt = ReconcileClientBehindEvent.TryCreate(
                behindTicks: 555, rebaseJumpM: 1.0, snapshotTick: 777, deltaTick: 1, speedMps: 1.0,
                explainedM: 0.0, unexplained: false).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("snapshot_tick=777"));
            Assert.That(line, Does.Contain("behind_ticks=555"));
            Assert.That(line, Does.Not.Contain("snapshot_tick=555"));
            Assert.That(line, Does.Not.Contain("behind_ticks=777"));
        }
    }
}
