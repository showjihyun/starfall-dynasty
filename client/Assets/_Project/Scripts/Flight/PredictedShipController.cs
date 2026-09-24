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

        /// <summary>H-16 (K-1 (4)): prediction_history_overflow_total. Must stay 0 on any
        /// normal path - the cap (carry_forward_max_ticks + M) is a worst-case safety net, not
        /// a value normal play should ever reach.</summary>
        public long PredictionHistoryOverflowTotal { get; private set; }

        /// <summary>H-16 cap: carry_forward_max_ticks (10) + M (20) - the worst case of the two
        /// bounded exit paths from an un-sent carry-forward run (K-1 (4)).</summary>
        public const int MaxHistoryEntries = 30;

        public PredictedShipController(ShipClassStats ship, ShipIntegrator.Boundary boundary, double dt, ShipSimState initialState)
        {
            _ship = ship;
            _boundary = boundary;
            _dt = dt;
            CurrentState = initialState;
        }

        /// <summary>One input sent = one tick predicted (ADR-0012 section 6). Call this exactly
        /// once per SET_SHIP_CONTROL the client sends, with the SAME quantized input
        /// (SetShipControlBuilder.ToDequantizedInput), never with a fresh raw-float value.
        /// <paramref name="derivedFromSeq"/>: null when this tick's own input_seq was actually
        /// sent; the source seq (H-15) when this is a carry-forward/dormant prediction that was
        /// not sent (TickCatchUp rules 4/5). Enforces the H-16 history cap after appending.</summary>
        public void ApplyInput(ShipControlInputD input, uint? derivedFromSeq = null)
        {
            (ShipSimState next, List<InputRecord> newHistory) = PredictionHistory.ApplyInput(
                _history, CurrentState, input, _ship, _boundary, _dt, derivedFromSeq);
            CurrentState = next;
            _history = newHistory;
            EnforceHistoryCap();
        }

        void EnforceHistoryCap()
        {
            (List<InputRecord> trimmed, int dropped) = PredictionHistory.EnforceCap(_history, MaxHistoryEntries);
            if (dropped > 0)
            {
                _history = trimmed;
                PredictionHistoryOverflowTotal += dropped;
            }
        }

        /// <summary>COMMAND_RESULT{REJECTED, *} for one command_id: the server never applied
        /// this input, so it must not be replayed on the next reconciliation either.</summary>
        public void DropRejected(uint rejectedInputSeq)
        {
            _history = PredictionHistory.DropRejected(_history, rejectedInputSeq);
        }

        /// <summary>H-2 (RebaseHold.Action.ForceRebaseDiscardingUnsent): drops every
        /// carry-forward/dormant (never-sent) entry from the retained history before the
        /// caller proceeds to Reconcile - the hold exceeded its 500ms ceiling, so those entries
        /// are discarded outright rather than guessed at (architect R4 판정 section 5).</summary>
        public void DiscardUnsentHistory()
        {
            _history = RebaseHold.DiscardUnsentEntries(_history);
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
