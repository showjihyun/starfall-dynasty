// F-33 (architect R10 판정 §2, 01_architect_decisions.md "## R10 판정" - team-lead R21 relay).
// ADR-0012 section 4 table row 2 promised reconciliation smoothing ("전체 오차를 오프셋으로,
// reconcile_smooth_duration_ms 에 걸쳐 수렴") and RenderOffset.cs's own header assigned the decay
// curve to "a presentation-layer concern (Greybox, C6)" - but qa r10 §G② found the consuming code
// never existed: RenderOffset.ClassifyPosition/Orientation's only caller was
// PredictedShipController's HardSnapTotal bookkeeping, and reconcile_smooth_duration_ms was
// parsed on both sides of the wire and consumed by neither. architect ruled this a defect, not an
// intentional scope cut, and this is the implementation.
//
// Pure, so EditMode can pin it (ADR-0010 section 3 restricts state ADVANCEMENT only - this never
// touches ShipSimState, it computes a RENDER-ONLY offset the caller adds on top when drawing).
// ADR-0012 section 4 explicitly allows exp()/trig here because the decay curve sits outside the
// determinism boundary.
//
// WHY THIS MUST NOT TRY TO SMOOTH F-27'S "CLIENT BEHIND" JUMPS (architect R10 §2.3): a jump the
// client did not draw because it was behind (SC-56 (c4)) is a real distance the ship travelled -
// smoothing it over reconcile_smooth_duration_ms (200 ms) would move the ship on screen FASTER
// than its own top speed (49 m / 200 ms = 245 m/s against a 140 m/s cap), which is not a
// correction, it is a second, fictional motion. That is why ComputePositionOffset/
// ComputeOrientationOffset below take the reconcile BAND, not the raw jump distance - a
// comparison-less reconcile is never classified into a Smooth/SmoothTracked band in the first
// place (Reconciliation.Result.HasError == false has no PositionErrorM to classify), so it is
// architecturally impossible for this code to smooth one. Ignore/HardSnap bands get offset zero
// for the same "never hide the correction" reason ADR-0012 section 4 states for hard snaps.

using System;
using Starfall.Sim;

namespace Starfall.Flight
{
    public static class RenderSmoothing
    {
        /// <summary>The offset this reconcile introduces - zero for Ignore/HardSnap (ADR-0012 §4:
        /// neither is ever hidden), otherwise (render position just before this correction) minus
        /// (simulated position just after it). Callers simply REPLACE their currently-decaying
        /// offset with this result - the "before" position already includes whatever offset was
        /// still visible from the previous correction, so this naturally captures the total
        /// remaining visual gap, not just this reconcile's own error.</summary>
        public static Vec3d ComputePositionOffset(ReconcileBand band, Vec3d renderPositionBeforeCorrection, Vec3d simPositionAfterCorrection)
        {
            if (!RenderOffset.IsSmoothed(band)) return Vec3d.Zero;
            return renderPositionBeforeCorrection - simPositionAfterCorrection;
        }

        /// <summary>Same shape as ComputePositionOffset, for orientation - the offset is the
        /// rotation still needed to go from the new simulated orientation to what was displayed
        /// a moment ago (displayed = offset * simulated, Hamilton product order matching Quatd's
        /// own convention).</summary>
        public static Quatd ComputeOrientationOffset(ReconcileBand band, Quatd renderOrientationBeforeCorrection, Quatd simOrientationAfterCorrection)
        {
            if (!RenderOffset.IsSmoothed(band)) return Quatd.Identity;
            return renderOrientationBeforeCorrection * simOrientationAfterCorrection.Conjugate();
        }

        /// <summary>Exponential decay toward zero with time constant durationMs
        /// (reconcile_smooth_duration_ms). durationMs &lt;= 0 decays instantly to zero (no
        /// smoothing time configured) rather than dividing by zero.</summary>
        public static Vec3d DecayPositionOffset(Vec3d offset, double dtSeconds, double durationMs) =>
            offset * DecayFactor(dtSeconds, durationMs);

        /// <summary>Same decay curve as DecayPositionOffset, applied as a slerp toward Quatd.Identity
        /// (the rotational equivalent of exponential decay - constant PROPORTION of the remaining
        /// angle closed per unit time, not a constant angular speed, so it decelerates into the
        /// target the same way the position offset decelerates into zero).</summary>
        public static Quatd DecayOrientationOffset(Quatd offset, double dtSeconds, double durationMs) =>
            Slerp(offset, Quatd.Identity, 1.0 - DecayFactor(dtSeconds, durationMs));

        static double DecayFactor(double dtSeconds, double durationMs)
        {
            if (durationMs <= 0.0) return 0.0;
            double tau = durationMs / 1000.0;
            return Math.Exp(-dtSeconds / tau);
        }

        /// <summary>Standard shortest-path slerp - deliberately NOT on Quatd itself (that type is
        /// the deterministic integrator core, ADR-0010 section 3; this is presentation-only and
        /// explicitly allowed to use exp()/trig, ADR-0012 section 4).</summary>
        static Quatd Slerp(Quatd a, Quatd b, double t)
        {
            double dot = a.X * b.X + a.Y * b.Y + a.Z * b.Z + a.W * b.W;
            if (dot < 0.0) { b = new Quatd(-b.X, -b.Y, -b.Z, -b.W); dot = -dot; } // shortest path (double cover)
            if (dot > 0.9995)
            {
                // Nearly identical - linear interpolate and renormalise rather than dividing by
                // a near-zero sin(theta0) below.
                Quatd lerped = new Quatd(
                    a.X + (b.X - a.X) * t, a.Y + (b.Y - a.Y) * t,
                    a.Z + (b.Z - a.Z) * t, a.W + (b.W - a.W) * t);
                return lerped.Normalized();
            }
            double theta0 = Math.Acos(dot);
            double theta = theta0 * t;
            double sinTheta0 = Math.Sin(theta0);
            double s1 = Math.Sin(theta) / sinTheta0;
            double s0 = Math.Cos(theta) - dot * s1;
            return new Quatd(a.X * s0 + b.X * s1, a.Y * s0 + b.Y * s1, a.Z * s0 + b.Z * s1, a.W * s0 + b.W * s1);
        }
    }
}
