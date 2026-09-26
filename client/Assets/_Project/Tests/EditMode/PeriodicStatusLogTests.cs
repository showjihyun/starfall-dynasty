// H-13 (OnGUI 외 주기적 로그, 리더 메시지 2026-09-23). Pure formatting tests for
// Starfall.Greybox.PeriodicStatusLog - see that file's header for why the log this backs exists
// (OnGUI never runs while the Game View is unfocused) and why it must not depend on focus.

using System.Globalization;
using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class PeriodicStatusLogTests
    {
        static PeriodicStatusLog Sample(bool applicationFocused = false) => new PeriodicStatusLog(
            tick: 350281,
            ackInputSeq: 42,
            speedMps: 12.5,
            originDistanceM: 1000.25,
            predictErrorM: 0.0012,
            predictErrorDeg: 0.0034,
            reconcileHardSnapTotal: 2, reconcileRebaseJumpMaxM: 49.5, reconcileRebaseJumpN: 8, reconcileUnexplainedJumpTotal: 3, reconcileClientBehindTotal: 6, reconcileClientBehindMaxTicks: 9, reconcileClientBehindMaxJumpM: 60.25, reconcileSmoothedReconcileTotal: 4, reconcileRenderOffsetNonZeroFrameTotal: 15, reconcileRenderOffsetMaxM: 0.31, reconcileRenderOffsetMaxN: 20, reconcileRenderOffsetDecayFrameTotal: 7,
            sendBurstMaxTicksDrainedPerUpdate: 9,
            sendBurstMaxSendsPerFrame: 1,
            catchupCarryForwardTicksTotal: 8,
            catchupDormantTicksTotal: 3,
            catchupTruncatedTotal: 0,
            reconcileForcedAfterHitchTotal: 1, reconcileTickDriftTotal: 4, reconcileTickDriftMax: 7,
            visibleShips: 2,
            applicationFocused: applicationFocused,
            // F-8 fields. Every value distinct and non-zero so a Format() that drops one, or
            // swaps two axes, cannot still produce a passing line (§7a: a shared value like 0
            // would let thrust_x=thrust_y go unnoticed).
            thrustX: 111,
            thrustY: 222,
            thrustZ: 333,
            roll: 444,
            aimTargetX: 555,
            aimTargetY: 666,
            aimTargetZ: 777,
            aimTargetW: 888,
            attitudeX: 999,
            attitudeY: 1111,
            attitudeZ: 2222,
            attitudeW: 3333,
            // qa r7 F-15: these two MUST differ. With both true (and both false in the
            // opposite-case fixtures below) a Format() that read flight_assist off
            // BoundarySoftCrossed passed the whole suite - qa proved it by injection. The
            // twelve integer fields already avoid this by carrying distinct values; the
            // bools were the only pair left sharing one.
            boundarySoftCrossed: true,
            flightAssist: false,
            positionX: 1111.5, positionY: -2222.5, positionZ: 3333.5,
            mouseDeltaXTotal: 4444.5, mouseDeltaYTotal: -5555.5,
            yawDeg: 66.25, pitchDeg: -77.75);

        [Test]
        public void Format_IsOneLine()
        {
            string line = Sample().Format();
            Assert.That(line, Does.Not.Contain("\n"), "must be exactly one line - grep depends on it");
            Assert.That(line, Does.Not.Contain("\r"));
        }

        [Test]
        public void Format_ContainsKeyValueSubstringsForEveryField()
        {
            // §7a: a format test that only checks "the string is non-empty" would pass for a
            // formatter that silently drops half its fields. Each key=value pair is asserted
            // individually so a regression that drops or renames one field fails here, not in
            // an unrelated qa grep three rounds later.
            string line = Sample().Format();

            Assert.That(line, Does.Contain("tick=350281"));
            Assert.That(line, Does.Contain("ack_input_seq=42"));
            Assert.That(line, Does.Contain("speed_mps=12.5"));
            Assert.That(line, Does.Contain("origin_distance_m=1000.3").Or.Contain("origin_distance_m=1000.2"));
            Assert.That(line, Does.Contain("predict_error_m=0.0012"));
            Assert.That(line, Does.Contain("predict_error_deg=0.0034"));
            Assert.That(line, Does.Contain("reconcile_hard_snap_total=2"));
            Assert.That(line, Does.Contain("reconcile_rebase_jump_max_m=49.5000"));
            Assert.That(line, Does.Contain("reconcile_rebase_jump_n=8"));
            Assert.That(line, Does.Contain("reconcile_unexplained_jump_total=3"));
            Assert.That(line, Does.Contain("reconcile_client_behind_total=6"));
            Assert.That(line, Does.Contain("reconcile_client_behind_max_ticks=9"));
            Assert.That(line, Does.Contain("reconcile_client_behind_max_jump_m=60.2500"));
            Assert.That(line, Does.Contain("render_smooth_band_total=4"));
            Assert.That(line, Does.Contain("render_offset_nonzero_frames_total=15"));
            Assert.That(line, Does.Contain("render_offset_max_m=0.3100"));
            Assert.That(line, Does.Contain("render_offset_max_n=20"));
            Assert.That(line, Does.Contain("render_offset_decay_frames_total=7"));
            Assert.That(line, Does.Contain("send_burst_max_ticks_per_update=9"));
            Assert.That(line, Does.Contain("send_burst_max_sends_per_frame=1"));
            Assert.That(line, Does.Contain("catchup_carry_forward_ticks_total=8"));
            Assert.That(line, Does.Contain("catchup_dormant_ticks_total=3"));
            Assert.That(line, Does.Contain("catchup_truncated_total=0"));
            Assert.That(line, Does.Contain("reconcile_forced_after_hitch_total=1"));
            Assert.That(line, Does.Contain("reconcile_tick_drift_total=4"));
            Assert.That(line, Does.Contain("reconcile_tick_drift_max=7"));
            Assert.That(line, Does.Contain("visible_ships=2"));
        }

        [Test]
        public void Format_NullAckInputSeq_PrintsNullNotZero()
        {
            var line = new PeriodicStatusLog(
                tick: -1, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: false,
                positionX: 0, positionY: 0, positionZ: 0,
                mouseDeltaXTotal: 0, mouseDeltaYTotal: 0, yawDeg: 0, pitchDeg: 0).Format();

            Assert.That(line, Does.Contain("ack_input_seq=null"));
        }

        [Test]
        public void Format_UnmeasuredSendBurstCounters_PrintNotAvailableNotZero()
        {
            // §7a: a 0-sample counter (no Update() has drained/sent yet) must not read as a
            // measured 0 - same discipline as GreyboxSession.FormatCount/FormatStat.
            var line = new PeriodicStatusLog(
                tick: 0, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: false,
                positionX: 0, positionY: 0, positionZ: 0,
                mouseDeltaXTotal: 0, mouseDeltaYTotal: 0, yawDeg: 0, pitchDeg: 0).Format();

            Assert.That(line, Does.Contain("send_burst_max_ticks_per_update=n/a"));
            Assert.That(line, Does.Contain("send_burst_max_sends_per_frame=n/a"));
            // §7b self-check: this line is trivially satisfied if Format() ALWAYS printed
            // "n/a" for these two fields regardless of input - paired by
            // Format_ContainsKeyValueSubstringsForEveryField above, which asserts the SAME two
            // fields print real numbers (9, 1) when the counters are non-null.
        }

        [Test]
        public void Format_RecordsApplicationFocusedVerbatim_TrueAndFalse()
        {
            // H-13's "prove it does not depend on focus" requirement, at the formatting layer:
            // this type must be able to produce a line with application_focused=false (the
            // exact case the whole feature exists for) with no special-casing - it takes
            // whatever bool it is given and prints it, nothing more.
            Assert.That(Sample(applicationFocused: false).Format(), Does.Contain("application_focused=false"));
            Assert.That(Sample(applicationFocused: true).Format(), Does.Contain("application_focused=true"));
        }

        // ------------------------------------------------------------------ F-8 (qa r5): the fields SC-59 is judged on

        [Test]
        public void Format_AttitudeThrustRollAndBoundary_EachFieldAppearsWithItsOwnValue()
        {
            // qa r5 F-8: without these the periodic log could not answer SC-59 criterion 2
            // (turn direction) or 5-b (auto-level), which is what made the leader's "collect it
            // from the log next session instead of reshooting" plan impossible.
            //
            // Asserted field-by-field WITH its distinct value, not by counting keys: a Format()
            // that printed thrust_y's number under thrust_z's name would pass any key-presence
            // check, and an axis swap is precisely the bug class SC-59 exists to catch.
            string line = Sample().Format();

            Assert.That(line, Does.Contain("thrust_x=111"));
            Assert.That(line, Does.Contain("thrust_y=222"));
            Assert.That(line, Does.Contain("thrust_z=333"));
            Assert.That(line, Does.Contain("roll=444"));
            Assert.That(line, Does.Contain("aim_target_x=555"));
            Assert.That(line, Does.Contain("aim_target_y=666"));
            Assert.That(line, Does.Contain("aim_target_z=777"));
            Assert.That(line, Does.Contain("aim_target_w=888"));
            Assert.That(line, Does.Contain("attitude_x=999"));
            Assert.That(line, Does.Contain("attitude_y=1111"));
            Assert.That(line, Does.Contain("attitude_z=2222"));
            Assert.That(line, Does.Contain("attitude_w=3333"));
            Assert.That(line, Does.Contain("boundary_soft_crossed=true"));
        }

        [Test]
        public void Format_BoundarySoftCrossed_PrintsBothStates()
        {
            // §7b(1) pairing for the assertion above: boundary_soft_crossed=true would also be
            // produced by a Format() that hard-coded "true". This is the other half.
            var notCrossed = new PeriodicStatusLog(
                tick: 0, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: false,
                positionX: 0, positionY: 0, positionZ: 0,
                mouseDeltaXTotal: 0, mouseDeltaYTotal: 0, yawDeg: 0, pitchDeg: 0).Format();

            Assert.That(notCrossed, Does.Contain("boundary_soft_crossed=false"));
        }

        [Test]
        public void Format_NegativeThrustAndRoll_KeepTheirSign()
        {
            // The sign IS the payload here (SC-59 is a sign-bug detector). A formatter that
            // printed magnitudes would satisfy every key-presence test above and destroy the
            // one thing the field is for.
            var reversed = new PeriodicStatusLog(
                tick: 0, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: -111, thrustY: -222, thrustZ: -1000, roll: -1000,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: false,
                positionX: 0, positionY: 0, positionZ: 0,
                mouseDeltaXTotal: 0, mouseDeltaYTotal: 0, yawDeg: 0, pitchDeg: 0).Format();

            Assert.That(reversed, Does.Contain("thrust_z=-1000"), "a reversed-sign build must be distinguishable in the log");
            Assert.That(reversed, Does.Contain("roll=-1000"));
            Assert.That(reversed, Does.Contain("thrust_x=-111"));
            Assert.That(reversed, Does.Contain("thrust_y=-222"));
        }

        [Test]
        public void Format_FlightAssist_PrintsBothStates()
        {
            // qa r6: criterion 5-b (auto-level) cannot be judged from a run of attitude lines
            // unless the reader can see whether assist was on - ShipIntegrator.cs:122 only runs
            // auto-level when it is. Both states asserted (§7b(1)): a hard-coded "true" would
            // satisfy either one alone.
            // Sample() is assist=false / boundary=true, deliberately opposite (qa r7 F-15):
            // asserting them together here is what makes a swapped source fail.
            Assert.That(Sample().Format(), Does.Contain("flight_assist=false"));
            Assert.That(Sample().Format(), Does.Contain("boundary_soft_crossed=true"));

            var assistOn = new PeriodicStatusLog(
                tick: 0, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: true,
                positionX: 0, positionY: 0, positionZ: 0,
                mouseDeltaXTotal: 0, mouseDeltaYTotal: 0, yawDeg: 0, pitchDeg: 0).Format();

            Assert.That(assistOn, Does.Contain("flight_assist=true"));
            Assert.That(assistOn, Does.Contain("boundary_soft_crossed=false"));

            var assistOff = new PeriodicStatusLog(
                tick: 0, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: false,
                positionX: 0, positionY: 0, positionZ: 0,
                mouseDeltaXTotal: 0, mouseDeltaYTotal: 0, yawDeg: 0, pitchDeg: 0).Format();

            Assert.That(assistOff, Does.Contain("flight_assist=false"));
        }

        [Test]
        public void Format_PositionIsAVector_NotJustTheMagnitudeItAlreadyHad()
        {
            // R16: origin_distance_m is Position.Length(). A build with the forward axis negated
            // produces the identical magnitude, which is why the R5 real-server session could not
            // close SC-59 criteria 1 and 3 despite 54 clean log lines. Each component asserted
            // with its own distinct value so an axis swap cannot slip through.
            string line = Sample().Format();

            Assert.That(line, Does.Contain("position_x=1111.5"));
            Assert.That(line, Does.Contain("position_y=-2222.5"), "sign must survive - it is the whole point");
            Assert.That(line, Does.Contain("position_z=3333.5"));
        }

        [Test]
        public void Format_RawMouseDeltasAndTheAnglesTheyFeed_BothAppear()
        {
            // R16: the pair is the evidence. mouse_dx_total is sampled BEFORE the mouse->aim
            // mapping SC-59 tests; yaw_deg is its output. A reader compares their signs; a single
            // one of them proves nothing about the mapping.
            string line = Sample().Format();

            Assert.That(line, Does.Contain("mouse_dx_total=4444.5"));
            Assert.That(line, Does.Contain("mouse_dy_total=-5555.5"));
            Assert.That(line, Does.Contain("yaw_deg=66.25"));
            Assert.That(line, Does.Contain("pitch_deg=-77.75"));
        }

        [Test]
        public void Format_NegatedMouseDelta_IsVisibleInTheLog()
        {
            // §7b(1) pairing for the test above: the assertions there would all pass against a
            // Format() that printed magnitudes. This is the case that separates them, and it is
            // the exact defect SC-59 exists to catch.
            var mirrored = new PeriodicStatusLog(
                tick: 0, ackInputSeq: null, speedMps: 0, originDistanceM: 0,
                predictErrorM: 0, predictErrorDeg: 0, reconcileHardSnapTotal: 0, reconcileRebaseJumpMaxM: 0.0, reconcileRebaseJumpN: 0, reconcileUnexplainedJumpTotal: 0, reconcileClientBehindTotal: 0, reconcileClientBehindMaxTicks: 0, reconcileClientBehindMaxJumpM: 0.0, reconcileSmoothedReconcileTotal: 0, reconcileRenderOffsetNonZeroFrameTotal: 0, reconcileRenderOffsetMaxM: 0.0, reconcileRenderOffsetMaxN: 0, reconcileRenderOffsetDecayFrameTotal: 0,
                sendBurstMaxTicksDrainedPerUpdate: null, sendBurstMaxSendsPerFrame: null,
                catchupCarryForwardTicksTotal: 0, catchupDormantTicksTotal: 0,
                catchupTruncatedTotal: 0, reconcileForcedAfterHitchTotal: 0, reconcileTickDriftTotal: 0, reconcileTickDriftMax: 0,
                visibleShips: 0, applicationFocused: false,
                thrustX: 0, thrustY: 0, thrustZ: 0, roll: 0,
                aimTargetX: 0, aimTargetY: 0, aimTargetZ: 0, aimTargetW: 0,
                attitudeX: 0, attitudeY: 0, attitudeZ: 0, attitudeW: 0,
                boundarySoftCrossed: false, flightAssist: false,
                positionX: -1111.5, positionY: 2222.5, positionZ: -3333.5,
                mouseDeltaXTotal: -4444.5, mouseDeltaYTotal: 5555.5,
                yawDeg: -66.25, pitchDeg: 77.75).Format();

            Assert.That(mirrored, Does.Contain("mouse_dx_total=-4444.5"));
            Assert.That(mirrored, Does.Contain("yaw_deg=-66.25"));
            Assert.That(mirrored, Does.Contain("position_x=-1111.5"));
            Assert.That(mirrored, Does.Contain("position_z=-3333.5"));
        }
    }
}
