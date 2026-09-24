// F-21 (team-lead R18, qa r9). C-1's reconcile_tick_drift_{total,max} (ReconcileTickDrift.cs,
// GreyboxSession.cs) proved the condition existed but could not answer WHEN - it is a 5-second
// periodic total, and qa r9 found the drift in R17's re-measurement all landed inside the
// session's first ~97 ticks (ship dormant, zero thrust) with zero drift events during the 140
// m/s window - a fact nobody could see without reading every raw log line by hand, because the
// only record was a running counter. That gap, not any code defect, is why SC-56 stayed open a
// fourth round (team-lead R18 assignment).
//
// This is the fix: one line per NON-ZERO drift, at the moment it happens, carrying the speed and
// reconcile error that were true at that instant - so the next real-server session answers "did
// drift and high speed ever coincide" by reading, not inferring.
//
// TryCreate is the pure decision (§7b(1) discipline): null for drift == null (unmeasurable, first
// snapshot) OR drift == 0 (the normal case) - only a genuine event produces a value. A Format()
// that always emitted, gated only by an `if` at the call site, would make "no line when drift is
// 0" untestable without a live scene; putting the gate IN the pure function makes it directly
// unit-testable (ReconcileTickDriftEventTests.cs's negative-contrast cases).
//
// R19 addition (team-lead, architect R9 residual model): architect re-derived the residual and
// found it is proportional to DeltaTick, and only accumulates when the server applied a
// DIFFERENT input at the drifted tick than the client predicted (abs(delta_accel) * dt^2 per
// tick - a single-axis flip over 2 ticks is 0.263 m, a full thrust reversal 0.525 m). Contract
// (c3) is position_error <= 0.25 m; a value in (0.25, 5.0) is judged 미검증 (architect only), not
// FAIL - but that judgement needs to see whether the input actually differed at that tick, which
// the log did not carry. DeltaTick and the quantised control axes active at the event now do.
//
// R21 addition (qa r10 §D.2 item 3, team-lead R21): position_error_m/orientation_error_deg print
// "n/a" whenever HasError is false - correctly ("not measured"), but qa r10 found that reading as
// "nothing was measured at all", when in fact PredictedShipController.LastRebaseJumpM always is
// (F-27 - a DIFFERENT quantity: the actual render-position jump, not the step-1 comparison).
// RebaseJumpM is carried on every line, HasError or not, so a reader never has to conclude "n/a"
// means "no evidence".
//
// R21 round 2 addition (architect R10 판정 §1.4 table row 4, 01_architect_decisions.md
// "## R10 판정"): behind_ticks (Flight.ReconcileRebaseJumpBudget.BehindTicks) rides alongside
// rebase_jump_m for the same reason - a reader looking at an n/a line needs to be able to tell
// "this jump is explained by elapsed time" from "it is not" without cross-referencing a separate
// log stream (Greybox.ReconcileClientBehindEvent covers the SAME condition as its own dedicated
// event, but only fires when behind_ticks > 0 - this field means the drift-triggered line never
// goes without it either).

using System.Globalization;

namespace Starfall.Greybox
{
    public readonly struct ReconcileTickDriftEvent
    {
        public readonly long Drift;

        /// <summary>R19 (architect R9 residual model): tick - prevTick for this same interval -
        /// the denominator the residual's "0.263 m over 2 ticks" shape is measured against.
        /// Distinct from Drift (= deltaAck - deltaTick): this is deltaTick alone.</summary>
        public readonly long DeltaTick;
        public readonly long Tick;
        public readonly double SpeedMps;

        /// <summary>Whether the SAME Reconcile call that produced this drift also had a
        /// step-1 error to report (Reconciliation.Result.HasError) - false is possible (e.g. the
        /// client has fallen far enough behind that no retained entry matches snapshotTick) and
        /// must not be printed as a measured zero (same discipline as GreyboxSession.FormatStat).</summary>
        public readonly bool HasError;
        public readonly double PositionErrorM;
        public readonly double OrientationErrorDeg;

        /// <summary>R19 (architect R9): the quantised control axes ACTIVE at this tick - same
        /// integers the wire carries (I-36), never a re-derivation. Lets a reader/architect see
        /// whether the server plausibly applied a DIFFERENT input at the drifted tick, which is
        /// what determines whether a (0.25, 5.0] m reading is 미검증 or a real problem.</summary>
        public readonly long ThrustX;
        public readonly long ThrustY;
        public readonly long ThrustZ;
        public readonly long Roll;

        /// <summary>R21 (qa r10 §D.2 item 3): PredictedShipController.LastRebaseJumpM - the
        /// actual render-position jump THIS reconcile produced, measured regardless of HasError
        /// (see ReconcileRebaseJumpEvent.cs's header). Always a real number, never "n/a" - unlike
        /// PositionErrorM/OrientationErrorDeg, this quantity is defined whether or not step 1 had
        /// anything to compare against.</summary>
        public readonly double RebaseJumpM;

        /// <summary>R21 round 2 (architect R10 판정 §1.4): Flight.ReconcileRebaseJumpBudget.
        /// BehindTicks for this same reconcile - how many ticks the client's own local tick
        /// index was behind the snapshot when this reconcile started. Zero is the normal,
        /// healthy value (client already caught up); always a real number, never "n/a".</summary>
        public readonly long BehindTicks;

        ReconcileTickDriftEvent(long drift, long deltaTick, long tick, double speedMps, bool hasError,
            double positionErrorM, double orientationErrorDeg, long thrustX, long thrustY, long thrustZ, long roll,
            double rebaseJumpM, long behindTicks)
        {
            Drift = drift;
            DeltaTick = deltaTick;
            Tick = tick;
            SpeedMps = speedMps;
            HasError = hasError;
            PositionErrorM = positionErrorM;
            OrientationErrorDeg = orientationErrorDeg;
            ThrustX = thrustX;
            ThrustY = thrustY;
            ThrustZ = thrustZ;
            Roll = roll;
            RebaseJumpM = rebaseJumpM;
            BehindTicks = behindTicks;
        }

        /// <summary>Null when there is nothing to report: <paramref name="drift"/> is null
        /// (ReconcileTickDrift.Compute had no baseline yet) or exactly 0 (the normal, 1:1 case -
        /// most reconciles). Non-null ONLY when drift != 0 - this is an event log, not a
        /// periodic one.</summary>
        public static ReconcileTickDriftEvent? TryCreate(
            long? drift, long deltaTick, long tick, double speedMps, bool hasError,
            double positionErrorM, double orientationErrorDeg, long thrustX, long thrustY, long thrustZ, long roll,
            double rebaseJumpM, long behindTicks)
        {
            if (!drift.HasValue || drift.Value == 0) return null;
            return new ReconcileTickDriftEvent(drift.Value, deltaTick, tick, speedMps, hasError,
                positionErrorM, orientationErrorDeg, thrustX, thrustY, thrustZ, roll, rebaseJumpM, behindTicks);
        }

        /// <summary>One key=value line, grep-able by `reconcile_tick_drift_event`. position_error_m
        /// / orientation_error_deg print "n/a" (never 0) when HasError is false - a reader must be
        /// able to tell "this reconcile had no error to report" apart from "the error was zero".
        /// No module prefix baked in (same convention as MarkerHudLine.Format()) - each caller
        /// (GreyboxSession, ObserverSession) prepends its own "starfall.xxx:" tag.</summary>
        public string Format()
        {
            return "reconcile_tick_drift_event drift=" + Drift.ToString(CultureInfo.InvariantCulture) +
                   " delta_tick=" + DeltaTick.ToString(CultureInfo.InvariantCulture) +
                   " tick=" + Tick.ToString(CultureInfo.InvariantCulture) +
                   " speed_mps=" + SpeedMps.ToString("F1", CultureInfo.InvariantCulture) +
                   " position_error_m=" + (HasError ? PositionErrorM.ToString("F4", CultureInfo.InvariantCulture) : "n/a") +
                   " orientation_error_deg=" + (HasError ? OrientationErrorDeg.ToString("F4", CultureInfo.InvariantCulture) : "n/a") +
                   " thrust_x=" + ThrustX.ToString(CultureInfo.InvariantCulture) +
                   " thrust_y=" + ThrustY.ToString(CultureInfo.InvariantCulture) +
                   " thrust_z=" + ThrustZ.ToString(CultureInfo.InvariantCulture) +
                   " roll=" + Roll.ToString(CultureInfo.InvariantCulture) +
                   " rebase_jump_m=" + RebaseJumpM.ToString("F4", CultureInfo.InvariantCulture) +
                   " behind_ticks=" + BehindTicks.ToString(CultureInfo.InvariantCulture);
        }
    }
}
