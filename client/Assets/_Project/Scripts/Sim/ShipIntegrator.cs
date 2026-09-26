// Hand-written. ADR-0010 section 2's 12-step integrator, transcribed step for step. This is
// the law both languages must execute identically (ADR-0010 section 3): same operation order,
// no transcendental functions, no FMA, no signum. Comments below carry the step numbers from
// the ADR so a diff against the Rust side is a line-for-line comparison, not a redesign.
//
// Pure function: no UnityEngine, no I/O, no mutable static state, no clock. Same inputs ->
// same outputs, always (ADR-0012 section 3 "reconciliation is a pure function" needs this).

using System;

namespace Starfall.Sim
{
    public static class ShipIntegrator
    {
        /// <summary>AIM_MIN_NORM - a target quaternion whose norm is below this cannot be
        /// treated as a direction (ADR-0009 section 2).</summary>
        public const double AimMinNorm = 0.1;

        /// <summary>The zero-division guard used everywhere in step 9 and step 5 - "&gt; 0" is
        /// not enough for subnormal inputs (ADR-0010 section 2).</summary>
        public const double Eps = 1e-9;

        static readonly Vec3d WorldUp = new Vec3d(0.0, 1.0, 0.0);
        static readonly Vec3d LocalForward = new Vec3d(0.0, 0.0, 1.0);

        /// <summary>Everything the boundary steps (7, 12) need. Same numbers the server reads
        /// from the same data/world/systems/*.json row (StarSystemData).</summary>
        public readonly struct Boundary
        {
            public readonly double SoftRadiusM;
            public readonly double HardRadiusM;
            public readonly double PullMps2;

            public Boundary(double softRadiusM, double hardRadiusM, double pullMps2)
            {
                SoftRadiusM = softRadiusM;
                HardRadiusM = hardRadiusM;
                PullMps2 = pullMps2;
            }
        }

        public readonly struct Result
        {
            public readonly ShipSimState State;

            /// <summary>True when step 1 replaced a degenerate target quaternion with the
            /// ship's current attitude (I-39). Not fed back into state; a caller that wants
            /// aim_degenerate_total sums this across ticks itself.</summary>
            public readonly bool AimDegenerate;

            public Result(ShipSimState state, bool aimDegenerate)
            {
                State = state;
                AimDegenerate = aimDegenerate;
            }
        }

        /// <summary>One tick, one ship. Ship ordering (ship_id ascending, I-37) is the
        /// caller's concern - this function only ever sees one ship at a time.</summary>
        public static Result Step(ShipSimState state, ShipControlInputD input, ShipClassStats ship, Boundary boundary, double dt)
        {
            Vec3d p = state.Position;
            Vec3d v = state.Velocity;
            Quatd q = state.Orientation;
            Vec3d omegaAim = state.AngularVelocityAim;
            double omegaRoll = state.AngularVelocityRoll;

            // 1. target attitude normalisation
            double n = input.AimRaw.Length();
            bool aimDegenerate;
            Quatd qAim;
            if (n < AimMinNorm)
            {
                qAim = q;
                aimDegenerate = true;
            }
            else
            {
                qAim = input.AimRaw / n;
                aimDegenerate = false;
            }

            // 2. attitude error -> target angular velocity (no transcendental functions)
            Quatd qErr = qAim * q.Conjugate();
            if (qErr.W < 0.0) qErr = -qErr; // shortest path
            Vec3d e = qErr.Vector();
            double s = e.Length(); // s = sin(theta/2)

            Vec3d omegaT;
            if (s < ship.TurnDeadzoneSinHalf)
            {
                omegaT = Vec3d.Zero;
            }
            else
            {
                double magnitude = Math.Min(ship.TurnRateMaxDegS, ship.TurnGainDegSPerSinHalf * s);
                omegaT = (e / s) * magnitude;
            }

            // Roll-axis authority split (server B-3): the attitude controller never touches the
            // forward axis component of its own target. A no-op for a client that sends aim
            // without roll baked in (the intended usage); it protects the server from clients
            // that do not.
            Vec3d fPreRoll = q.Rotate(LocalForward);
            omegaT = omegaT - fPreRoll * Vec3d.Dot(omegaT, fPreRoll);

            // 3. angular velocity slew (acceleration limit)
            Vec3d delta = omegaT - omegaAim;
            double m = delta.Length();
            if (m > ship.TurnAccelDegS2 * dt)
                delta = delta * (ship.TurnAccelDegS2 * dt / m);
            omegaAim = omegaAim + delta;

            // 4. attitude integration (first order + renormalise)
            Vec3d w1 = omegaAim * (Math.PI / 180.0);
            q = q + (Quatd.Pure(w1) * q) * (0.5 * dt);
            q = q.Normalized();

            // 5. roll / auto-level (exactly one, result is one rotation about the forward axis)
            Vec3d fwd = q.Rotate(LocalForward);
            double omegaRollT;
            if (input.Roll != 0.0 || !input.FlightAssist)
            {
                omegaRollT = input.Roll * ship.RollRateMaxDegS;
            }
            else
            {
                Vec3d u = WorldUp - fwd * Vec3d.Dot(WorldUp, fwd);
                double nu = u.Length();
                if (nu < ship.AutoLevelDeadzoneSin)
                {
                    omegaRollT = 0.0; // nose parallel to system up: no roll reference
                }
                else
                {
                    Vec3d upS = q.Rotate(new Vec3d(0.0, 1.0, 0.0));
                    double sinErr = Vec3d.Dot(Vec3d.Cross(upS, u / nu), fwd); // [-1,1], signed - section 2.1
                    omegaRollT = Math.Abs(sinErr) < ship.AutoLevelDeadzoneSin ? 0.0 : ship.AutoLevelRateDegS * sinErr;
                }
            }

            double d = omegaRollT - omegaRoll;
            double lim = ship.RollAccelDegS2 * dt;
            if (Math.Abs(d) > lim) d = (d > 0.0 ? 1.0 : -1.0) * lim; // not signum(): see ADR-0010 section 3
            omegaRoll = omegaRoll + d;

            Vec3d w2 = fwd * (omegaRoll * (Math.PI / 180.0));
            q = q + (Quatd.Pure(w2) * q) * (0.5 * dt);
            q = q.Normalized();

            // 6. thrust -> world acceleration
            Vec3d aLocal = new Vec3d(
                input.Thrust.X * ship.LateralThrustMps2,
                input.Thrust.Y * ship.LateralThrustMps2,
                input.Thrust.Z >= 0.0 ? input.Thrust.Z * ship.MainThrustMps2 : input.Thrust.Z * ship.ReverseThrustMps2);
            if (input.Brake) aLocal = Vec3d.Zero; // brake ignores thrust
            double la = aLocal.Length();
            if (la > ship.MainThrustMps2) aLocal = aLocal * (ship.MainThrustMps2 / la); // diagonal clamp
            Vec3d aThrust = q.Rotate(aLocal); // rotated by the post-step-5 attitude

            // 7. boundary pull
            double r = p.Length();
            Vec3d aBound = r > boundary.SoftRadiusM ? -(p / r) * boundary.PullMps2 : Vec3d.Zero;

            // 8. velocity integration
            v = v + (aThrust + aBound) * dt;

            // 9. damping - exactly one, never overshoots zero. Computed from the v produced by
            // step 8, and never from a_bound - the pull is not pilot intent.
            if (input.Brake)
            {
                Vec3d u2 = v;
                double lim2 = ship.BrakeDecelMps2 * dt;
                double mag2 = u2.Length();
                if (mag2 > Eps) v = v - (u2 / mag2) * Math.Min(lim2, mag2);
            }
            else if (input.FlightAssist && aThrust.Length() > Eps)
            {
                Vec3d dHat = aThrust / aThrust.Length();
                Vec3d u2 = v - dHat * Vec3d.Dot(v, dHat);
                double lim2 = ship.AssistLateralDecelMps2 * dt;
                double mag2 = u2.Length();
                if (mag2 > Eps) v = v - (u2 / mag2) * Math.Min(lim2, mag2);
            }
            else if (input.FlightAssist)
            {
                Vec3d u2 = v;
                double lim2 = ship.AssistLinearDecelMps2 * dt;
                double mag2 = u2.Length();
                if (mag2 > Eps) v = v - (u2 / mag2) * Math.Min(lim2, mag2);
            }
            // else: no thrust, no brake, assist off - no damping at all (full Newtonian coast)

            // 10. speed cap - |v| only, no per-axis caps
            double sv = v.Length();
            if (sv > ship.MaxSpeedMps) v = v * (ship.MaxSpeedMps / sv);

            // 11. position integration
            p = p + v * dt;

            // 12. hard boundary
            double r2 = p.Length();
            if (r2 > boundary.HardRadiusM)
            {
                Vec3d nHat = p / r2;
                p = nHat * boundary.HardRadiusM;
                double vr = Vec3d.Dot(v, nHat);
                if (vr > 0.0) v = v - nHat * vr; // remove only the outward radial component
            }

            return new Result(new ShipSimState(p, v, q, omegaAim, omegaRoll), aimDegenerate);
        }
    }
}
