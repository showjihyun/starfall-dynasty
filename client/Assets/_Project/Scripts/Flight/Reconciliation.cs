// Hand-written. ADR-0012 section 3, the algorithm transcribed step for step:
//
//   S = snapshot.ack_input_seq            # null means nothing has been applied yet
//   X = snapshot.ships[controlled_ship_id]     # dequantized confirmed state
//
//   1. error = |retained_predicted_state[S].p - X.p|   # skipped if S is null or not retained
//   2. predicted_state = X                              # position, velocity, attitude, BOTH angular velocities
//   3. for seq in (S+1 .. last_sent_seq): predicted_state = integrate(predicted_state, input[seq])
//   4. drop retained entries with seq <= S
//   5. render offset from the error magnitude (Flight.RenderOffset, not this file)
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

        /// <param name="history">Retained inputs in ascending input_seq order (the order
        /// PredictionHistory.ApplyInput appends them in).</param>
        /// <param name="confirmedState">Server's authoritative state for this tick, already
        /// dequantized (X in the algorithm above).</param>
        /// <param name="ackInputSeq">snapshot.ack_input_seq (S). Null means the server has not
        /// applied anything from this session yet.</param>
        public static Result Reconcile(
            IReadOnlyList<InputRecord> history,
            ShipSimState confirmedState,
            uint? ackInputSeq,
            ShipClassStats ship,
            ShipIntegrator.Boundary boundary,
            double dt)
        {
            // 1. error, measured BEFORE rebasing - against what was predicted for tick S.
            bool hasError = false;
            double positionErrorM = 0.0;
            double orientationErrorDeg = 0.0;

            if (ackInputSeq.HasValue)
            {
                for (int i = 0; i < history.Count; i++)
                {
                    if (history[i].InputSeq != ackInputSeq.Value) continue;
                    positionErrorM = (history[i].StateAfter.Position - confirmedState.Position).Length();
                    orientationErrorDeg = Quatd.AngleDegrees(history[i].StateAfter.Orientation, confirmedState.Orientation);
                    hasError = true;
                    break;
                }
            }

            // 2. rebase on the confirmed state - all five fields (position, velocity,
            // orientation, omega_aim, omega_roll). No partial rebase: ShipSimState is copied
            // wholesale, there is no field-by-field path that could rebase only some of it.
            ShipSimState state = confirmedState;

            // 3 + 4 together: replay everything with input_seq > S, drop everything <= S.
            // When ackInputSeq is null every retained entry has seq > null in the ADR's sense
            // (nothing has been confirmed yet), so nothing is dropped and everything replays -
            // exactly "S가 null이면 아직 아무것도 반영되지 않았다".
            var retained = new List<InputRecord>(history.Count);
            for (int i = 0; i < history.Count; i++)
            {
                InputRecord record = history[i];
                if (ackInputSeq.HasValue && record.InputSeq <= ackInputSeq.Value) continue;

                state = ShipIntegrator.Step(state, record.Input, ship, boundary, dt).State;
                retained.Add(new InputRecord(record.InputSeq, record.Input, state));
            }

            return new Result(state, retained, hasError, positionErrorM, orientationErrorDeg);
        }
    }
}
