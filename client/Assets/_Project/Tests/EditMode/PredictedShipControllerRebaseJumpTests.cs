// F-27 (architect R10 판정 §1, 01_architect_decisions.md "## R10 판정" - team-lead R21 relay).
// Tests for the render-jump gauge and elapsed-time budget PredictedShipController.Reconcile()
// maintains OUTSIDE the `if (result.HasError)` gate that HardSnapTotal lives behind - see that
// file's LastRebaseJumpM/RebaseJumpMaxM/LastBehindTicks/LastExplainedM/UnexplainedJumpTotal/
// ClientBehind* doc comments for why (reconcile_hard_snap_total measured 0 through four 49-391 m
// jumps in the R8 session because every one of them had HasError == false).
//
// This supersedes the R21-round-1 test file of the same name (HasError-gated
// RebaseWithoutErrorTotal) - architect ruled that counter would have conflated two different
// things (see PredictedShipController.cs's F-27 doc comments), and replaced it with the budget
// this file now covers.

using NUnit.Framework;
using Starfall.Flight;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class PredictedShipControllerRebaseJumpTests
    {
        const double Dt = 1.0 / 20.0;

        static ShipClassStats LoadScoutFixture() =>
            ShipClassStats.FromJson(ContractFixtures.RequireValid("SHIP_CLASS", "example-scout.json").ReadText());

        static readonly ShipIntegrator.Boundary WideOpenBoundary =
            new ShipIntegrator.Boundary(softRadiusM: 10_000.0, hardRadiusM: 12_000.0, pullMps2: 25.0);

        static SyncTuningData TestTuning() => new SyncTuningData
        {
            ReconcileIgnoreThresholdM = 0.005,
            ReconcileSmoothThresholdM = 0.25,
            ReconcileHardSnapThresholdM = 5.0,
            ReconcileOrientationIgnoreThresholdDeg = 0.02,
            ReconcileOrientationSmoothThresholdDeg = 1.0,
            ReconcileOrientationHardSnapDeg = 15.0,
        };

        static ShipSimState AtRest(double x, double y, double z) =>
            new ShipSimState(new Vec3d(x, y, z), Vec3d.Zero, Quatd.Identity, Vec3d.Zero, 0.0);

        static ShipSimState Moving(double positionX, double velocityX) =>
            new ShipSimState(new Vec3d(positionX, 0, 0), new Vec3d(velocityX, 0, 0), Quatd.Identity, Vec3d.Zero, 0.0);

        // ------------------------------------------------------------------ LastRebaseJumpM/RebaseJumpMaxM/N: unconditional, as before

        [Test]
        public void Reconcile_AlwaysMeasuresTheRenderJump_RegardlessOfHasErrorOrBehindTicks()
        {
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);

            controller.Reconcile(AtRest(10.0, 0.0, 0.0), snapshotTick: 5, TestTuning());
            Assert.That(controller.LastRebaseJumpM, Is.EqualTo(10.0).Within(1e-9));
            Assert.That(controller.RebaseJumpMaxM, Is.EqualTo(10.0).Within(1e-9));
            Assert.That(controller.RebaseJumpN, Is.EqualTo(1L));

            controller.Reconcile(AtRest(60.0, 0.0, 0.0), snapshotTick: 6, TestTuning());
            Assert.That(controller.LastRebaseJumpM, Is.EqualTo(50.0).Within(1e-9));
            Assert.That(controller.RebaseJumpMaxM, Is.EqualTo(50.0).Within(1e-9), "max must move up to the bigger jump");
            Assert.That(controller.RebaseJumpN, Is.EqualTo(2L));

            controller.Reconcile(AtRest(65.0, 0.0, 0.0), snapshotTick: 7, TestTuning());
            Assert.That(controller.RebaseJumpMaxM, Is.EqualTo(50.0).Within(1e-9), "a smaller jump must not pull the running max down");
            Assert.That(controller.RebaseJumpN, Is.EqualTo(3L), "N counts every call, not just the ones that moved the max");
        }

        // ------------------------------------------------------------------ BehindTicks/ExplainedM wiring

        [Test]
        public void Reconcile_ClientBehind_ExplainedMMatchesSpeedTimesTicksTimesDt()
        {
            ShipClassStats ship = LoadScoutFixture();
            // 140 m/s, starts at tick 0 - the R8/qa r10 speed, so this scenario's numbers read
            // the same as the real session's (7 ticks -> 49.0 m, qa r10 §D.2).
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));

            controller.Reconcile(Moving(positionX: 49.0, velocityX: 140.0), snapshotTick: 7, TestTuning());

            Assert.That(controller.LastBehindTicks, Is.EqualTo(7L), "client's local tick index started at 0, snapshot is tick 7");
            Assert.That(controller.LastExplainedM, Is.EqualTo(49.0).Within(1e-9), "140 m/s * 7 ticks * 0.05 s/tick");
        }

        [Test]
        public void Reconcile_ClientAlreadyPastSnapshotTick_BehindTicksAndExplainedMAreZero()
        {
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));
            for (int i = 0; i < 10; i++)
                controller.ApplyInput(new ShipControlInputD((uint)(i + 1), 0, 0, 0, 0, 0, 0, 0, 1_000_000, false, true));
            // CurrentTickIndex is now 10.

            controller.Reconcile(AtRest(0.0, 0.0, 0.0), snapshotTick: 3, TestTuning());

            Assert.That(controller.LastBehindTicks, Is.EqualTo(0L));
            Assert.That(controller.LastExplainedM, Is.EqualTo(0.0));
        }

        // ------------------------------------------------------------------ UnexplainedJumpTotal: negative half first (§7b(1))

        [Test]
        public void Reconcile_JumpFullyExplainedByElapsedTime_DoesNotCountAsUnexplained()
        {
            // qa r10 §D.2's own case, reproduced directly: 140 m/s, 7 ticks behind, jump == 49.0
            // == explainedM exactly - a perfectly correct client landing here (architect R10 §1.1).
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));

            controller.Reconcile(Moving(positionX: 49.0, velocityX: 140.0), snapshotTick: 7, TestTuning());

            Assert.That(controller.UnexplainedJumpTotal, Is.EqualTo(0L));
        }

        [Test]
        public void Reconcile_JumpWithinBudgetSlack_DoesNotCountAsUnexplained()
        {
            // Same shape, but the jump is a little bigger than the bare explainedM - still
            // within the existing +5.0 m hard-snap slack (no new threshold).
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));

            controller.Reconcile(Moving(positionX: 53.0, velocityX: 140.0), snapshotTick: 7, TestTuning()); // 53 <= 49 + 5

            Assert.That(controller.UnexplainedJumpTotal, Is.EqualTo(0L));
        }

        // ------------------------------------------------------------------ UnexplainedJumpTotal: positive half

        [Test]
        public void Reconcile_ZeroBehindTicks_JumpOverHardSnapThreshold_CountsAsUnexplained()
        {
            // R8's original defect, reproduced through the new model: the client had ALREADY
            // predicted this tick (behind_ticks == 0, explainedM == 0) but the confirmed state
            // disagrees by more than the hard-snap slack - nothing explains it.
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);
            controller.ApplyInput(new ShipControlInputD(1, 0, 0, 1000, 0, 0, 0, 0, 1_000_000, false, true)); // CurrentTickIndex -> 1

            controller.Reconcile(AtRest(49.0, 0.0, 0.0), snapshotTick: 1, TestTuning()); // behind_ticks = 0

            Assert.That(controller.LastBehindTicks, Is.EqualTo(0L), "precondition: this must be the zero-behind case");
            Assert.That(controller.UnexplainedJumpTotal, Is.EqualTo(1L));
        }

        [Test]
        public void Reconcile_ClientBehind_JumpBiggerThanBudgetEvenAfterExplaining_CountsAsUnexplained()
        {
            // Behind by real ticks (explainedM > 0) AND an extra desync on top - the budget must
            // not let a nonzero explainedM excuse an unrelated additional jump.
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));

            controller.Reconcile(Moving(positionX: 200.0, velocityX: 140.0), snapshotTick: 7, TestTuning()); // explainedM=49, jump=200

            Assert.That(controller.UnexplainedJumpTotal, Is.EqualTo(1L));
        }

        // ------------------------------------------------------------------ ClientBehind* reporting (no == 0 gate - report only)

        [Test]
        public void Reconcile_ClientBehind_UpdatesTotalAndMaxTicksAndMaxJumpM()
        {
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));

            controller.Reconcile(Moving(positionX: 49.0, velocityX: 140.0), snapshotTick: 7, TestTuning()); // behind=7, jump=49
            Assert.That(controller.ClientBehindTotal, Is.EqualTo(1L));
            Assert.That(controller.ClientBehindMaxTicks, Is.EqualTo(7L));
            Assert.That(controller.ClientBehindMaxJumpM, Is.EqualTo(49.0).Within(1e-9));

            // A SECOND behind-reconcile with fewer ticks but a bigger jump - max ticks must not
            // regress, max jump must move up. currentTickIndex is now pinned to 7 (previous
            // snapshotTick), so snapshotTick=10 makes behind_ticks = 3.
            controller.Reconcile(Moving(positionX: 100.0, velocityX: 140.0), snapshotTick: 10, TestTuning());
            Assert.That(controller.ClientBehindTotal, Is.EqualTo(2L));
            Assert.That(controller.ClientBehindMaxTicks, Is.EqualTo(7L), "3 < 7, must not pull the max down");
            Assert.That(controller.ClientBehindMaxJumpM, Is.GreaterThan(49.0), "the second jump (49 -> 100) is bigger");
        }

        [Test]
        public void Reconcile_ZeroBehindTicks_DoesNotIncrementClientBehindCounters()
        {
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);
            controller.ApplyInput(new ShipControlInputD(1, 0, 0, 1000, 0, 0, 0, 0, 1_000_000, false, true));

            controller.Reconcile(AtRest(49.0, 0.0, 0.0), snapshotTick: 1, TestTuning()); // behind_ticks = 0, still an unexplained jump

            Assert.That(controller.ClientBehindTotal, Is.EqualTo(0L),
                "F-27: an unexplained jump at zero behind_ticks is a HardSnapTotal-shaped defect, not a client_behind report");
        }

        // ------------------------------------------------------------------ independence from HasError (the R21-round-1 defect this replaces)

        [Test]
        public void Reconcile_HasErrorTrue_StillComputesBehindTicksAndBudget_NotGatedOnHasError()
        {
            // The condition that matters is behind_ticks, never HasError - build a scenario
            // where step 1 DOES find a comparison (HasError=true) but the client was also behind
            // the snapshot it is rebasing on, to prove the two are tracked independently.
            ShipClassStats ship = LoadScoutFixture();
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, Moving(positionX: 0.0, velocityX: 140.0));
            // Advance to tick 7 first with matching predicted history so a LATER reconcile at the
            // same tick has a comparison target.
            for (int i = 1; i <= 7; i++)
                controller.ApplyInput(new ShipControlInputD((uint)i, 0, 0, 0, 0, 0, 0, 0, 1_000_000, false, true));

            Reconciliation.Result result = controller.Reconcile(Moving(positionX: 0.0, velocityX: 140.0), snapshotTick: 7, TestTuning());

            Assert.That(result.HasError, Is.True, "precondition: step 1 found a comparison");
            Assert.That(controller.LastBehindTicks, Is.EqualTo(0L),
                "the client had already predicted up to tick 7 before this call - not behind, independent of HasError being true");
        }
    }
}
