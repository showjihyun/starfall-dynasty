// F-29 (qa r10 §H, team-lead R21). Tests for Starfall.Greybox.ReconcileErrorThresholdEvent - see
// that file's header for why it exists (qa r10 could not identify WHICH reconcile produced the
// R8 session's 0.3151 m max position_error_m, because that snapshot had drift == 0 and
// ReconcileTickDriftEvent only logs on drift != 0).

using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconcileErrorThresholdEventTests
    {
        // Reused designer constants (sync-tuning.json / GreyboxSession.DefaultTuning()) - not new
        // tunables invented for this event (team-lead R21 instruction).
        const double PositionThresholdM = 0.25;
        const double OrientationThresholdDeg = 1.0;

        // ------------------------------------------------------------------ §7b(1): the negative half comes first

        [Test]
        public void TryCreate_HasErrorFalse_ReturnsNull_EvenWayOverThreshold()
        {
            // Nothing was measured this reconcile - there is no error value to compare against a
            // threshold. Must not log "n/a is over the threshold".
            Assert.That(ReconcileErrorThresholdEvent.TryCreate(
                hasError: false, tick: 100, speedMps: 140.0, positionErrorM: 999.0, orientationErrorDeg: 999.0,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg), Is.Null);
        }

        [Test]
        public void TryCreate_HasErrorTrue_BothErrorsUnderThreshold_ReturnsNull()
        {
            // The overwhelming common case (near-1:1 reconcile) - must produce nothing.
            Assert.That(ReconcileErrorThresholdEvent.TryCreate(
                hasError: true, tick: 100, speedMps: 50.0, positionErrorM: 0.01, orientationErrorDeg: 0.01,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg), Is.Null);
        }

        [Test]
        public void TryCreate_HasErrorTrue_ErrorsExactlyAtThreshold_ReturnsNull()
        {
            // Strictly-greater-than, not >=: sitting exactly on the boundary is still "smooth"
            // per RenderOffset.ClassifyPosition/ClassifyOrientation's own band edges.
            Assert.That(ReconcileErrorThresholdEvent.TryCreate(
                hasError: true, tick: 100, speedMps: 50.0, positionErrorM: PositionThresholdM, orientationErrorDeg: OrientationThresholdDeg,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg), Is.Null);
        }

        // ------------------------------------------------------------------ the positive half

        [Test]
        public void TryCreate_PositionErrorOverThreshold_ReturnsAnEvent()
        {
            // qa r10's own unlocated max: position_error_m = 0.3151, over 0.25.
            ReconcileErrorThresholdEvent? evt = ReconcileErrorThresholdEvent.TryCreate(
                hasError: true, tick: 607724, speedMps: 140.0, positionErrorM: 0.3151, orientationErrorDeg: 0.01,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg);

            Assert.That(evt, Is.Not.Null);
            Assert.That(evt.Value.Tick, Is.EqualTo(607724L));
            Assert.That(evt.Value.PositionErrorM, Is.EqualTo(0.3151));
        }

        [Test]
        public void TryCreate_OrientationErrorOverThreshold_ReturnsAnEvent_EvenWithPositionUnderThreshold()
        {
            // qa r10's second unlocated max: orientation_error_deg = 3.2981, over 1.0 - the two
            // thresholds are independent, either one alone must be able to trigger a line.
            ReconcileErrorThresholdEvent? evt = ReconcileErrorThresholdEvent.TryCreate(
                hasError: true, tick: 5, speedMps: 10.0, positionErrorM: 0.01, orientationErrorDeg: 3.2981,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg);

            Assert.That(evt, Is.Not.Null);
            Assert.That(evt.Value.OrientationErrorDeg, Is.EqualTo(3.2981));
        }

        // ------------------------------------------------------------------ Format()

        [Test]
        public void Format_ContainsEveryField()
        {
            ReconcileErrorThresholdEvent evt = ReconcileErrorThresholdEvent.TryCreate(
                hasError: true, tick: 607724, speedMps: 140.0, positionErrorM: 0.3151, orientationErrorDeg: 3.2981,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("reconcile_error_threshold_event"));
            Assert.That(line, Does.Contain("tick=607724"));
            Assert.That(line, Does.Contain("speed_mps=140.0"));
            Assert.That(line, Does.Contain("position_error_m=0.3151"));
            Assert.That(line, Does.Contain("orientation_error_deg=3.2981"));
            Assert.That(line, Does.Contain("position_smooth_threshold_m=0.25"));
            Assert.That(line, Does.Contain("orientation_smooth_threshold_deg=1.00"));
            Assert.That(line, Does.Not.Contain("\n"), "must be one grep-able line");
        }

        [Test]
        public void Format_PositionAndOrientationError_DoNotAlias()
        {
            ReconcileErrorThresholdEvent evt = ReconcileErrorThresholdEvent.TryCreate(
                hasError: true, tick: 1, speedMps: 1.0, positionErrorM: 0.9, orientationErrorDeg: 4.4,
                positionSmoothThresholdM: PositionThresholdM, orientationSmoothThresholdDeg: OrientationThresholdDeg).Value;
            string line = evt.Format();

            Assert.That(line, Does.Contain("position_error_m=0.9000"));
            Assert.That(line, Does.Contain("orientation_error_deg=4.4000"));
            Assert.That(line, Does.Not.Contain("position_error_m=4.4000"));
            Assert.That(line, Does.Not.Contain("orientation_error_deg=0.9000"));
        }
    }
}
