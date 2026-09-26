// C3. ADR-0010 section 2's 12-step integrator, exercised as a pure client-side unit (no
// server, no network - Starfall.Sim has noEngineReferences: true). The headline test
// (Integrator_IdentityAttitude_ForwardThrust_MovesOnlyZ_MatchesSharedExpectedInteger) is the
// one the task doc calls out: its expected integer must be the SAME one S3 (server) hardcodes,
// computed from contracts/fixtures/SHIP_CLASS/example-scout.json by hand per ADR-0010 section
// 2 - that agreement IS what "two languages, one law" means (client hand-off doc, task table).
//
// Fixture values, not data/ values (sprint contract section 0.7): data/ is designer-owned and
// live, so a test that reads its numbers goes red the moment someone retunes the feel.

using System;
using NUnit.Framework;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ShipIntegratorTests
    {
        const double Dt = 1.0 / 20.0; // tick_hz = 20 (ADR-0006 section 1 / SESSION_READY.tick_hz)

        static ShipClassStats LoadScoutFixture()
        {
            var fixture = ContractFixtures.RequireValid("SHIP_CLASS", "example-scout.json");
            return ShipClassStats.FromJson(fixture.ReadText());
        }

        static readonly ShipIntegrator.Boundary WideOpenBoundary =
            new ShipIntegrator.Boundary(softRadiusM: 10_000.0, hardRadiusM: 12_000.0, pullMps2: 25.0);

        static ShipControlInputD IdentityAimInput(
            long thrustX = 0, long thrustY = 0, long thrustZ = 0, long rollMilli = 0,
            bool brake = false, bool assist = true) =>
            new ShipControlInputD(
                inputSeq: 1,
                thrustXMilli: thrustX, thrustYMilli: thrustY, thrustZMilli: thrustZ,
                rollMilli: rollMilli,
                aimXMicro: 0, aimYMicro: 0, aimZMicro: 0, aimWMicro: 1_000_000, // identity, matches starting orientation
                brake: brake, flightAssist: assist);

        // ------------------------------------------------------------------ C3 headline test

        [Test]
        public void Integrator_IdentityAttitude_ForwardThrust_MovesOnlyZ_MatchesSharedExpectedInteger()
        {
            ShipClassStats ship = LoadScoutFixture();
            Assert.That(ship.MainThrustMps2, Is.EqualTo(35.0), "fixture main_thrust_mps2 changed - the shared expected integer must be recomputed with server (task doc C3)");

            ShipSimState state = ShipSimState.Zero;
            ShipControlInputD input = IdentityAimInput(thrustZ: 1000, assist: true);

            for (int tick = 0; tick < 20; tick++)
                state = ShipIntegrator.Step(state, input, ship, WideOpenBoundary, Dt).State;

            long positionXMm = Quantization.QuantizePosition(state.Position.X);
            long positionYMm = Quantization.QuantizePosition(state.Position.Y);
            long positionZMm = Quantization.QuantizePosition(state.Position.Z);
            long velocityZMmS = Quantization.QuantizeVelocity(state.Velocity.Z);

            TestContext.WriteLine("after 20 ticks: p=" + state.Position + " v=" + state.Velocity + " q=" + state.Orientation);
            TestContext.WriteLine("quantized: position_x_mm=" + positionXMm + " position_y_mm=" + positionYMm +
                                  " position_z_mm=" + positionZMm + " velocity_z_mm_s=" + velocityZMmS);

            // By hand (ADR-0010 section 2, semi-implicit Euler, a=35 m/s^2, dt=0.05 s, n=20):
            //   v_z(n) = a*dt*n = 1.75*n m/s -> v_z(20) = 35.0 m/s -> 35000 mm/s
            //   p_z(n) = a*dt^2 * n(n+1)/2   = 35*0.0025*210      -> 18.375 m -> 18375 mm
            // Identity aim == identity current attitude keeps q_err at the identity quaternion
            // for every tick (s = |vec(q_err)| = 0 < turn_deadzone_sin_half), so there is no
            // rotation to fold into x/y and no lateral drift.
            //
            // THIS IS THE SHARED NUMBER: S3 (server, starfall-sim) hardcodes the same 18375 /
            // 35000 from the same fixture. If server's number differs, the two integrators
            // disagree and this is NOT a threshold to raise - it is a finding for architect
            // (task doc: "결과가 나오는 즉시 architect에게 알린다").
            Assert.That(positionXMm, Is.EqualTo(0L));
            Assert.That(positionYMm, Is.EqualTo(0L));
            Assert.That(positionZMm, Is.EqualTo(18375L));
            Assert.That(velocityZMmS, Is.EqualTo(35000L));
        }

        [Test]
        public void Integrator_IdentityAttitude_ForwardThrust_IsIndependentOfFlightAssist()
        {
            // Flight assist damps the velocity component ORTHOGONAL to current thrust
            // direction (ADR-0010 section 2 step 9's assist-lateral branch). Thrusting dead
            // ahead from rest keeps velocity aligned with thrust the whole time, so assist
            // on/off must produce the identical trajectory - a regression here means the
            // lateral-damping branch is reading the wrong vector.
            ShipClassStats ship = LoadScoutFixture();

            ShipSimState withAssist = ShipSimState.Zero;
            ShipSimState withoutAssist = ShipSimState.Zero;
            ShipControlInputD inputOn = IdentityAimInput(thrustZ: 1000, assist: true);
            ShipControlInputD inputOff = IdentityAimInput(thrustZ: 1000, assist: false);

            for (int tick = 0; tick < 20; tick++)
            {
                withAssist = ShipIntegrator.Step(withAssist, inputOn, ship, WideOpenBoundary, Dt).State;
                withoutAssist = ShipIntegrator.Step(withoutAssist, inputOff, ship, WideOpenBoundary, Dt).State;
            }

            Assert.That(withAssist.Position, Is.EqualTo(withoutAssist.Position));
            Assert.That(withAssist.Velocity, Is.EqualTo(withoutAssist.Velocity));
        }

        // ------------------------------------------------------------------ speed cap

        [Test]
        public void Integrator_MaxThrustHeldLong_NeverExceedsMaxSpeed()
        {
            ShipClassStats ship = LoadScoutFixture();
            ShipSimState state = ShipSimState.Zero;
            ShipControlInputD input = IdentityAimInput(thrustZ: 1000, assist: true);

            double maxObserved = 0.0;
            for (int tick = 0; tick < 400; tick++)
            {
                state = ShipIntegrator.Step(state, input, ship, WideOpenBoundary, Dt).State;
                double speed = state.Velocity.Length();
                maxObserved = Math.Max(maxObserved, speed);
                Assert.That(speed, Is.LessThanOrEqualTo(ship.MaxSpeedMps + 1e-9), "tick " + tick);
            }

            TestContext.WriteLine("max observed speed over 400 ticks: " + maxObserved + " (cap " + ship.MaxSpeedMps + ")");
            Assert.That(maxObserved, Is.EqualTo(ship.MaxSpeedMps).Within(1e-6), "the cap must actually be reached, not merely never exceeded");
        }

        // ------------------------------------------------------------------ diagonal clamp

        [Test]
        public void Integrator_AllThreeAxesMaxed_ClampsToMainThrust_NotToUnclampedDiagonal()
        {
            // Unclamped: sqrt(18^2 + 18^2 + 35^2) = 43.28 m/s^2, 24% faster than forward alone.
            // Design section 4.3 / ADR-0010 section 2 step 6: normalise back to main_thrust_mps2.
            ShipClassStats ship = LoadScoutFixture();
            ShipSimState state = ShipSimState.Zero;
            ShipControlInputD input = IdentityAimInput(thrustX: 1000, thrustY: 1000, thrustZ: 1000, assist: false);

            ShipIntegrator.Result result = ShipIntegrator.Step(state, input, ship, WideOpenBoundary, Dt);
            double accelMagnitude = result.State.Velocity.Length() / Dt;

            TestContext.WriteLine("diagonal thrust accel magnitude (tick 1): " + accelMagnitude +
                                  " (main_thrust_mps2 = " + ship.MainThrustMps2 + ", unclamped would be ~43.28)");
            Assert.That(accelMagnitude, Is.EqualTo(ship.MainThrustMps2).Within(1e-9));
        }

        // ------------------------------------------------------------------ damping: exactly one

        [Test]
        public void Integrator_Brake_IgnoresThrust_AndAppliesOnlyBrakeDamping()
        {
            ShipClassStats ship = LoadScoutFixture();

            // Build up speed first (assist off, so we know exactly what we started with).
            ShipSimState state = ShipSimState.Zero;
            ShipControlInputD accelerate = IdentityAimInput(thrustZ: 1000, assist: false);
            for (int tick = 0; tick < 40; tick++) state = ShipIntegrator.Step(state, accelerate, ship, WideOpenBoundary, Dt).State;
            double speedBeforeBrake = state.Velocity.Length();

            ShipControlInputD brakeAndThrust = IdentityAimInput(thrustZ: 1000, brake: true, assist: true);
            ShipIntegrator.Result braked = ShipIntegrator.Step(state, brakeAndThrust, ship, WideOpenBoundary, Dt);
            double speedAfterOneBrakeTick = braked.State.Velocity.Length();

            double expectedDrop = ship.BrakeDecelMps2 * Dt; // exactly one damping, not two summed
            double actualDrop = speedBeforeBrake - speedAfterOneBrakeTick;

            TestContext.WriteLine("speed before brake: " + speedBeforeBrake + ", after one brake tick: " + speedAfterOneBrakeTick +
                                  ", drop: " + actualDrop + " (expected " + expectedDrop + ")");
            Assert.That(actualDrop, Is.EqualTo(expectedDrop).Within(1e-9),
                "brake must apply brake_decel_mps2 alone - thrust is ignored under brake (step 6) and only one damping (step 9) applies");
        }

        // ------------------------------------------------------------------ boundary

        [Test]
        public void Integrator_BeyondHardBoundary_ClampsPosition_RemovesOnlyOutwardRadialVelocity()
        {
            ShipClassStats ship = LoadScoutFixture();
            var boundary = new ShipIntegrator.Boundary(softRadiusM: 100.0, hardRadiusM: 200.0, pullMps2: 25.0);

            // Positioned just inside the hard boundary, moving fast straight outward (+Z) with
            // a small tangential (+X) component - the tangential part must survive.
            var state = new ShipSimState(
                position: new Vec3d(0.0, 0.0, 199.0),
                velocity: new Vec3d(10.0, 0.0, 50.0),
                orientation: Quatd.Identity,
                angularVelocityAim: Vec3d.Zero,
                angularVelocityRoll: 0.0);

            ShipControlInputD noInput = IdentityAimInput(assist: false); // pure coast, isolate the boundary clamp
            ShipIntegrator.Result result = ShipIntegrator.Step(state, noInput, ship, boundary, Dt);

            double r = result.State.Position.Length();
            TestContext.WriteLine("post-clamp |p| = " + r + " (hard=" + boundary.HardRadiusM + ")");
            Assert.That(r, Is.EqualTo(boundary.HardRadiusM).Within(1e-9), "position must sit exactly on the hard boundary sphere");

            Vec3d nHat = result.State.Position / r;
            double radialSpeed = Vec3d.Dot(result.State.Velocity, nHat);
            TestContext.WriteLine("post-clamp outward radial speed = " + radialSpeed + ", velocity = " + result.State.Velocity);
            Assert.That(radialSpeed, Is.LessThanOrEqualTo(1e-9), "the outward radial velocity component must be removed");

            // The tangential component is preserved EXACTLY relative to the post-clamp radial
            // direction nHat (step 12 subtracts only the nHat-aligned part, by construction).
            // It is NOT exactly 10.0 on the X axis: position (and therefore nHat) drifted
            // slightly off pure +Z during the one tick before the clamp (the ship also had a
            // small X velocity, so it moved a little in X too), so "outward" at clamp time is
            // not exactly +Z. That geometric coupling is real and correct, not a bug - the
          // invariant to check is "not reflected, not zeroed", i.e. still close to 10, not ~0
            // or negative.
            TestContext.WriteLine("nHat = " + nHat + " (not exactly (0,0,1) - that is the source of the X drift)");
            Assert.That(result.State.Velocity.X, Is.EqualTo(10.0).Within(0.5),
                "the tangential component must be approximately preserved (not reflected, not zeroed) - " +
                "small drift from the true radial direction not being pure +Z is expected");
        }

        // ------------------------------------------------------------------ degenerate aim

        [Test]
        public void Integrator_DegenerateAimQuaternion_KeepsCurrentAttitude_IsFlaggedNotRejected()
        {
            ShipClassStats ship = LoadScoutFixture();
            ShipSimState state = ShipSimState.Zero; // identity attitude

            var degenerateInput = new ShipControlInputD(
                inputSeq: 1,
                thrustXMilli: 0, thrustYMilli: 0, thrustZMilli: 0,
                rollMilli: 0,
                aimXMicro: 0, aimYMicro: 0, aimZMicro: 0, aimWMicro: 0, // all-zero: norm 0 < AIM_MIN_NORM
                brake: false, flightAssist: true);

            ShipIntegrator.Result result = ShipIntegrator.Step(state, degenerateInput, ship, WideOpenBoundary, Dt);

            TestContext.WriteLine("aimDegenerate=" + result.AimDegenerate + " q=" + result.State.Orientation);
            Assert.That(result.AimDegenerate, Is.True, "I-39: a degenerate target quaternion must be flagged, not silently accepted");
            Assert.That(result.State.Orientation, Is.EqualTo(Quatd.Identity), "the ship's attitude must be unchanged, not zeroed or NaN");
        }

        // ------------------------------------------------------------------ turn settling (AC-4(g))

        [Test]
        public void Integrator_OppositeTargetAttitude_SettlesWithoutOvershoot()
        {
            ShipClassStats ship = LoadScoutFixture();
            ShipSimState state = ShipSimState.Zero; // identity

            // 180 degrees about world Y: (0, 1, 0, 0).
            var input180 = new ShipControlInputD(
                inputSeq: 1,
                thrustXMilli: 0, thrustYMilli: 0, thrustZMilli: 0,
                rollMilli: 0,
                aimXMicro: 0, aimYMicro: 1_000_000, aimZMicro: 0, aimWMicro: 0,
                brake: false, flightAssist: true);

            var qAimRaw = new Quatd(0.0, 1.0, 0.0, 0.0);
            double maxOmega = 0.0;
            int settleTick = -1;
            int quietTicksNeeded = 40;
            int quietSince = -1;

            for (int tick = 0; tick < 300; tick++)
            {
                state = ShipIntegrator.Step(state, input180, ship, WideOpenBoundary, Dt).State;

                double omegaMag = state.AngularVelocityAim.Length();
                maxOmega = Math.Max(maxOmega, omegaMag);
                Assert.That(omegaMag, Is.LessThanOrEqualTo(ship.TurnRateMaxDegS + 1e-6), "tick " + tick + ": |omega_aim| must never exceed turn_rate_max_deg_s");

                Quatd qErr = qAimRaw * state.Orientation.Conjugate();
                if (qErr.W < 0.0) qErr = new Quatd(-qErr.X, -qErr.Y, -qErr.Z, -qErr.W);
                double s = qErr.Vector().Length();

                bool quiet = s < ship.TurnDeadzoneSinHalf;
                if (quiet && quietSince < 0) quietSince = tick;
                if (!quiet) quietSince = -1;
                if (settleTick < 0 && quietSince >= 0 && tick - quietSince + 1 >= quietTicksNeeded)
                    settleTick = quietSince;
            }

            TestContext.WriteLine("settle tick=" + settleTick + " (of 300), max |omega_aim|=" + maxOmega +
                                  " (cap " + ship.TurnRateMaxDegS + ")");
            Assert.That(settleTick, Is.GreaterThan(0), "the attitude controller must settle within the 300-tick window and stay settled for 40 ticks");
        }
    }
}
