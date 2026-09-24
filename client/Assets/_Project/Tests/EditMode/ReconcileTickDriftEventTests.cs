// F-21 (team-lead R18, qa r9) + R19 field additions (team-lead, architect R9 residual model).
// Tests for Starfall.Greybox.ReconcileTickDriftEvent - see that file's header for why it exists
// (the periodic reconcile_tick_drift_total could not answer WHEN drift happened or at what
// speed, which is exactly what stopped SC-56 closing in R17's re-measurement) and why
// delta_tick/thrust_x/y/z/roll were added (architect's residual model needs to see whether the
// server plausibly applied a DIFFERENT input at the drifted tick to judge a (0.25, 5.0] m
// reading 미검증 rather than FAIL).

using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconcileTickDriftEventTests
    {
        // ------------------------------------------------------------------ §7b(1): the negative half comes first

        [Test]
        public void TryCreate_DriftIsZero_ReturnsNull()
        {
            // The normal case (1:1 reconcile) - must produce NOTHING, not a line reading "drift=0".
            // A formatter that always emits, gated only by an `if` at the call site, would make
            // this half of the feature untestable without a live scene - so the gate lives here.
            Assert.That(ReconcileTickDriftEvent.TryCreate(drift: 0, deltaTick: 1, tick: 100, speedMps: 50.0,
                hasError: true, positionErrorM: 0.001, orientationErrorDeg: 0.001,
                thrustX: 0, thrustY: 0, thrustZ: 1000, roll: 0, rebaseJumpM: 0.0, behindTicks: 0), Is.Null);
        }

        [Test]
        public void TryCreate_DriftIsNull_ReturnsNull()
        {
            // No baseline yet (first snapshot after connect/reconnect) - same "nothing to report"
            // discipline as ReconcileTickDrift.Compute itself.
            Assert.That(ReconcileTickDriftEvent.TryCreate(drift: null, deltaTick: 1, tick: 100, speedMps: 50.0,
                hasError: true, positionErrorM: 0.001, orientationErrorDeg: 0.001,
                thrustX: 0, thrustY: 0, thrustZ: 1000, roll: 0, rebaseJumpM: 0.0, behindTicks: 0), Is.Null);
        }

        // ------------------------------------------------------------------ the positive half

        [Test]
        public void TryCreate_DriftIsNonZero_ReturnsAnEvent()
        {
            ReconcileTickDriftEvent? evt = ReconcileTickDriftEvent.TryCreate(drift: -1, deltaTick: 2, tick: 607724, speedMps: 140.0,
                hasError: true, positionErrorM: 7.0005, orientationErrorDeg: 0.02,
                thrustX: 0, thrustY: 0, thrustZ: 1000, roll: 0, rebaseJumpM: 0.0, behindTicks: 0);

            Assert.That(evt, Is.Not.Null);
            Assert.That(evt.Value.Drift, Is.EqualTo(-1L));
            Assert.That(evt.Value.DeltaTick, Is.EqualTo(2L));
            Assert.That(evt.Value.Tick, Is.EqualTo(607724L));
            Assert.That(evt.Value.SpeedMps, Is.EqualTo(140.0));
        }

        [Test]
        public void TryCreate_NegativeDrift_IsNotTreatedAsZero()
        {
            // A naive `drift != 0` check done wrong (e.g. comparing the wrong sign or an
            // unsigned cast) could swallow negative drift specifically - carry-forward's shape
            // (R8 판정: drift = -1) is exactly the failure this must not have.
            Assert.That(ReconcileTickDriftEvent.TryCreate(drift: -1, deltaTick: 1, tick: 1, speedMps: 1.0,
                hasError: false, positionErrorM: 0, orientationErrorDeg: 0,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0, rebaseJumpM: 0.0, behindTicks: 0), Is.Not.Null);
            Assert.That(ReconcileTickDriftEvent.TryCreate(drift: 1, deltaTick: 1, tick: 1, speedMps: 1.0,
                hasError: false, positionErrorM: 0, orientationErrorDeg: 0,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0, rebaseJumpM: 0.0, behindTicks: 0), Is.Not.Null);
        }

        // ------------------------------------------------------------------ Format()

        [Test]
        public void Format_ContainsEveryField()
        {
            // Every value distinct and non-zero, same §7a discipline PeriodicStatusLogTests uses
            // (F-8) - a Format() that dropped or swapped a field cannot still produce a passing line.
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: -1, deltaTick: 2, tick: 607724, speedMps: 140.0,
                hasError: true, positionErrorM: 7.0005, orientationErrorDeg: 0.02,
                thrustX: 111, thrustY: 222, thrustZ: 333, roll: 444, rebaseJumpM: 49.0, behindTicks: 7).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("reconcile_tick_drift_event"));
            Assert.That(line, Does.Contain("drift=-1"));
            Assert.That(line, Does.Contain("delta_tick=2"));
            Assert.That(line, Does.Contain("tick=607724"));
            Assert.That(line, Does.Contain("speed_mps=140.0"));
            Assert.That(line, Does.Contain("position_error_m=7.0005"));
            Assert.That(line, Does.Contain("orientation_error_deg=0.0200"));
            Assert.That(line, Does.Contain("thrust_x=111"));
            Assert.That(line, Does.Contain("thrust_y=222"));
            Assert.That(line, Does.Contain("thrust_z=333"));
            Assert.That(line, Does.Contain("roll=444"));
            Assert.That(line, Does.Contain("rebase_jump_m=49.0000"));
            Assert.That(line, Does.Contain("behind_ticks=7"));
            Assert.That(line, Does.Not.Contain("\n"), "must be one grep-able line");
        }

        [Test]
        public void Format_RebaseJumpMAndBehindTicks_PrintEvenWhenHasErrorFalse()
        {
            // R21 (architect R10 판정 §1.4 table row 4): the whole point - rebase_jump_m and
            // behind_ticks must NOT go missing or print "n/a" alongside
            // position_error_m=n/a/orientation_error_deg=n/a. Both are different, always-measured
            // quantities (PredictedShipController.LastRebaseJumpM/LastBehindTicks).
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: -7, deltaTick: 8, tick: 1050346, speedMps: 140.0,
                hasError: false, positionErrorM: 0, orientationErrorDeg: 0,
                thrustX: 0, thrustY: 0, thrustZ: 1000, roll: 0, rebaseJumpM: 49.00, behindTicks: 7).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("position_error_m=n/a"));
            Assert.That(line, Does.Contain("rebase_jump_m=49.0000"), "must carry a real number even though the error fields are n/a");
            Assert.That(line, Does.Contain("behind_ticks=7"), "must carry a real number even though the error fields are n/a");
        }

        [Test]
        public void Format_RebaseJumpMAndBehindTicks_DoNotAlias()
        {
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: 1, deltaTick: 1, tick: 1, speedMps: 1.0,
                hasError: false, positionErrorM: 0, orientationErrorDeg: 0,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0, rebaseJumpM: 49.0, behindTicks: 12).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("rebase_jump_m=49.0000"));
            Assert.That(line, Does.Contain("behind_ticks=12"));
            Assert.That(line, Does.Not.Contain("behind_ticks=49"));
        }

        [Test]
        public void Format_DeltaTickAndDrift_DoNotAlias()
        {
            // R19: delta_tick and drift are both small signed integers in a real session - a
            // formatter that swapped the two arguments would still "look plausible". Distinct
            // values catch that (same shape as HitchInjectionTests' stall_ms/tick guard).
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: 5, deltaTick: 9, tick: 1, speedMps: 0,
                hasError: false, positionErrorM: 0, orientationErrorDeg: 0,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0, rebaseJumpM: 0.0, behindTicks: 0).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("drift=5"));
            Assert.That(line, Does.Contain("delta_tick=9"));
            Assert.That(line, Does.Not.Contain("drift=9"));
            Assert.That(line, Does.Not.Contain("delta_tick=5"));
        }

        [Test]
        public void Format_HasErrorFalse_PrintsNotAvailable_NotZero()
        {
            // §7a: a reconcile whose step 1 found no matching retained entry (HasError=false)
            // must not print position_error_m=0.0000 - that would read as a MEASURED zero.
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: 1, deltaTick: 1, tick: 5, speedMps: 10.0,
                hasError: false, positionErrorM: 0.0, orientationErrorDeg: 0.0,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0, rebaseJumpM: 0.0, behindTicks: 0).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("position_error_m=n/a"));
            Assert.That(line, Does.Contain("orientation_error_deg=n/a"));
        }

        [Test]
        public void Format_HasErrorTrue_PrintsTheActualNumbers()
        {
            // §7b(1) pairing for the test above: a Format() that always printed "n/a" would pass
            // that test trivially. This is the other half.
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: 1, deltaTick: 1, tick: 5, speedMps: 10.0,
                hasError: true, positionErrorM: 1.2345, orientationErrorDeg: 6.789,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0, rebaseJumpM: 0.0, behindTicks: 0).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("position_error_m=1.2345"));
            Assert.That(line, Does.Contain("orientation_error_deg=6.7890"));
        }

        [Test]
        public void Format_NegativeThrustAndRoll_KeepTheirSign()
        {
            // R19: a reversed-input tick is exactly the "server applied a DIFFERENT input" case
            // the architect residual model needs to see - the sign is the payload (same
            // discipline PeriodicStatusLogTests.Format_NegativeThrustAndRoll_KeepTheirSign uses).
            ReconcileTickDriftEvent evt = ReconcileTickDriftEvent.TryCreate(drift: 1, deltaTick: 1, tick: 5, speedMps: 10.0,
                hasError: false, positionErrorM: 0, orientationErrorDeg: 0,
                thrustX: -111, thrustY: -222, thrustZ: -1000, roll: -1000, rebaseJumpM: 0.0, behindTicks: 0).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("thrust_x=-111"));
            Assert.That(line, Does.Contain("thrust_y=-222"));
            Assert.That(line, Does.Contain("thrust_z=-1000"));
            Assert.That(line, Does.Contain("roll=-1000"));
        }
    }
}
