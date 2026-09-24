// D-4 (R8 판정, 01_architect_decisions.md "R8 판정" §A-5/§C-2, ADR-0012 section 6.4 "검증").
//
// Why this file exists: 246 pre-existing tests never caught the seq/tick misalignment bug
// (R8 판정 §A-5) because every one of them builds its retained-input history BY HAND, and a
// hand-built history has input_seq and server tick in lockstep by construction - the exact
// precondition the real bug breaks. This harness is the first place in the suite where
// ack_input_seq and server tick are allowed to DISAGREE, via an explicit "arrival schedule"
// (architect: "명령 i는 서버 tick f(i)에 도착한다") - the axis that was missing everywhere else.
//
// It drives the REAL PredictionHistory.ApplyInput / Reconciliation.Reconcile code (via
// PredictedShipController, itself unmodified production code) - no reimplementation of the
// reconcile algorithm lives here (the discipline SC-89 (c) already enforces for TickCatchUp).
//
// Three vectors (architect's table, R8 판정 §A-5):
//   V-0  1:1 (ack and tick always agree)                    - control. Must stay green always.
//   V-1  one server carry-forward (ack lags tick by 1)       - GREEN (post-fix, D-1/D-2 landed).
//   V-2  one server supersede (ack leads tick by 1)          - GREEN (post-fix, D-1/D-2 landed).
//
// RED-before-GREEN history (§7a discipline - R8 판정 C-1's "confirm the counter is nonzero in
// the CURRENT code before the fix"): before Reconciliation.Reconcile was rekeyed to snapshot.tick
// (this file's git history / 03_client_impl.md "R17" §RED), V-1 measured a ~7.0 m position error
// (result.State drifted a full tick ahead of truth) and V-2 measured a ~7.0 m SPURIOUS
// result.PositionErrorM (a false-positive "error" report on an otherwise-correct rebase) - both
// against the seq-keyed (ack_input_seq) algorithm, both exactly v*dt at the pinned 140 m/s. Both
// assertions below are what those same measurements look like AFTER the fix.
//
// Each vector pins the ship at max speed (140 m/s) before the incident, per architect: "빨강과
// 초록의 거리를 1000배 이상으로" - at 140 m/s a 1-tick misalignment is exactly 7.0 m
// (140 * 0.05), 1400x the 5mm ignore threshold, so there was no ambiguity about whether the RED
// run actually exercised the bug (it measured exactly 7.0 m both times - see 03_client_impl.md).
//
// Honest limit (architect D-5, same shape as SC-89 (a)'s "no bots"): this harness cannot prove a
// real network produces exactly this schedule - only that IF it does, Reconciliation handles it
// correctly. What CAN be checked here, and is (§7a): that each vector's schedule really does
// produce the divergence it claims, via the same ReconcileTickDrift.Compute this file's C-1
// sibling uses.
//
// CORRECTION (team-lead R18, F-20, qa r9): an earlier version of this comment said that side is
// "closed only by reconcile_tick_drift_total == 0 in a live session". THE SIGN WAS BACKWARDS.
// Drift is not a defect - it is a fact about the network (server carry-forward/supersede,
// ADR-0012 section 6.4) that this fix is supposed to ABSORB, not eliminate. Requiring
// drift == 0 would let a session that never exercised the bug (e.g. one that stayed idle, or
// stayed below 100 m/s - the threshold's own visibility floor, R8 판정 관측 3) pass trivially -
// exactly the "quiet session as evidence" shape SC-56 has already failed on three times. qa r9's
// corrected condition (NOT YET a ratified contract clause - pending architect F-22, cite that
// item, not this comment, as the authority once it lands):
//   (c1) reconcile_hard_snap_total == 0
//   (c2) at least one APPLIED snapshot has drift != 0 AND speed_mps >= 100 at that instant
//   (c3) that same snapshot's position_error_m <= 0.25 m (ReconcileTickDriftEvent.cs, F-21,
//        gives the per-event fields (c2)/(c3) need - the periodic total alone cannot answer this)
// A wrong-signed comment like the one this replaces is not cosmetic: a third party (or a future
// round) can read it as license to judge SC-56 closed from a quiet session, which is the same
// shape as r8's F-17 finding - the second time a comment in this slice has stated a closing
// condition backwards.

using System;
using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Contracts.Generated;
using Starfall.Flight;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class TickAlignedReconciliationHarnessTests
    {
        const double Dt = 1.0 / 20.0;

        static ShipClassStats LoadScoutFixture() =>
            ShipClassStats.FromJson(ContractFixtures.RequireValid("SHIP_CLASS", "example-scout.json").ReadText());

        static readonly ShipIntegrator.Boundary WideOpenBoundary =
            new ShipIntegrator.Boundary(softRadiusM: 10_000.0, hardRadiusM: 12_000.0, pullMps2: 25.0);

        static ShipControlInputD ForwardThrust(uint seq) => new ShipControlInputD(
            seq, 0, 0, 1000, 0, 0, 0, 0, 1_000_000, false, true);

        static WorldSnapshotMessage.WorldSnapshotPayload.ShipState ToWireShipState(ShipSimState state) =>
            new WorldSnapshotMessage.WorldSnapshotPayload.ShipState
            {
                ShipId = Guid.Empty,
                ActorId = Guid.Empty,
                ShipClassId = "scout-s01",
                Presence = "ACTIVE",
                PositionXMm = Quantization.QuantizePosition(state.Position.X),
                PositionYMm = Quantization.QuantizePosition(state.Position.Y),
                PositionZMm = Quantization.QuantizePosition(state.Position.Z),
                VelocityXMmS = (int)Quantization.QuantizeVelocity(state.Velocity.X),
                VelocityYMmS = (int)Quantization.QuantizeVelocity(state.Velocity.Y),
                VelocityZMmS = (int)Quantization.QuantizeVelocity(state.Velocity.Z),
                OrientationXMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.X),
                OrientationYMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.Y),
                OrientationZMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.Z),
                OrientationWMicro = (int)Quantization.QuantizeQuaternionComponent(state.Orientation.W),
                AngularVelocityXMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityAim.X),
                AngularVelocityYMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityAim.Y),
                AngularVelocityZMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityAim.Z),
                AngularVelocityRollMdegS = (int)Quantization.QuantizeAngularVelocity(state.AngularVelocityRoll),
            };

        /// <summary>Ramps a fresh controller (and an identical ground-truth loop) up to (near)
        /// max speed under constant forward thrust, for <paramref name="ticks"/> local ticks -
        /// enough that a later 1-tick misalignment is measured in metres, not millimetres
        /// (architect: 1000x separation). 90 ticks = 4.5s at main_thrust_mps2=35 reaches the
        /// scout's 140 m/s cap (example-scout.json) with margin.</summary>
        static (PredictedShipController Controller, ShipSimState GroundTruth) RampToMaxSpeed(
            ShipClassStats ship, int ticks)
        {
            var controller = new PredictedShipController(ship, WideOpenBoundary, Dt, ShipSimState.Zero);
            ShipSimState groundTruth = ShipSimState.Zero;
            for (uint seq = 1; seq <= ticks; seq++)
            {
                ShipControlInputD input = ForwardThrust(seq);
                groundTruth = ShipIntegrator.Step(groundTruth, input, ship, WideOpenBoundary, Dt).State;
                controller.ApplyInput(input);
            }
            return (controller, groundTruth);
        }

        // ------------------------------------------------------------------ V-0: 1:1 (control)

        [Test]
        public void V0_OneToOneSchedule_ReconcileErrorStaysAtQuantisationNoise()
        {
            ShipClassStats ship = LoadScoutFixture();
            (PredictedShipController controller, ShipSimState groundTruth) = RampToMaxSpeed(ship, 90);

            // Self-check (architect: "이 벡터가 빨개지면 하네스가 틀린 것"): ack and tick agree
            // exactly, so drift must be 0 - if it is not, the vector itself is malformed, not the
            // code under test.
            long? drift = ReconcileTickDrift.Compute(prevAck: 89, prevTick: 89, ack: 90, tick: 90);
            Assert.That(drift, Is.EqualTo(0L), "V-0 must be a 1:1 schedule by construction");

            ShipSimState confirmed = ShipStateWire.ToSimState(ToWireShipState(groundTruth));
            Reconciliation.Result result = controller.Reconcile(confirmed, snapshotTick: 90, DefaultTuning());

            double errM = (result.State.Position - groundTruth.Position).Length();
            TestContext.WriteLine("V-0 (1:1): errM=" + errM);

            Assert.That(result.HasError, Is.True);
            Assert.That(errM, Is.LessThanOrEqualTo(0.005), "V-0 is the control vector - it must never show a hard-snap-scale error");
        }

        // ------------------------------------------------------------------ V-1: server carry-forward

        [Test]
        public void V1_ServerCarriedForwardOnce_AckLagsTickByOne_ReconcileStillConverges()
        {
            // Arrival schedule: local ticks 1..90 all arrive at their own server tick, EXCEPT
            // local tick 90's command, which the server never receives in this window - so the
            // server's tick 90 carries the last applied input forward (ADR-0011 section 6.1).
            // The client, meanwhile, has no way to know this and has already locally predicted
            // through its own local tick 90 (it always predicts one tick per real send -
            // ADR-0012 section 6). Snapshot for this tick reports: tick=90 (the server DID
            // tick), ack_input_seq=89 (unchanged - nothing new was applied at tick 90).
            ShipClassStats ship = LoadScoutFixture();
            (PredictedShipController controller, ShipSimState groundTruth) = RampToMaxSpeed(ship, 90);

            // §7a: confirm the schedule actually produces the divergence it claims to, via the
            // same pure function C-1 uses in production (ReconcileTickDrift.Compute) - this is
            // the same RED-CONFIRMATION assertion the pre-fix version of this test used
            // (01_architect_decisions.md "R8 판정" C-1: the drift counter must be shown nonzero
            // before the fix, and this is what made that measurable without a live server). The
            // schedule is unchanged by the fix - only Reconcile's response to it is.
            long? drift = ReconcileTickDrift.Compute(prevAck: 89, prevTick: 89, ack: 89, tick: 90);
            Assert.That(drift, Is.EqualTo(-1L), "V-1 must isolate exactly one carry-forward tick (drift = -1)");

            // Ground truth: the server DID physically integrate tick 90 (carry-forward re-applies
            // the SAME constant forward-thrust input, so the physics result is identical to a
            // normal 90th tick - carry-forward changes ack bookkeeping, not physics). The
            // snapshot's OWN tick field (D-1's alignment key) is 90, regardless of what
            // ack_input_seq says.
            ShipSimState confirmed = ShipStateWire.ToSimState(ToWireShipState(groundTruth));

            Reconciliation.Result result = controller.Reconcile(confirmed, snapshotTick: 90, DefaultTuning());

            double errM = (result.State.Position - groundTruth.Position).Length();
            double speedMps = groundTruth.Velocity.Length();
            double onceBrokenErrM = speedMps * Dt; // what the seq-keyed (pre-fix) code produced here - see git history
            TestContext.WriteLine("V-1 (carry-forward, POST-FIX/tick-keyed code): speed=" + speedMps +
                                  " m/s, errM=" + errM + ", pre-fix would have been ~" + onceBrokenErrM);

            // GREEN (post-fix, D-1/D-2): step 1 now finds history[ServerTick=90] - the client's
            // OWN prediction for tick 90, not a one-tick-stale entry - and step 3 has nothing
            // left to replay (nothing has ServerTick > 90), so the result IS confirmed, with no
            // over-replay. Error is quantisation noise only, ~1400x smaller than the pre-fix
            // ~7.0 m (architect's separation requirement).
            Assert.That(result.HasError, Is.True, "tick 90 has a retained entry - step 1 must find it");
            Assert.That(errM, Is.LessThanOrEqualTo(0.005),
                "GREEN CONFIRMATION (post-fix): tick-keyed Reconcile must converge to quantisation-noise-" +
                "level error even when ack lagged tick by one carried-forward tick. Measured error=" + errM +
                " m (pre-fix code measured ~" + onceBrokenErrM + " m here - see git history).");
        }

        // ------------------------------------------------------------------ V-2: server supersede

        [Test]
        public void V2_ServerSupersededOnce_AckLeadsTickByOne_ReconcileStillConverges()
        {
            // Arrival schedule: local ticks 1..90 arrive normally, EXCEPT the server's tick 89
            // receives BOTH local tick 89's and local tick 90's commands in the same tick
            // (network reordering/burst) - the server applies only the last one it receives
            // (ADR-0011 section 4) and reports ack_input_seq=90 after a tick that only physically
            // advanced its own clock to 89. Physics is unaffected (both commands are identical
            // constant forward thrust), so confirmed state after the server's tick 89 equals a
            // plain 89-tick integration; the snapshot's own tick field (D-1's alignment key) is
            // 89, not 90.
            ShipClassStats ship = LoadScoutFixture();
            (PredictedShipController controller, ShipSimState groundTruth90) = RampToMaxSpeed(ship, 90);

            // The server only physically ticked 89 times this window - recompute the 89-tick
            // ground truth separately (same constant input, so this is just one fewer step of
            // the identical loop RampToMaxSpeed already ran).
            ShipSimState groundTruth89 = ShipSimState.Zero;
            for (uint seq = 1; seq <= 89; seq++)
                groundTruth89 = ShipIntegrator.Step(groundTruth89, ForwardThrust(seq), ship, WideOpenBoundary, Dt).State;

            // §7a: same schedule-invariant self-check as V-1 (see its comment for why this
            // doubles as the R8 판정 C-1 pre-fix RED confirmation).
            long? drift = ReconcileTickDrift.Compute(prevAck: 88, prevTick: 88, ack: 90, tick: 89);
            Assert.That(drift, Is.EqualTo(1L), "V-2 must isolate exactly one supersede tick (drift = +1)");

            ShipSimState confirmed = ShipStateWire.ToSimState(ToWireShipState(groundTruth89));

            Reconciliation.Result result = controller.Reconcile(confirmed, snapshotTick: 89, DefaultTuning());

            // GREEN (post-fix, D-1/D-2): step 1 finds history[ServerTick=89] (the client's own
            // tick-89 prediction) against confirmed(groundTruth89) - both represent exactly 89
            // ticks of identical constant integration, so the reported error is quantisation
            // noise, not the pre-fix code's spurious ~7.0 m (see git history for that version -
            // it compared history[InputSeq=90] against a tick-89 truth and reported a fake
            // one-tick error even though nothing was actually wrong, R8 판정 §A-2 관측 3's exact
            // shape). Step 3 THEN replays ServerTick=90 (the one entry above 89) on top of
            // confirmed, landing exactly on groundTruth90 - the tick the client had already
            // locally predicted, correctly preserved rather than snapped backward.
            double speedMps = groundTruth89.Velocity.Length();
            double onceBrokenErrM = speedMps * Dt; // what the seq-keyed (pre-fix) code reported here
            double finalStateErrM = (result.State.Position - groundTruth90.Position).Length();
            TestContext.WriteLine("V-2 (supersede, POST-FIX/tick-keyed code): speed=" + speedMps +
                                  " m/s, result.PositionErrorM=" + result.PositionErrorM +
                                  ", finalStateErrM(vs tick90)=" + finalStateErrM +
                                  ", pre-fix result.PositionErrorM would have been ~" + onceBrokenErrM);

            Assert.That(result.HasError, Is.True, "tick 89 has a retained entry - step 1 must find it");
            Assert.That(result.PositionErrorM, Is.LessThanOrEqualTo(0.005),
                "GREEN CONFIRMATION (post-fix): tick-keyed Reconcile must report only quantisation-noise-" +
                "level error, not the pre-fix code's spurious ~" + onceBrokenErrM + " m, when ack led tick " +
                "by one superseded tick. Measured result.PositionErrorM=" + result.PositionErrorM + " m.");
            Assert.That(finalStateErrM, Is.LessThanOrEqualTo(0.005),
                "the rebased-and-replayed state must land on the tick the client had already locally " +
                "predicted (tick 90), not snap backward to the tick-89 confirmed state alone.");
        }

        static SyncTuningData DefaultTuning() => new SyncTuningData
        {
            ReconcileIgnoreThresholdM = 0.005,
            ReconcileSmoothThresholdM = 0.25,
            ReconcileHardSnapThresholdM = 5.0,
            ReconcileOrientationIgnoreThresholdDeg = 0.02,
            ReconcileOrientationSmoothThresholdDeg = 1.0,
            ReconcileOrientationHardSnapDeg = 15.0,
        };
    }
}
