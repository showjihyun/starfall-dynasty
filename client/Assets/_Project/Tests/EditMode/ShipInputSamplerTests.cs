// F-19 (qa r8 §A2.3 / 08_qa_report_r8.md 부록 A2, "영검출 2건" #1). Starfall.Greybox.ShipInputSampler
// had ZERO EditMode coverage before this file - nunit found no fixture at all (qa r8's own
// search). A deliberate `halfYaw` sign flip in YawPitchToQuaternion (the exact shape SC-59
// criterion 2 hunts: mouse right maps to the ship turning the WRONG way) passed the entire
// 246-test suite unnoticed, because nothing called this pure function outside Sample()'s full
// device-reading pipeline.
//
// qa's prescribed shape, followed exactly: assert what yaw=+90 deg does to the WORLD forward
// vector via atan2(fwd.x, fwd.z), never by naming a hand rule. This project's own axis-convention
// comment (PeriodicStatusLog.cs) has been wrong twice from citing a rule by name instead of
// executing the algebra - qa r8 explicitly asked this test not repeat that.

using System;
using NUnit.Framework;
using Starfall.Greybox;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ShipInputSamplerTests
    {
        const double Deg2Rad = Math.PI / 180.0;

        /// <summary>World-frame azimuth of a forward vector, degrees, matching the convention
        /// this test asserts against: atan2(x, z), so straight-ahead (0,0,1) is 0 deg and a
        /// vector tilted toward world +X reads positive - no hand rule named or assumed.</summary>
        static double AzimuthDeg(Vec3d v) => Math.Atan2(v.X, v.Z) / Deg2Rad;

        [Test]
        public void YawPitchToQuaternion_YawPlus90Degrees_RotatesWorldForwardTowardWorldPlusX()
        {
            // The exact fixture qa r8 §A2.3 specified: "yaw = +90°이면 AimTargetWorld가 월드 +Z를
            // 월드 +X로 보낸다". This is SC-59 criterion 2's defect shape in its smallest possible
            // form - no ship, no server, no mouse device, just the pure yaw->quaternion mapping.
            Quatd q = ShipInputSampler.YawPitchToQuaternion(yawDeg: 90.0, pitchDeg: 0.0);

            Vec3d worldForward = new Vec3d(0.0, 0.0, 1.0);
            Vec3d rotated = q.Rotate(worldForward);

            double azimuthDeg = AzimuthDeg(rotated);
            TestContext.WriteLine("yaw=+90: rotated forward=" + rotated + " azimuth=" + azimuthDeg + " deg");

            Assert.That(azimuthDeg, Is.EqualTo(90.0).Within(0.01),
                "yaw=+90 degrees must rotate world +Z toward world +X (atan2(x,z) must read +90) - " +
                "a `halfYaw` sign flip here silently reverses which way the ship's nose turns for a " +
                "given yaw (SC-59 criterion 2's exact bug shape) and 246 other tests did not catch it " +
                "(qa r8 §A2.3, injection #5).");
        }

        [Test]
        public void YawPitchToQuaternion_YawMinus90Degrees_RotatesWorldForwardTowardWorldMinusX()
        {
            // §7b(1) pairing for the assertion above: a Format()-style bug that always reported
            // +90 regardless of sign would pass the first test alone. This is the other half.
            Quatd q = ShipInputSampler.YawPitchToQuaternion(yawDeg: -90.0, pitchDeg: 0.0);
            Vec3d rotated = q.Rotate(new Vec3d(0.0, 0.0, 1.0));

            double azimuthDeg = AzimuthDeg(rotated);
            TestContext.WriteLine("yaw=-90: rotated forward=" + rotated + " azimuth=" + azimuthDeg + " deg");

            Assert.That(azimuthDeg, Is.EqualTo(-90.0).Within(0.01),
                "yaw=-90 degrees must rotate world +Z toward world -X - the mirror case of the +90 " +
                "assertion above, so a formatter/mapper that ignores sign entirely cannot pass both.");
        }

        [Test]
        public void YawPitchToQuaternion_YawZero_LeavesWorldForwardUnchanged()
        {
            // Baseline: no yaw input means the aim target still points along world +Z exactly -
            // the reference point the two turning assertions above are measured relative to.
            Quatd q = ShipInputSampler.YawPitchToQuaternion(yawDeg: 0.0, pitchDeg: 0.0);
            Vec3d rotated = q.Rotate(new Vec3d(0.0, 0.0, 1.0));

            Assert.That(AzimuthDeg(rotated), Is.EqualTo(0.0).Within(0.01));
            Assert.That(rotated.X, Is.EqualTo(0.0).Within(1e-9));
            Assert.That(rotated.Z, Is.EqualTo(1.0).Within(1e-9));
        }
    }
}
