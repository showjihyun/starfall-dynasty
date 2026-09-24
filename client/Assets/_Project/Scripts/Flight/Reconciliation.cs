// Hand-written. ADR-0012 section 3's algorithm, RE-KEYED by section 6.4 (R8 판정, 2026-09-24):
//
//   S = snapshot.tick                      # the server tick this snapshot confirms - NOT ack_input_seq
//   X = snapshot.ships[controlled_ship_id]     # dequantized confirmed state
//
//   1. error = |retained_predicted_state[tick == S].p - X.p|   # skipped if no entry has ServerTick == S
//   2. predicted_state = X                              # position, velocity, attitude, BOTH angular velocities
//   3. for entry in retained_history where entry.ServerTick > S (ascending): predicted_state = integrate(predicted_state, entry.Input)
//   4. drop retained entries with ServerTick <= S
//   5. render offset from the error magnitude (Flight.RenderOffset, not this file)
//
// WHY THE REKEY (R8 판정 §A-2 / ADR-0012 section 6.4): the original algorithm assumed
// "Δack_input_seq == Δtick" between consecutive snapshots - true only when exactly one command
// is acknowledged per server tick. The server does not guarantee that: tick advances on the wall
// clock (simulation.rs next_tick += 1, independent of commands), ack_input_seq advances only when
// a command is newly applied (session.rs last_applied_input_seq). Server carry-forward (a tick
// with zero new arrivals), server supersede (a tick with two), and client truncate all break the
// equality - each makes step 1 compare against a state that is off by whole ticks, producing an
// error of exactly v*dt (140 m/s -> 7.0 m, well past reconcile_hard_snap_threshold_m=5.0).
// ack_input_seq is a network/scheduling quantity, not a simulation one (R8 판정 §A-2) - it
// cannot be an alignment key. InputRecord.ServerTick can, because it is stamped from the
// client's OWN tick index, which (like the server's) advances on the wall clock and is re-pinned
// to snapshot.tick on every reconciliation (PredictedShipController.CurrentTickIndex) - no clock
// sync, no new wire field (ADR-0012 section 6.4 point 3).
//
// ack_input_seq is NOT a parameter of this function any more - per ADR-0012 section 6.4 point 1
// its role from here on is reject/dedup/diagnostic only (its original ADR-0011 section 4 role),
// never reconciliation alignment. Callers that still need it for that (RebaseHold, HUD/log
// diagnostics, ReconcileTickDrift) read it from the snapshot directly, outside this function.
//
// Pure function (ADR-0012 section 3: "no clock, no frame time, no arrival time as input" -
// SC-53 checks exactly this by calling it twice on the same arguments). Step 2 rebases ALL
// FIVE state fields, not just position/orientation (ADR-0010 section 1.1 / B-1) - that is
// what SC-55 exists to catch.

using System.Collections.Generic;
using Starfall.Sim;

namespace Starfall.Flight
{
    public static class Reconciliation
    {
        public readonly struct Result
        {
            /// <summary>Rebased-and-replayed predicted state (step 2 then step 3).</summary>
            public readonly ShipSimState State;

            /// <summary>History with acknowledged entries dropped (step 4) and every retained
            /// entry's StateAfter recomputed from the new baseline (so the NEXT reconciliation's
            /// step 1 error is measured against the current replay, not a stale one).</summary>
            public readonly IReadOnlyList<InputRecord> RetainedHistory;

            /// <summary>True when step 1 could measure an error (ack_input_seq was non-null and
            /// found in the retained history before this call). False for the first snapshot
            /// after connecting/reconnecting - there is nothing to compare yet, so this is
            /// "no error to report", not "zero error" (do not chart a zero here).</summary>
            public readonly bool HasError;

            public readonly double PositionErrorM;
            public readonly double OrientationErrorDeg;

            public Result(ShipSimState state, IReadOnlyList<InputRecord> retainedHistory,
                bool hasError, double positionErrorM, double orientationErrorDeg)
            {
                State = state;
                RetainedHistory = retainedHistory;
                HasError = hasError;
                PositionErrorM = positionErrorM;
                OrientationErrorDeg = orientationErrorDeg;
            }
        }

        /// <param name="history">Retained inputs in ascending ServerTick order (the order
        /// PredictionHistory.ApplyInput appends them in - local tick index only ever
        /// increases).</param>
        /// <param name="confirmedState">Server's authoritative state for this tick, already
        /// dequantized (X in the algorithm above).</param>
        /// <param name="snapshotTick">snapshot.tick (S) - ADR-0012 section 6.4: the alignment
        /// key. Not nullable: the envelope's tick is always present (unlike ack_input_seq,
        /// which can be null before the server has applied anything from this session).</param>
        public static Result Reconcile(
            IReadOnlyList<InputRecord> history,
            ShipSimState confirmedState,
            long snapshotTick,
            ShipClassStats ship,
            ShipIntegrator.Boundary boundary,
            double dt)
        {
            // 1. error, measured BEFORE rebasing - against what was predicted for tick S. No
            // match means this session has no retained entry for tick S yet (first snapshot
            // after connect/reconnect, or the client fell behind and every retained entry's
            // ServerTick is already <= S) - "no error to report", not "zero error" (ADR-0012
            // section 3's discipline, preserved verbatim by the rekey).
            bool hasError = false;
            double positionErrorM = 0.0;
            double orientationErrorDeg = 0.0;

            for (int i = 0; i < history.Count; i++)
            {
                if (history[i].ServerTick != snapshotTick) continue;
                positionErrorM = (history[i].StateAfter.Position - confirmedState.Position).Length();
                orientationErrorDeg = Quatd.AngleDegrees(history[i].StateAfter.Orientation, confirmedState.Orientation);
                hasError = true;
                break;
            }

            // 2. rebase on the confirmed state - all five fields (position, velocity,
            // orientation, omega_aim, omega_roll). No partial rebase: ShipSimState is copied
            // wholesale, there is no field-by-field path that could rebase only some of it.
            ShipSimState state = confirmedState;

            // 3 + 4 together: replay everything with ServerTick > S, drop everything <= S. A
            // retained entry with ServerTick <= S is either already reflected in `confirmedState`
            // (normal case) or was predicted for a tick the server has since moved past without
            // ever seeing it (client fell behind) - either way it must not be replayed again.
            var retained = new List<InputRecord>(history.Count);
            for (int i = 0; i < history.Count; i++)
            {
                InputRecord record = history[i];
                if (record.ServerTick <= snapshotTick) continue;

                state = ShipIntegrator.Step(state, record.Input, ship, boundary, dt).State;
                // NOTE: DerivedFromSeq intentionally not carried over here - matches the
                // pre-existing behaviour before this rekey (unrelated to D-1/D-2, not touched).
                retained.Add(new InputRecord(record.InputSeq, record.Input, state, record.ServerTick));
            }

            return new Result(state, retained, hasError, positionErrorM, orientationErrorDeg);
        }
    }
}
