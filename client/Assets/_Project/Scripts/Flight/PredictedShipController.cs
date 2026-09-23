// Hand-written. Stateful wrapper around the pure PredictionHistory/Reconciliation functions -
// the thing a running client actually holds one instance of per controlled ship. All the
// state-changing logic still lives in the pure functions this class calls; this class only
// remembers "what happened last" between calls.

using System.Collections.Generic;
using Starfall.Sim;

namespace Starfall.Flight
{
    public sealed class PredictedShipController
    {
        readonly ShipClassStats _ship;
        readonly ShipIntegrator.Boundary _boundary;
        readonly double _dt;

        List<InputRecord> _history = new List<InputRecord>();

        public ShipSimState CurrentState { get; private set; }
        public IReadOnlyList<InputRecord> History => _history;

        public bool LastReconcileHadError { get; private set; }
        public double LastPositionErrorM { get; private set; }
        public double LastOrientationErrorDeg { get; private set; }

        /// <summary>reconcile_hard_snap_total, client side (design section 6.4 / AC-13(c)).
        /// On a lossless local link this must stay 0 for the whole session - non-zero is a bug
        /// signal, not expected behaviour.</summary>
        public long HardSnapTotal { get; private set; }

        public PredictedShipController(ShipClassStats ship, ShipIntegrator.Boundary boundary, double dt, ShipSimState initialState)
        {
            _ship = ship;
            _boundary = boundary;
            _dt = dt;
            CurrentState = initialState;
        }

        /// <summary>One input sent = one tick predicted (ADR-0012 section 6). Call this exactly
        /// once per SET_SHIP_CONTROL the client sends, with the SAME quantized input
        /// (SetShipControlBuilder.ToDequantizedInput), never with a fresh raw-float value.</summary>
        public void ApplyInput(ShipControlInputD input)
        {
            (ShipSimState next, List<InputRecord> newHistory) = PredictionHistory.ApplyInput(_history, CurrentState, input, _ship, _boundary, _dt);
            CurrentState = next;
            _history = newHistory;
        }

        /// <summary>COMMAND_RESULT{REJECTED, *} for one command_id: the server never applied
        /// this input, so it must not be replayed on the next reconciliation either.</summary>
        public void DropRejected(uint rejectedInputSeq)
        {
            _history = PredictionHistory.DropRejected(_history, rejectedInputSeq);
        }

        /// <summary>Call on every WORLD_SNAPSHOT. Rebases on the confirmed state and replays
        /// unconfirmed inputs (Reconciliation.Reconcile), and updates the render-band /
        /// hard-snap bookkeeping from the result.</summary>
        public Reconciliation.Result Reconcile(ShipSimState confirmedState, uint? ackInputSeq, SyncTuningData tuning)
        {
            Reconciliation.Result result = Reconciliation.Reconcile(_history, confirmedState, ackInputSeq, _ship, _boundary, _dt);

            CurrentState = result.State;
            _history = new List<InputRecord>(result.RetainedHistory);
            LastReconcileHadError = result.HasError;
            LastPositionErrorM = result.PositionErrorM;
            LastOrientationErrorDeg = result.OrientationErrorDeg;

            if (result.HasError)
            {
                ReconcileBand positionBand = RenderOffset.ClassifyPosition(result.PositionErrorM, tuning);
                ReconcileBand orientationBand = RenderOffset.ClassifyOrientation(result.OrientationErrorDeg, tuning);
                if (positionBand == ReconcileBand.HardSnap || orientationBand == ReconcileBand.HardSnap)
                    HardSnapTotal++;
            }

            return result;
        }

        /// <summary>New session (fresh connect, or resume after a reconnect - architect's
        /// confirmed gap: "재개 후 세션·입력 상태는 새로 시작, last_applied_input_seq = None").
        /// Clears the retained-input history entirely and trusts <paramref name="newBaseline"/>
        /// unconditionally - ADR-0012 section 3: "연결이 끊기면 보관 목록을 비우고, 재접속 후
        /// 첫 스냅샷을 무조건 진실로 받는다. 옛 세션의 input_seq를 이어 쓰지 않는다."</summary>
        public void Reset(ShipSimState newBaseline)
        {
            _history = new List<InputRecord>();
            CurrentState = newBaseline;
            LastReconcileHadError = false;
            LastPositionErrorM = 0.0;
            LastOrientationErrorDeg = 0.0;
        }
    }
}
