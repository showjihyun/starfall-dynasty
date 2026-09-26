// F-27 (architect R10 판정 §1, team-lead R21). Tests for Starfall.Flight.ReconcileRebaseJumpBudget
// - see that file's header for why it exists (a comparison-less rebase jump is only a defect
// when elapsed time cannot explain it - qa r10's 49 m/400 ms jump is fully explained; a genuinely
// desynced client would not be).

using NUnit.Framework;
using Starfall.Flight;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconcileRebaseJumpBudgetTests
    {
        const double Dt = 1.0 / 20.0;
        const double HardSnapThresholdM = 5.0;

        // ------------------------------------------------------------------ BehindTicks

        [Test]
        public void BehindTicks_ClientAlreadyPastSnapshotTick_IsZero()
        {
            // The normal, healthy case: the client had already predicted at or past the
            // snapshot's tick when the snapshot arrived - not "behind" by any amount.
            Assert.That(ReconcileRebaseJumpBudget.BehindTicks(snapshotTick: 100, currentTickIndexBeforeReconcile: 105), Is.EqualTo(0L));
            Assert.That(ReconcileRebaseJumpBudget.BehindTicks(snapshotTick: 100, currentTickIndexBeforeReconcile: 100), Is.EqualTo(0L),
                "exactly caught up must also read zero, not negative-clamped-to-zero by accident");
        }

        [Test]
        public void BehindTicks_ClientFellBehind_IsThePositiveDifference()
        {
            // Shape of qa r10 §D.2's 400 ms case: an 8-tick gap between the snapshot's tick and
            // the client's own local index at the moment it started rebasing.
            Assert.That(ReconcileRebaseJumpBudget.BehindTicks(snapshotTick: 1050346, currentTickIndexBeforeReconcile: 1050338), Is.EqualTo(8L));
        }

        // ------------------------------------------------------------------ ExplainedM

        [Test]
        public void ExplainedM_ZeroBehindTicks_IsZero_RegardlessOfSpeed()
        {
            Assert.That(ReconcileRebaseJumpBudget.ExplainedM(speedMpsBeforeReconcile: 140.0, behindTicks: 0, dt: Dt), Is.EqualTo(0.0));
        }

        [Test]
        public void ExplainedM_MatchesQaR10sOwnArithmetic()
        {
            // qa r10 §D.2: 400 ms stall (8 ticks) at 140 m/s -> 49.00 m = 7 * 140 * 0.05... the
            // exact number qa r10 used was |drift| * 140 * 0.05, but the underlying physics is
            // speed * ticks * dt - 8 ticks * 0.05 s/tick * 140 m/s = 56.0 m for a FULLY missed
            // 400 ms window (qa's 49 m used drift=7, one tick less - both are the same formula).
            Assert.That(ReconcileRebaseJumpBudget.ExplainedM(speedMpsBeforeReconcile: 140.0, behindTicks: 8, dt: Dt),
                Is.EqualTo(56.0).Within(1e-9));
            Assert.That(ReconcileRebaseJumpBudget.ExplainedM(speedMpsBeforeReconcile: 140.0, behindTicks: 7, dt: Dt),
                Is.EqualTo(49.0).Within(1e-9));
        }

        // ------------------------------------------------------------------ IsUnexplained: negative half first (§7b(1))

        [Test]
        public void IsUnexplained_JumpFullyWithinExplainedPlusSlack_IsFalse()
        {
            // qa r10's 49 m/400 ms case: explainedM = 49.0 (7 ticks), the observed jump was
            // ALSO 49.0 - well within the +5.0 m slack. A perfectly correct client landing here.
            Assert.That(ReconcileRebaseJumpBudget.IsUnexplained(rebaseJumpM: 49.0, explainedM: 49.0, hardSnapThresholdM: HardSnapThresholdM), Is.False);
        }

        [Test]
        public void IsUnexplained_JumpExactlyAtBudgetBoundary_IsFalse()
        {
            // Strictly-greater-than: sitting exactly on explainedM + slack must not FAIL - same
            // "boundary reads as the safe side" discipline as RenderOffset's band edges.
            Assert.That(ReconcileRebaseJumpBudget.IsUnexplained(rebaseJumpM: 54.0, explainedM: 49.0, hardSnapThresholdM: HardSnapThresholdM), Is.False);
        }

        [Test]
        public void IsUnexplained_ZeroBehindTicks_CollapsesToTheHardSnapThreshold()
        {
            // architect's stated convergence (R10 §1.3): at explainedM == 0, this gate is
            // EXACTLY reconcile_hard_snap_total's own threshold - a jump of 5.0 m with nothing
            // explaining it must not FAIL (at the threshold), 5.01 m must.
            Assert.That(ReconcileRebaseJumpBudget.IsUnexplained(rebaseJumpM: 5.0, explainedM: 0.0, hardSnapThresholdM: HardSnapThresholdM), Is.False);
            Assert.That(ReconcileRebaseJumpBudget.IsUnexplained(rebaseJumpM: 5.01, explainedM: 0.0, hardSnapThresholdM: HardSnapThresholdM), Is.True);
        }

        // ------------------------------------------------------------------ IsUnexplained: positive half

        [Test]
        public void IsUnexplained_JumpBiggerThanElapsedTimeCanAccountFor_IsTrue()
        {
            // R8's original defect, re-expressed in this model: behind_ticks == 0 (the client had
            // already predicted that tick) but the two integrators disagreed by 49 m - nothing
            // explains it, this is exactly the case (c4)/F-27 exists to catch.
            Assert.That(ReconcileRebaseJumpBudget.IsUnexplained(rebaseJumpM: 49.0, explainedM: 0.0, hardSnapThresholdM: HardSnapThresholdM), Is.True);
        }

        [Test]
        public void IsUnexplained_PartiallyExplainedButStillTooBig_IsTrue()
        {
            // Behind by some ticks (explainedM > 0) AND a real desync on top - the budget must
            // not let a nonzero explainedM excuse an unrelated extra jump.
            Assert.That(ReconcileRebaseJumpBudget.IsUnexplained(rebaseJumpM: 100.0, explainedM: 49.0, hardSnapThresholdM: HardSnapThresholdM), Is.True);
        }
    }
}
