// F-33 (architect R10 판정 §2, team-lead R21). Tests for Starfall.Flight.RenderSmoothing - see
// that file's header for why it exists (ADR-0012 section 4's promised smoothing had no consuming
// code, qa r10 §G②) and why it must never be applied to a comparison-less (F-27) rebase jump.

using NUnit.Framework;
using Starfall.Flight;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RenderSmoothingTests
    {
        const double DurationMs = 200.0;

        // ------------------------------------------------------------------ ComputePositionOffset: negative half first (§7b(1))

        [Test]
        public void ComputePositionOffset_IgnoreBand_IsZero_EvenWithADifference()
        {
            Assert.That(RenderSmoothing.ComputePositionOffset(
                ReconcileBand.Ignore, new Vec3d(10, 0, 0), new Vec3d(0, 0, 0)), Is.EqualTo(Vec3d.Zero));
        }

        [Test]
        public void ComputePositionOffset_HardSnapBand_IsZero_EvenWithALargeDifference()
        {
            // ADR-0012 §4: a hard snap is never hidden - offset must be zero so the teleport is
            // visible, not smoothed away.
            Assert.That(RenderSmoothing.ComputePositionOffset(
                ReconcileBand.HardSnap, new Vec3d(400, 0, 0), new Vec3d(0, 0, 0)), Is.EqualTo(Vec3d.Zero));
        }

        // ------------------------------------------------------------------ ComputePositionOffset: positive half

        [Test]
        public void ComputePositionOffset_SmoothBand_IsTheDifference()
        {
            Vec3d offset = RenderSmoothing.ComputePositionOffset(
                ReconcileBand.Smooth, new Vec3d(10, 20, 30), new Vec3d(1, 2, 3));
            Assert.That(offset.X, Is.EqualTo(9.0).Within(1e-9));
            Assert.That(offset.Y, Is.EqualTo(18.0).Within(1e-9));
            Assert.That(offset.Z, Is.EqualTo(27.0).Within(1e-9));
        }

        [Test]
        public void ComputePositionOffset_SmoothTrackedBand_IsAlsoTheDifference()
        {
            // Same smoothing curve for both smooth bands (RenderOffset.cs's own doc comment: the
            // split exists only for telemetry granularity).
            Vec3d offset = RenderSmoothing.ComputePositionOffset(
                ReconcileBand.SmoothTracked, new Vec3d(5, 0, 0), new Vec3d(1, 0, 0));
            Assert.That(offset.X, Is.EqualTo(4.0).Within(1e-9));
        }

        // ------------------------------------------------------------------ ComputeOrientationOffset

        [Test]
        public void ComputeOrientationOffset_IgnoreBand_IsIdentity()
        {
            Assert.That(RenderSmoothing.ComputeOrientationOffset(
                ReconcileBand.Ignore, new Quatd(0, 0.707, 0, 0.707), Quatd.Identity), Is.EqualTo(Quatd.Identity));
        }

        [Test]
        public void ComputeOrientationOffset_HardSnapBand_IsIdentity()
        {
            Assert.That(RenderSmoothing.ComputeOrientationOffset(
                ReconcileBand.HardSnap, new Quatd(0, 0.707, 0, 0.707), Quatd.Identity), Is.EqualTo(Quatd.Identity));
        }

        [Test]
        public void ComputeOrientationOffset_SmoothBand_RecoversTheDisplayedOrientationWhenComposed()
        {
            // offset * simAfter must reconstruct renderBefore - the defining property callers
            // rely on when they draw offset * currentSimOrientation next frame.
            Quatd renderBefore = new Quatd(0, 0, 0.7071, 0.7071); // 90 deg about Z
            Quatd simAfter = Quatd.Identity;
            Quatd offset = RenderSmoothing.ComputeOrientationOffset(ReconcileBand.Smooth, renderBefore, simAfter);

            Quatd recomposed = offset * simAfter;
            Assert.That(recomposed.Z, Is.EqualTo(renderBefore.Z).Within(1e-6));
            Assert.That(recomposed.W, Is.EqualTo(renderBefore.W).Within(1e-6));
        }

        // ------------------------------------------------------------------ DecayPositionOffset

        [Test]
        public void DecayPositionOffset_ZeroOffset_StaysZero()
        {
            Assert.That(RenderSmoothing.DecayPositionOffset(Vec3d.Zero, dtSeconds: 0.05, durationMs: DurationMs), Is.EqualTo(Vec3d.Zero));
        }

        [Test]
        public void DecayPositionOffset_NonZeroOffset_ShrinksTowardZero_ButNeverReversesSign()
        {
            Vec3d offset = new Vec3d(10, 0, 0);
            Vec3d decayed = RenderSmoothing.DecayPositionOffset(offset, dtSeconds: 0.05, durationMs: DurationMs);

            Assert.That(decayed.X, Is.LessThan(10.0), "must shrink");
            Assert.That(decayed.X, Is.GreaterThan(0.0), "must not overshoot past zero or flip sign");
        }

        [Test]
        public void DecayPositionOffset_RepeatedApplication_ConvergesTowardZero()
        {
            Vec3d offset = new Vec3d(100, 0, 0);
            for (int i = 0; i < 200; i++)
                offset = RenderSmoothing.DecayPositionOffset(offset, dtSeconds: 1.0 / 60.0, durationMs: DurationMs);

            Assert.That(offset.X, Is.LessThan(0.5), "should have decayed to a small fraction of the original 100 m after >3 seconds");
        }

        [Test]
        public void DecayPositionOffset_ZeroDuration_SnapsToZeroImmediately_NotDivideByZero()
        {
            Vec3d decayed = RenderSmoothing.DecayPositionOffset(new Vec3d(10, 0, 0), dtSeconds: 0.05, durationMs: 0.0);
            Assert.That(decayed, Is.EqualTo(Vec3d.Zero));
        }

        // ------------------------------------------------------------------ DecayOrientationOffset

        [Test]
        public void DecayOrientationOffset_IdentityOffset_StaysIdentity()
        {
            Quatd decayed = RenderSmoothing.DecayOrientationOffset(Quatd.Identity, dtSeconds: 0.05, durationMs: DurationMs);
            Assert.That(decayed.W, Is.EqualTo(1.0).Within(1e-9));
        }

        [Test]
        public void DecayOrientationOffset_RepeatedApplication_ConvergesTowardIdentity()
        {
            Quatd offset = new Quatd(0, 0, 0.7071, 0.7071); // 90 deg about Z
            for (int i = 0; i < 200; i++)
                offset = RenderSmoothing.DecayOrientationOffset(offset, dtSeconds: 1.0 / 60.0, durationMs: DurationMs);

            double angleFromIdentityDeg = Quatd.AngleDegrees(offset, Quatd.Identity);
            Assert.That(angleFromIdentityDeg, Is.LessThan(1.0), "should have decayed to under 1 degree after >3 seconds");
        }

        [Test]
        public void DecayOrientationOffset_ZeroDuration_SnapsToIdentityImmediately()
        {
            Quatd decayed = RenderSmoothing.DecayOrientationOffset(new Quatd(0, 0, 0.7071, 0.7071), dtSeconds: 0.05, durationMs: 0.0);
            Assert.That(Quatd.AngleDegrees(decayed, Quatd.Identity), Is.LessThan(1e-6));
        }
    }
}
