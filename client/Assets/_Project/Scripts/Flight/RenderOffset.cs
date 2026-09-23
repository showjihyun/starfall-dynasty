// Hand-written. ADR-0012 section 4's band classification: reconciliation itself is always
// immediate (Reconciliation.cs runs every snapshot, no threshold gates it). What the
// thresholds decide is how the CORRECTION SHOWS ON SCREEN - and that decision is exactly what
// this file is. The actual per-frame decay curve is a presentation-layer concern (Greybox,
// C6): ADR-0012 section 4 explicitly allows exp() there because it is outside the determinism
// boundary (ADR-0010 section 3 only restricts state advancement).

using Starfall.Sim;

namespace Starfall.Flight
{
    public enum ReconcileBand
    {
        /// <summary>Error at or below the wire's own quantisation noise floor
        /// (sqrt(3)*0.5 mm ~= 0.87 mm for position). No offset is introduced - a perfectly
        /// correct client sees this on every snapshot (ADR-0009 section 2, ADR-0012 section 4:
        /// the ignore band architect added specifically so this is not treated as a correction).</summary>
        Ignore,

        /// <summary>Visible but small: smoothed over reconcile_smooth_duration_ms, not counted
        /// toward the correction histogram (designer's table splits "smoothed" into two rows
        /// only for telemetry granularity - the smoothing itself is identical).</summary>
        Smooth,

        /// <summary>Same smoothing as Smooth, but large enough that design section 6.4 wants it
        /// in the reconcile_correction_m histogram - worth watching even though it still
        /// resolves smoothly.</summary>
        SmoothTracked,

        /// <summary>At or beyond the hard-snap threshold: offset is zero (no smoothing at all -
        /// teleport), and the caller must log a warning and increment reconcile_hard_snap_total.
        /// On a lossless local link this must be 0 for the whole session (design section 6.4) -
        /// a non-zero count here is a bug signal, not expected behaviour.</summary>
        HardSnap,
    }

    public static class RenderOffset
    {
        public static ReconcileBand ClassifyPosition(double errorM, SyncTuningData tuning)
        {
            if (errorM <= tuning.ReconcileIgnoreThresholdM) return ReconcileBand.Ignore;
            if (errorM >= tuning.ReconcileHardSnapThresholdM) return ReconcileBand.HardSnap;
            return errorM <= tuning.ReconcileSmoothThresholdM ? ReconcileBand.Smooth : ReconcileBand.SmoothTracked;
        }

        public static ReconcileBand ClassifyOrientation(double errorDeg, SyncTuningData tuning)
        {
            if (errorDeg <= tuning.ReconcileOrientationIgnoreThresholdDeg) return ReconcileBand.Ignore;
            if (errorDeg >= tuning.ReconcileOrientationHardSnapDeg) return ReconcileBand.HardSnap;
            return errorDeg <= tuning.ReconcileOrientationSmoothThresholdDeg ? ReconcileBand.Smooth : ReconcileBand.SmoothTracked;
        }

        /// <summary>True for either smooth band - the caller does not need to branch on which
        /// one, only telemetry does.</summary>
        public static bool IsSmoothed(ReconcileBand band) => band == ReconcileBand.Smooth || band == ReconcileBand.SmoothTracked;
    }
}
