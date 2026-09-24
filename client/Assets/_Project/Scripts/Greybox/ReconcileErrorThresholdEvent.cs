// F-29 (qa r10 §H, team-lead R21). ReconcileTickDriftEvent only logs a line when drift != 0 -
// qa r10 found that the R8 session's position_error_m max (0.3151 m) landed on a snapshot with
// drift == 0, so nothing in the log named which reconcile it was ("출처 특정 불가능" - source
// cannot be identified). p50/p99/max (SC-56(b)) say a big error happened SOMEWHERE in the
// session; they cannot say when.
//
// This is the fix: one line whenever a reconcile actually MEASURED an error (HasError == true -
// there is no error to log otherwise, same "no error to report, not zero error" discipline as
// everywhere else in this file's neighbours) and that error exceeds the threshold where
// RenderOffset stops calling it ignorable - reusing the EXISTING designer constants
// (reconcile_smooth_threshold_m = 0.25, reconcile_orientation_smooth_threshold_deg = 1.0) rather
// than inventing a new tunable that would itself need justifying (team-lead R21 instruction).
//
// Deliberately a SEPARATE type from ReconcileTickDriftEvent, not a merged/extended one: the two
// fire for different reasons (drift != 0 vs. error over threshold, independent conditions that
// can each be true without the other), and a distinct grep key
// (reconcile_error_threshold_event vs. reconcile_tick_drift_event) already answers "why did this
// line get written" without needing a reason field inside a shared struct - a reader does not
// have to inspect the payload to know which condition fired, only the tag on the line.
//
// §7b(1) discipline: the gate (HasError, then the threshold compare) lives in TryCreate, not at
// the Debug.Log call site - "always emit, gate with an if() outside" would make the negative half
// untestable without a live scene (ReconcileErrorThresholdEventTests.cs's negative-contrast
// cases: HasError false, and HasError true but under both thresholds).

using System.Globalization;

namespace Starfall.Greybox
{
    public readonly struct ReconcileErrorThresholdEvent
    {
        public readonly long Tick;
        public readonly double SpeedMps;
        public readonly double PositionErrorM;
        public readonly double OrientationErrorDeg;

        /// <summary>The threshold actually compared against, printed so a reader never has to go
        /// find sync-tuning.json to know what "over threshold" meant for this line - and so a
        /// change to the designer constant is visible in old logs instead of silently
        /// reinterpreting them.</summary>
        public readonly double PositionSmoothThresholdM;
        public readonly double OrientationSmoothThresholdDeg;

        ReconcileErrorThresholdEvent(long tick, double speedMps, double positionErrorM, double orientationErrorDeg,
            double positionSmoothThresholdM, double orientationSmoothThresholdDeg)
        {
            Tick = tick;
            SpeedMps = speedMps;
            PositionErrorM = positionErrorM;
            OrientationErrorDeg = orientationErrorDeg;
            PositionSmoothThresholdM = positionSmoothThresholdM;
            OrientationSmoothThresholdDeg = orientationSmoothThresholdDeg;
        }

        /// <summary>Null when <paramref name="hasError"/> is false (nothing was measured this
        /// reconcile - never log a threshold compare against a value that was not taken), OR when
        /// both errors are at-or-under their threshold (the normal case for almost every
        /// reconcile). Strictly-greater-than, not >=, so a value sitting exactly on the threshold
        /// reads as "still smooth", matching RenderOffset.ClassifyPosition/ClassifyOrientation's
        /// own band edges.</summary>
        public static ReconcileErrorThresholdEvent? TryCreate(
            bool hasError, long tick, double speedMps, double positionErrorM, double orientationErrorDeg,
            double positionSmoothThresholdM, double orientationSmoothThresholdDeg)
        {
            if (!hasError) return null;
            if (positionErrorM <= positionSmoothThresholdM && orientationErrorDeg <= orientationSmoothThresholdDeg) return null;
            return new ReconcileErrorThresholdEvent(tick, speedMps, positionErrorM, orientationErrorDeg,
                positionSmoothThresholdM, orientationSmoothThresholdDeg);
        }

        /// <summary>One key=value line, grep-able by `reconcile_error_threshold_event`. No module
        /// prefix baked in - each caller prepends its own "starfall.xxx:" tag.</summary>
        public string Format()
        {
            return "reconcile_error_threshold_event tick=" + Tick.ToString(CultureInfo.InvariantCulture) +
                   " speed_mps=" + SpeedMps.ToString("F1", CultureInfo.InvariantCulture) +
                   " position_error_m=" + PositionErrorM.ToString("F4", CultureInfo.InvariantCulture) +
                   " orientation_error_deg=" + OrientationErrorDeg.ToString("F4", CultureInfo.InvariantCulture) +
                   " position_smooth_threshold_m=" + PositionSmoothThresholdM.ToString("F2", CultureInfo.InvariantCulture) +
                   " orientation_smooth_threshold_deg=" + OrientationSmoothThresholdDeg.ToString("F2", CultureInfo.InvariantCulture);
        }
    }
}
