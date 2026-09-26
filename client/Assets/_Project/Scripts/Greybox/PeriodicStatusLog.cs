// H-13 (OnGUI 외 주기적 로그, 리더 메시지 2026-09-23 / 02_sprint_contract.md SC-89 방법 칸, client
// H-13). Two recording sessions in R4 lost their HUD entirely because the Unity Game View was
// backgrounded: OnGUI is never invoked by the Editor while its window lacks focus (client R8
// finding), so a HUD-only evidence trail is structurally incapable of covering the exact
// background window SC-89 (a)/(b) need evidence from.
//
// CORRECTION (R13, leader, after qa r5 §E): the paragraph that used to stand here claimed
// "Update() keeps running while the Editor is backgrounded", citing TickCatchUp and ADR-0012 -
// which qa correctly called circular (TickCatchUp's existence is not evidence that Update runs
// unfocused; it is evidence that SOMETHING made the accumulator jump). ProjectSettings.asset:90
// has runInBackground: 0, so the honest statement is the opposite: Update() very likely does NOT
// run while the Game View is unfocused.
//
// That setting is NOT being changed, deliberately - but not for the reason R13 first gave here.
// R13 argued "the catch-up burst only exists because Update stops, so flipping the flag would
// delete the phenomenon SC-89 judges". qa r6 showed that causality is backwards: what produces a
// burst is a large Time.unscaledDeltaTime, and runInBackground = true removes the PAUSE, not the
// burst - the contract itself names a 2-4 Hz foreground throttle as a good reproduction vector.
//
// The reason that holds: runInBackground is part of the system under test, and changing the
// system to make its instrumentation more convenient is how a measurement stops measuring
// anything.
//
// R14 also claimed "the contract already specifies a foreground vector, so nothing is gained".
// That was false and qa r7 caught it: the contract's 2-4 Hz throttle is a BACKGROUND vector; its
// foreground vectors are domain reload, heavy scene load and a forced GC loop. The claim is
// dropped rather than repaired, because it was never load-bearing - the reason above stands on
// its own. Note also that the contract gates those vectors behind first MEASURING whether the
// editor throttles or pauses; that measurement has not been run, and "runInBackground: 0
// therefore it pauses" is an inference, not the measurement.
//
// So this type's real job is narrower than "background logging", and it is still the job that
// was needed: it removes the evidence trail's dependence on OnGUI and on screen capture. Flying
// happens with the Game View focused (it must - input requires focus), which is exactly when
// Update() runs, and that is when these lines are written. What the ApplicationFocused field then
// buys is the ability to see, after the fact, where the focused run began and ended - the
// transition, not the gap. SC-89's background window is evidenced by OnSessionEnded's full
// counter dump plus the discontinuity between two focused runs, never by lines from inside it.
//
// This type is the pure "format the numbers into one grep-able key=value line" half of
// the fix: GreyboxSession.Update() calls it on a wall-clock interval (Time.unscaledTime, not
// Time.time - same reasoning as everywhere else in this file that reads real elapsed time across
// a hitch) and passes the result to Debug.Log. Split out so the formatting is testable without a
// MonoBehaviour, a scene, or Unity's Time class - same discipline as TickCatchUp and
// SnapshotRebaseBatch in this folder.
//
// What IS proven, and all that is claimed: this type never consults focus to decide whether to
// emit. It has no reference to UnityEngine.GUI, EditorWindow or Application.isFocused (it only
// RECORDS that value as a field, never branches on it), and GreyboxSession calls it from
// Update(), never from OnGUI. So wherever Update() runs, a line is written. Whether Update()
// runs unfocused is a separate question, answered above: probably not, and deliberately left
// that way.

using System.Globalization;

namespace Starfall.Greybox
{
    /// <summary>One periodic status line's worth of HUD-equivalent numbers, plus their
    /// key=value formatting. Field selection mirrors GreyboxSession.OnGUI()'s HUD (AC-14/SC-56/
    /// SC-89 metrics) - not a duplicate of every HUD line (prose lines like "BOUNDARY WARNING"
    /// or the raw input-axis dump are HUD-only), just the ones §8.4's background reshoot
    /// checklist and SC-89 (a)/(b)/(f) actually grep for.</summary>
    public readonly struct PeriodicStatusLog
    {
        public readonly long Tick;
        public readonly int? AckInputSeq;
        public readonly double SpeedMps;
        public readonly double OriginDistanceM;
        public readonly double PredictErrorM;
        public readonly double PredictErrorDeg;
        public readonly long ReconcileHardSnapTotal;

        /// <summary>F-27 (architect R10 판정 §1, 01_architect_decisions.md "## R10 판정"):
        /// running max of the render-position jump a reconcile has produced this session (+ N,
        /// the sample count - SC-56 (b)'s "max always with n" discipline), measured regardless
        /// of whether step 1 had a comparison to report - read alongside ReconcileHardSnapTotal
        /// above (hard_snap can stay 0 while this is 49-391 m).</summary>
        public readonly double ReconcileRebaseJumpMaxM;
        public readonly long ReconcileRebaseJumpN;

        /// <summary>SC-56 (c4) FAIL gate (must be 0): count of reconciles where the jump exceeded
        /// what elapsed time (behind_ticks x speed x dt) plus the existing hard-snap slack could
        /// account for (Flight.ReconcileRebaseJumpBudget.IsUnexplained). Never add this to
        /// ReconcileHardSnapTotal - the two answer different questions (architect R10 §1.1).</summary>
        public readonly long ReconcileUnexplainedJumpTotal;

        /// <summary>SC-56 (c4) reporting fields - NO == 0 gate (Editor domain reloads/GC/focus
        /// loss make being behind common regardless of product code), but reporting them is
        /// mandatory. Count of reconciles where the client was behind the snapshot's tick, the
        /// largest such gap, and the largest jump seen AT one of those behind-reconciles
        /// specifically.</summary>
        public readonly long ReconcileClientBehindTotal;
        public readonly long ReconcileClientBehindMaxTicks;
        public readonly double ReconcileClientBehindMaxJumpM;

        /// <summary>SC-56 (e) (architect R10 판정 §2.4, 후속 판정 §5.3): reconciliation
        /// smoothing's four required-together fields - reconciles classified Smooth/
        /// SmoothTracked, frames where the render offset was actually nonzero, the offset's
        /// largest magnitude (+ N), and frames where decay alone (not a fresh reconcile) shrank
        /// it. Any one of the four alone can pass vacuously (classified-but-nothing-moved,
        /// decayed-to-zero-instantly, offset-set-once-and-never-decayed) - all four together is
        /// the evidence requirement.</summary>
        public readonly long ReconcileSmoothedReconcileTotal;
        public readonly long ReconcileRenderOffsetNonZeroFrameTotal;
        public readonly double ReconcileRenderOffsetMaxM;
        public readonly long ReconcileRenderOffsetMaxN;
        public readonly long ReconcileRenderOffsetDecayFrameTotal;

        public readonly int? SendBurstMaxTicksDrainedPerUpdate;
        public readonly int? SendBurstMaxSendsPerFrame;
        public readonly long CatchupCarryForwardTicksTotal;
        public readonly long CatchupDormantTicksTotal;
        public readonly long CatchupTruncatedTotal;
        public readonly long ReconcileForcedAfterHitchTotal;

        // C-1 (R8 판정 D-3 / ADR-0012 section 6.4 point 6, Starfall.Flight.ReconcileTickDrift).
        // ReconcileTickDriftTotal counts snapshots where (Δack_input_seq != Δtick) - the
        // precondition Reconciliation.Reconcile used to assume and R8 판정 showed is false three
        // ways (server carry-forward, server supersede, client truncate). ReconcileTickDriftMax
        // is the largest |drift| seen this session. Normal value for both: 0. Non-zero here with
        // reconcile_hard_snap_total == 0 means the fix held under a condition that actually
        // occurred - zero-and-zero together, not hard_snap alone, is what closes SC-56 (architect
        // R8 판정 §A-4/§C: "이 카운터 없이는 ... 구분되지 않는다").
        public readonly long ReconcileTickDriftTotal;
        public readonly long ReconcileTickDriftMax;
        public readonly int VisibleShips;
        /// <summary>Recorded, never branched on - see this file's header. Lets a reader confirm
        /// after the fact that a run of these lines really did span an unfocused window.</summary>
        public readonly bool ApplicationFocused;

        // F-8 (05_qa_report_r5.md, round R5). Until R13 this struct carried none of the fields
        // SC-59 is actually judged on, which made "collect criterion 2/5-b from the periodic log
        // instead of reshooting" (leader's 11-sc59-observation.md §6.4) impossible - the log
        // would have run for an hour and answered nothing. The thirteen below are the ones a reader
        // needs to decide turn direction and auto-level WITHOUT the leader's eyes.
        //
        // Sourcing note (qa r7): these come from the same _input/_controller the HUD reads, not
        // from a re-derivation - with one divergence worth knowing when reading a log against a
        // recording. When _input is null the HUD prints "no input sampler" while this line prints
        // ordinary zeros and flight_assist=false. R14's comment claimed the two "can never
        // disagree"; that was too strong.
        //
        // AXIS CONVENTION (ADR-0009 §1, mirrored by Sim/ShipControlInputD.cs:17):
        //   +X = ship-right, +Y = ship-up, +Z = ship-forward (bow).
        // So thrust=(0,0,1000) is full forward, (0,1000,0) is full up, (1000,0,0) is full right.
        // Roll is the scalar rate about ship-local +Z (Sim/ShipSimState.cs:19). Positive roll maps
        // ship +X onto ship +Y - the ship's right wing RISES.
        //
        // Do NOT reach for the right-hand rule to re-derive that. ADR-0009 §1 fixes a LEFT-handed
        // frame ("오른손 좌표계를 쓰지 않는다", stated in bold) and defines positive angular
        // velocity as Unity's positive rotation direction. Right-hand rule about +Z, sighted from
        // behind the ship, gives +X -> -Y and therefore the opposite answer. The mapping above is
        // what the algebra in ShipIntegrator.cs:147-149 actually produces; the named rule that
        // matches it is the left-hand one.
        //
        // This block has now been wrong twice. R13 wrote "the right wing drops" (qa r6). R14 fixed
        // the conclusion but kept "right-hand-rule about +Z" in the same sentence, so a reader
        // applying the named rule still arrived at the R13 answer (qa r7). Both times the failure
        // was the same: stating a convention without executing it. The conclusion above was
        // re-derived from the integrator, not from a rule's name.
        //
        // Attitude* is the ship's CURRENT orientation, AimTarget* is where the player is pointing
        // at. Both matter and neither substitutes for the other: the aim target alone cannot show
        // auto-level (the ship levelling out while the target stays put is exactly the case), and
        // the attitude alone cannot show that the player asked for the turn rather than drifting.
        // Reading the two as a pair over consecutive lines is what makes "did it turn RIGHT" and
        // "did it level off when the hands came off" third-party checkable rather than a claim.
        public readonly long ThrustX;
        public readonly long ThrustY;
        public readonly long ThrustZ;
        public readonly long Roll;
        public readonly long AimTargetX;
        public readonly long AimTargetY;
        public readonly long AimTargetZ;
        public readonly long AimTargetW;
        public readonly long AttitudeX;
        public readonly long AttitudeY;
        public readonly long AttitudeZ;
        public readonly long AttitudeW;
        /// <summary>origin_distance_m has crossed the star system's soft boundary radius - the
        /// same condition that draws "BOUNDARY WARNING" on the HUD. A bool, not a re-derivation
        /// from OriginDistanceM, so a reader never has to know the radius to read the log.</summary>
        public readonly bool BoundarySoftCrossed;
        /// <summary>qa r6: SC-59 criterion 5-b (auto-level) is unreadable without this. A run of
        /// lines showing attitude converging to level proves nothing unless the reader can see
        /// assist was ON - and a run where it does NOT converge proves nothing unless they can see
        /// it was OFF. R13 shipped the attitude fields without it; the HUD had it and the log did
        /// not.
        ///
        /// Assist is NECESSARY for auto-level, not sufficient (R14 wrote it as if it were;
        /// qa r7). ShipIntegrator.cs:122 takes the manual branch when `input.Roll != 0.0 ||
        /// !input.FlightAssist`, so auto-level needs assist ON **and** roll input at zero; and
        /// even then :138 zeroes the correction inside AutoLevelDeadzoneSin, and :132 zeroes it
        /// when the nose is parallel to system up (no roll reference). A reader judging 5-b needs
        /// flight_assist AND roll together, both of which this line carries.</summary>
        public readonly bool FlightAssist;

        // R16 (real-server session, 2026-09-24). The R5 session produced the first non-vacuous
        // SC-56/SC-89 numbers, and in doing so showed exactly what the log still could not decide:
        //
        //   Criterion 1/3 (W = bow, R = up): the log carried origin_distance_m, a MAGNITUDE
        //   (Position.Length()). "distance rose while thrust_z=1000" is produced identically by a
        //   build with the forward axis negated. Position as a vector settles it - the reader
        //   projects the displacement onto the ship's forward axis, which attitude_* already gives.
        //
        //   Criterion 2 (mouse right = turn right): every rotational field was downstream of the
        //   mouse mapping under test. MouseDelta*Total are the raw device deltas from before it;
        //   Yaw/PitchDeg are the mapping's output. Sign disagreement between the pair IS the bug
        //   SC-59 hunts.
        //
        // Both are metres/pixels as doubles rather than quantised ints: they are diagnostics read
        // by a human or a script, never wire values, and rounding them would reintroduce the same
        // "can't tell small motion from none" ambiguity the scalar distance had.
        public readonly double PositionX;
        public readonly double PositionY;
        public readonly double PositionZ;
        public readonly double MouseDeltaXTotal;
        public readonly double MouseDeltaYTotal;
        public readonly double YawDeg;
        public readonly double PitchDeg;

        public PeriodicStatusLog(
            long tick,
            int? ackInputSeq,
            double speedMps,
            double originDistanceM,
            double predictErrorM,
            double predictErrorDeg,
            long reconcileHardSnapTotal,
            double reconcileRebaseJumpMaxM,
            long reconcileRebaseJumpN,
            long reconcileUnexplainedJumpTotal,
            long reconcileClientBehindTotal,
            long reconcileClientBehindMaxTicks,
            double reconcileClientBehindMaxJumpM,
            long reconcileSmoothedReconcileTotal,
            long reconcileRenderOffsetNonZeroFrameTotal,
            double reconcileRenderOffsetMaxM,
            long reconcileRenderOffsetMaxN,
            long reconcileRenderOffsetDecayFrameTotal,
            int? sendBurstMaxTicksDrainedPerUpdate,
            int? sendBurstMaxSendsPerFrame,
            long catchupCarryForwardTicksTotal,
            long catchupDormantTicksTotal,
            long catchupTruncatedTotal,
            long reconcileForcedAfterHitchTotal,
            long reconcileTickDriftTotal,
            long reconcileTickDriftMax,
            int visibleShips,
            bool applicationFocused,
            long thrustX,
            long thrustY,
            long thrustZ,
            long roll,
            long aimTargetX,
            long aimTargetY,
            long aimTargetZ,
            long aimTargetW,
            long attitudeX,
            long attitudeY,
            long attitudeZ,
            long attitudeW,
            bool boundarySoftCrossed,
            bool flightAssist,
            double positionX,
            double positionY,
            double positionZ,
            double mouseDeltaXTotal,
            double mouseDeltaYTotal,
            double yawDeg,
            double pitchDeg)
        {
            FlightAssist = flightAssist;
            PositionX = positionX;
            PositionY = positionY;
            PositionZ = positionZ;
            MouseDeltaXTotal = mouseDeltaXTotal;
            MouseDeltaYTotal = mouseDeltaYTotal;
            YawDeg = yawDeg;
            PitchDeg = pitchDeg;
            ThrustX = thrustX;
            ThrustY = thrustY;
            ThrustZ = thrustZ;
            Roll = roll;
            AimTargetX = aimTargetX;
            AimTargetY = aimTargetY;
            AimTargetZ = aimTargetZ;
            AimTargetW = aimTargetW;
            AttitudeX = attitudeX;
            AttitudeY = attitudeY;
            AttitudeZ = attitudeZ;
            AttitudeW = attitudeW;
            BoundarySoftCrossed = boundarySoftCrossed;

            Tick = tick;
            AckInputSeq = ackInputSeq;
            SpeedMps = speedMps;
            OriginDistanceM = originDistanceM;
            PredictErrorM = predictErrorM;
            PredictErrorDeg = predictErrorDeg;
            ReconcileHardSnapTotal = reconcileHardSnapTotal;
            ReconcileRebaseJumpMaxM = reconcileRebaseJumpMaxM;
            ReconcileRebaseJumpN = reconcileRebaseJumpN;
            ReconcileUnexplainedJumpTotal = reconcileUnexplainedJumpTotal;
            ReconcileClientBehindTotal = reconcileClientBehindTotal;
            ReconcileClientBehindMaxTicks = reconcileClientBehindMaxTicks;
            ReconcileClientBehindMaxJumpM = reconcileClientBehindMaxJumpM;
            ReconcileSmoothedReconcileTotal = reconcileSmoothedReconcileTotal;
            ReconcileRenderOffsetNonZeroFrameTotal = reconcileRenderOffsetNonZeroFrameTotal;
            ReconcileRenderOffsetMaxM = reconcileRenderOffsetMaxM;
            ReconcileRenderOffsetMaxN = reconcileRenderOffsetMaxN;
            ReconcileRenderOffsetDecayFrameTotal = reconcileRenderOffsetDecayFrameTotal;
            SendBurstMaxTicksDrainedPerUpdate = sendBurstMaxTicksDrainedPerUpdate;
            SendBurstMaxSendsPerFrame = sendBurstMaxSendsPerFrame;
            CatchupCarryForwardTicksTotal = catchupCarryForwardTicksTotal;
            CatchupDormantTicksTotal = catchupDormantTicksTotal;
            CatchupTruncatedTotal = catchupTruncatedTotal;
            ReconcileForcedAfterHitchTotal = reconcileForcedAfterHitchTotal;
            ReconcileTickDriftTotal = reconcileTickDriftTotal;
            ReconcileTickDriftMax = reconcileTickDriftMax;
            VisibleShips = visibleShips;
            ApplicationFocused = applicationFocused;
        }

        /// <summary>One line, all key=value, space-separated - grep/awk-friendly by
        /// construction (each field name is unique in the line, so `grep 'reconcile_hard_snap_total='`
        /// finds exactly one number per line with no ambiguity).</summary>
        public string Format()
        {
            return "starfall.greybox: periodic_status tick=" + Tick.ToString(CultureInfo.InvariantCulture) +
                   " ack_input_seq=" + (AckInputSeq.HasValue ? AckInputSeq.Value.ToString(CultureInfo.InvariantCulture) : "null") +
                   " speed_mps=" + SpeedMps.ToString("F1", CultureInfo.InvariantCulture) +
                   " origin_distance_m=" + OriginDistanceM.ToString("F1", CultureInfo.InvariantCulture) +
                   " predict_error_m=" + PredictErrorM.ToString("F4", CultureInfo.InvariantCulture) +
                   " predict_error_deg=" + PredictErrorDeg.ToString("F4", CultureInfo.InvariantCulture) +
                   " reconcile_hard_snap_total=" + ReconcileHardSnapTotal.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_rebase_jump_max_m=" + ReconcileRebaseJumpMaxM.ToString("F4", CultureInfo.InvariantCulture) +
                   " reconcile_rebase_jump_n=" + ReconcileRebaseJumpN.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_unexplained_jump_total=" + ReconcileUnexplainedJumpTotal.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_client_behind_total=" + ReconcileClientBehindTotal.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_client_behind_max_ticks=" + ReconcileClientBehindMaxTicks.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_client_behind_max_jump_m=" + ReconcileClientBehindMaxJumpM.ToString("F4", CultureInfo.InvariantCulture) +
                   " render_smooth_band_total=" + ReconcileSmoothedReconcileTotal.ToString(CultureInfo.InvariantCulture) +
                   " render_offset_nonzero_frames_total=" + ReconcileRenderOffsetNonZeroFrameTotal.ToString(CultureInfo.InvariantCulture) +
                   " render_offset_max_m=" + ReconcileRenderOffsetMaxM.ToString("F4", CultureInfo.InvariantCulture) +
                   " render_offset_max_n=" + ReconcileRenderOffsetMaxN.ToString(CultureInfo.InvariantCulture) +
                   " render_offset_decay_frames_total=" + ReconcileRenderOffsetDecayFrameTotal.ToString(CultureInfo.InvariantCulture) +
                   " send_burst_max_ticks_per_update=" + FormatCount(SendBurstMaxTicksDrainedPerUpdate) +
                   " send_burst_max_sends_per_frame=" + FormatCount(SendBurstMaxSendsPerFrame) +
                   " catchup_carry_forward_ticks_total=" + CatchupCarryForwardTicksTotal.ToString(CultureInfo.InvariantCulture) +
                   " catchup_dormant_ticks_total=" + CatchupDormantTicksTotal.ToString(CultureInfo.InvariantCulture) +
                   " catchup_truncated_total=" + CatchupTruncatedTotal.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_forced_after_hitch_total=" + ReconcileForcedAfterHitchTotal.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_tick_drift_total=" + ReconcileTickDriftTotal.ToString(CultureInfo.InvariantCulture) +
                   " reconcile_tick_drift_max=" + ReconcileTickDriftMax.ToString(CultureInfo.InvariantCulture) +
                   " visible_ships=" + VisibleShips.ToString(CultureInfo.InvariantCulture) +
                   " application_focused=" + (ApplicationFocused ? "true" : "false") +
                   // F-8: axis convention is +X right / +Y up / +Z forward (see the field block
                   // above). Printed as separate keys rather than a "(x,y,z)" tuple so a reader
                   // can grep one axis out of a session without parsing parentheses - the HUD's
                   // tuple form is what made the leader's R4 read ambiguous.
                   " thrust_x=" + ThrustX.ToString(CultureInfo.InvariantCulture) +
                   " thrust_y=" + ThrustY.ToString(CultureInfo.InvariantCulture) +
                   " thrust_z=" + ThrustZ.ToString(CultureInfo.InvariantCulture) +
                   " roll=" + Roll.ToString(CultureInfo.InvariantCulture) +
                   " aim_target_x=" + AimTargetX.ToString(CultureInfo.InvariantCulture) +
                   " aim_target_y=" + AimTargetY.ToString(CultureInfo.InvariantCulture) +
                   " aim_target_z=" + AimTargetZ.ToString(CultureInfo.InvariantCulture) +
                   " aim_target_w=" + AimTargetW.ToString(CultureInfo.InvariantCulture) +
                   " attitude_x=" + AttitudeX.ToString(CultureInfo.InvariantCulture) +
                   " attitude_y=" + AttitudeY.ToString(CultureInfo.InvariantCulture) +
                   " attitude_z=" + AttitudeZ.ToString(CultureInfo.InvariantCulture) +
                   " attitude_w=" + AttitudeW.ToString(CultureInfo.InvariantCulture) +
                   " boundary_soft_crossed=" + (BoundarySoftCrossed ? "true" : "false") +
                   " flight_assist=" + (FlightAssist ? "true" : "false") +
                   " position_x=" + PositionX.ToString("F1", CultureInfo.InvariantCulture) +
                   " position_y=" + PositionY.ToString("F1", CultureInfo.InvariantCulture) +
                   " position_z=" + PositionZ.ToString("F1", CultureInfo.InvariantCulture) +
                   " mouse_dx_total=" + MouseDeltaXTotal.ToString("F1", CultureInfo.InvariantCulture) +
                   " mouse_dy_total=" + MouseDeltaYTotal.ToString("F1", CultureInfo.InvariantCulture) +
                   " yaw_deg=" + YawDeg.ToString("F2", CultureInfo.InvariantCulture) +
                   " pitch_deg=" + PitchDeg.ToString("F2", CultureInfo.InvariantCulture);
        }

        /// <summary>Same "unmeasured, not measured-and-zero" discipline as
        /// GreyboxSession.FormatCount/FormatStat (§7a: a 0-sample counter must not print as a
        /// plain 0) - duplicated here rather than shared because that method is a private
        /// instance helper on a MonoBehaviour this type must stay free of, to remain testable
        /// without a scene.</summary>
        static string FormatCount(int? value) =>
            value.HasValue ? value.Value.ToString(CultureInfo.InvariantCulture) : "n/a";
    }
}
