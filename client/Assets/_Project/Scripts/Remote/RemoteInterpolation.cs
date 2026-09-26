// Hand-written. Position lerp + orientation slerp for remote (non-controlled) ships
// (ADR-0012 section 5). This is presentation, not simulation: it never has to match the server
// bit-for-bit (nothing here feeds back into anything the server checks), so unlike
// Starfall.Sim.ShipIntegrator it is free to use Math.Acos/Math.Sin - ADR-0010 section 3's
// transcendental-function ban is scoped to state advancement, not remote-ship rendering.

using System;
using Starfall.Sim;

namespace Starfall.Remote
{
    public static class RemoteInterpolation
    {
        /// <summary>Spherical linear interpolation between two orientations, shortest path.
        /// NOT the same as normalized-lerp except at t=0, t=1 and (by symmetry) t=0.5 - client
        /// R6: a test at t=0.5 cannot tell slerp from nlerp+normalize, which is exactly why
        /// C5's own tests use t=0.25 with a large synthetic angle instead.</summary>
        public static Quatd Slerp(Quatd a, Quatd b, double t)
        {
            double dot = a.X * b.X + a.Y * b.Y + a.Z * b.Z + a.W * b.W;

            Quatd bAdjusted = b;
            if (dot < 0.0)
            {
                // Shortest path: q and -q represent the same rotation: take the nearer one.
                bAdjusted = new Quatd(-b.X, -b.Y, -b.Z, -b.W);
                dot = -dot;
            }

            if (dot > 0.9995)
            {
                // Nearly parallel: sin(theta0) is too close to zero for the general formula to
                // stay numerically stable. Falling back to nlerp+normalize here is standard
                // practice and correct - the two methods provably converge as the angle -> 0.
                var lerp = new Quatd(
                    a.X + (bAdjusted.X - a.X) * t,
                    a.Y + (bAdjusted.Y - a.Y) * t,
                    a.Z + (bAdjusted.Z - a.Z) * t,
                    a.W + (bAdjusted.W - a.W) * t);
                return lerp.Normalized();
            }

            double theta0 = Math.Acos(dot);
            double theta = theta0 * t;
            double sinTheta0 = Math.Sin(theta0);
            double sinTheta = Math.Sin(theta);

            double s0 = Math.Cos(theta) - dot * sinTheta / sinTheta0;
            double s1 = sinTheta / sinTheta0;

            var result = new Quatd(
                s0 * a.X + s1 * bAdjusted.X,
                s0 * a.Y + s1 * bAdjusted.Y,
                s0 * a.Z + s1 * bAdjusted.Z,
                s0 * a.W + s1 * bAdjusted.W);
            return result.Normalized();
        }

        /// <summary>Position linear, orientation slerp, between two full ship states. Angular
        /// velocities are carried through by lerp too - display-only, not re-integrated.</summary>
        public static ShipSimState Interpolate(ShipSimState from, ShipSimState to, double t)
        {
            Vec3d position = from.Position + (to.Position - from.Position) * t;
            Vec3d velocity = from.Velocity + (to.Velocity - from.Velocity) * t;
            Quatd orientation = Slerp(from.Orientation, to.Orientation, t);
            Vec3d angularVelocityAim = from.AngularVelocityAim + (to.AngularVelocityAim - from.AngularVelocityAim) * t;
            double angularVelocityRoll = from.AngularVelocityRoll + (to.AngularVelocityRoll - from.AngularVelocityRoll) * t;

            return new ShipSimState(position, velocity, orientation, angularVelocityAim, angularVelocityRoll);
        }

        /// <summary>Extrapolates forward from the last known sample using its last known
        /// velocity/angular velocity - a one-shot first-order step over
        /// <paramref name="elapsedSeconds"/>, not tick-stepped (a remote ship is never
        /// simulated tick by tick; this approximates "coast at the last known rate" for the
        /// short window ADR-0012 section 5 allows before freezing).</summary>
        public static ShipSimState Extrapolate(ShipSimState from, double elapsedSeconds)
        {
            Vec3d position = from.Position + from.Velocity * elapsedSeconds;

            Vec3d w1 = from.AngularVelocityAim * (Math.PI / 180.0);
            Quatd q = from.Orientation + (Quatd.Pure(w1) * from.Orientation) * (0.5 * elapsedSeconds);

            Vec3d fwd = from.Orientation.Rotate(new Vec3d(0.0, 0.0, 1.0));
            Vec3d w2 = fwd * (from.AngularVelocityRoll * (Math.PI / 180.0));
            q = q + (Quatd.Pure(w2) * q) * (0.5 * elapsedSeconds);
            q = q.Normalized();

            return new ShipSimState(position, from.Velocity, q, from.AngularVelocityAim, from.AngularVelocityRoll);
        }
    }
}
