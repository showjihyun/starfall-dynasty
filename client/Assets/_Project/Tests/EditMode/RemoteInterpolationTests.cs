// C5. Remote-ship interpolation (ADR-0012 section 5). The task doc is explicit about the
// trap: "t = 0.5" is the symmetric point where slerp and nlerp+normalize produce EXACTLY the
// same quaternion, so a broken implementation (lerp + normalize, no real slerp) passes a t=0.5
// test. These tests use t = 0.25 with a SYNTHESIZED >= 60 degree pair (real snapshot pairs
// never differ by more than ~7.5 degrees at max turn rate x 2 ticks, so a real fixture cannot
// exercise this - client R6 / task doc C5).

using System;
using NUnit.Framework;
using Starfall.Remote;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RemoteInterpolationTests
    {
        // 90 degrees about world Y: (0, sin45, 0, cos45). Comfortably over the 60 degree
        // requirement and far larger than any real two-tick snapshot pair (<= 7.5 degrees).
        static readonly Quatd Identity = Quatd.Identity;
        static readonly Quatd Rotated90Y = new Quatd(0.0, 0.70710678118654752, 0.0, 0.70710678118654752);

        [Test]
        public void Slerp_AtQuarterPoint_DiffersFromNormalizedLerp_ForLargeAngle()
        {
            double angleDeg = Quatd.AngleDegrees(Identity, Rotated90Y);
            TestContext.WriteLine("synthetic pair angle: " + angleDeg + " degrees (>= 60 required, real pairs are <= 7.5)");
            Assert.That(angleDeg, Is.GreaterThanOrEqualTo(60.0));

            const double t = 0.25; // NOT 0.5 - see file header.

            Quatd viaSlerp = RemoteInterpolation.Slerp(Identity, Rotated90Y, t);

            var lerped = new Quatd(
                Identity.X + (Rotated90Y.X - Identity.X) * t,
                Identity.Y + (Rotated90Y.Y - Identity.Y) * t,
                Identity.Z + (Rotated90Y.Z - Identity.Z) * t,
                Identity.W + (Rotated90Y.W - Identity.W) * t);
            Quatd viaNlerp = lerped.Normalized();

            double angleBetweenMethods = Quatd.AngleDegrees(viaSlerp, viaNlerp);
            TestContext.WriteLine("slerp(t=0.25)  = " + viaSlerp);
            TestContext.WriteLine("nlerp(t=0.25)  = " + viaNlerp);
            TestContext.WriteLine("angle between the two results: " + angleBetweenMethods + " degrees");

            Assert.That(angleBetweenMethods, Is.GreaterThan(0.5),
                "at t=0.25 with a >=60 degree pair, slerp and nlerp+normalize MUST visibly differ - " +
                "if they don't, this implementation is secretly doing nlerp (client R6's trap)");

            // Also confirm the true slerp result is exactly 1/4 of the way around the great
            // circle - the property that actually defines slerp.
            double angleFromStart = Quatd.AngleDegrees(Identity, viaSlerp);
            TestContext.WriteLine("angle(identity, slerp result) = " + angleFromStart + " (expected " + (angleDeg * t) + ")");
            Assert.That(angleFromStart, Is.EqualTo(angleDeg * t).Within(1e-6));
        }

        [Test]
        public void Slerp_AtEndpoints_ReturnsExactEndpoints()
        {
            Quatd at0 = RemoteInterpolation.Slerp(Identity, Rotated90Y, 0.0);
            Quatd at1 = RemoteInterpolation.Slerp(Identity, Rotated90Y, 1.0);

            Assert.That(Quatd.AngleDegrees(at0, Identity), Is.EqualTo(0.0).Within(1e-9));
            Assert.That(Quatd.AngleDegrees(at1, Rotated90Y), Is.EqualTo(0.0).Within(1e-9));
        }

        [Test]
        public void Slerp_NearlyParallelQuaternions_FallsBackToStableNlerp_NoNaN()
        {
            // Two nearly-identical orientations (as two real consecutive snapshots normally
            // are, max 3.75 deg/tick) - the general slerp formula divides by sin(theta0), which
            // is unstable near theta0=0. Must not produce NaN.
            var almostIdentity = new Quatd(0.0001, 0.0, 0.0, 1.0).Normalized();
            Quatd result = RemoteInterpolation.Slerp(Identity, almostIdentity, 0.5);

            TestContext.WriteLine("nearly-parallel slerp result: " + result);
            Assert.That(double.IsNaN(result.X) || double.IsNaN(result.Y) || double.IsNaN(result.Z) || double.IsNaN(result.W), Is.False);
            Assert.That(result.Length(), Is.EqualTo(1.0).Within(1e-9));
        }

        [Test]
        public void Interpolate_PositionIsLinear_OrientationIsSlerp()
        {
            var from = new ShipSimState(new Vec3d(0, 0, 0), Vec3d.Zero, Identity, Vec3d.Zero, 0);
            var to = new ShipSimState(new Vec3d(100, 0, 0), Vec3d.Zero, Rotated90Y, Vec3d.Zero, 0);

            ShipSimState mid = RemoteInterpolation.Interpolate(from, to, 0.25);

            Assert.That(mid.Position.X, Is.EqualTo(25.0).Within(1e-9), "position must be plain linear interpolation");
            Assert.That(Quatd.AngleDegrees(mid.Orientation, from.Orientation), Is.EqualTo(90.0 * 0.25).Within(1e-6));
        }

        [Test]
        public void Extrapolate_UsesLastKnownVelocity()
        {
            var state = new ShipSimState(new Vec3d(0, 0, 0), new Vec3d(0, 0, 140.0), Identity, Vec3d.Zero, 0);
            ShipSimState after1s = RemoteInterpolation.Extrapolate(state, 1.0);

            TestContext.WriteLine("extrapolated 1s at 140 m/s: " + after1s.Position);
            Assert.That(after1s.Position.Z, Is.EqualTo(140.0).Within(1e-9));
        }
    }
}
