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

        /// <summary>D-1 (R8 판정, ADR-0012 section 6.4 point 3): this controller's own local tick
        /// index - the server tick a freshly-predicted entry is stamped with is this value AFTER
        /// incrementing by one. Starts at the tick of the first confirmed snapshot (the
        /// constructor's/Reset's baselineTick) and is re-pinned FORWARD (never rewound - retained
        /// history entries already sit above snapshot.tick by construction, ADR-0012 section
        /// 6.4 point 3: "재고정") on every Reconcile call whose snapshotTick has caught up to or
        /// passed it - the "client fell behind" case (a forced rebase after RebaseHold's ceiling,
        /// or a TickCatchUp truncation, R8 판정 §A-4 D-2 rule 3).</summary>
        public long CurrentTickIndex { get; private set; }

        public bool LastReconcileHadError { get; private set; }
        public double LastPositionErrorM { get; private set; }
        public double LastOrientationErrorDeg { get; private set; }

        /// <summary>reconcile_hard_snap_total, client side (design section 6.4 / AC-13(c)).
        /// On a lossless local link this must stay 0 for the whole session - non-zero is a bug
        /// signal, not expected behaviour.</summary>
        public long HardSnapTotal { get; private set; }

        /// <summary>F-27 (architect R10 판정 §1.4, 01_architect_decisions.md "## R10 판정"):
        /// the render-position jump the LAST Reconcile() call actually produced - |position
        /// immediately before that call (what was on screen the previous frame) minus
        /// result.State.Position (what is on screen now)|. Measured unconditionally, OUTSIDE the
        /// `if (result.HasError)` gate HardSnapTotal lives behind - qa r10 measured a 49.00 m
        /// jump (400 ms stall) and a 391.56 m jump (3000 ms stall) where HasError was FALSE both
        /// times, so HardSnapTotal never saw either one. This is a different quantity from
        /// PositionErrorM: that compares a stored prediction against the confirmed state at the
        /// SAME tick (step 1, only meaningful when HasError); this compares what was actually
        /// rendered, one frame to the next, regardless of whether step 1 had anything to compare
        /// against.</summary>
        public double LastRebaseJumpM { get; private set; }

        /// <summary>Running max of LastRebaseJumpM for this controller's lifetime (resets with a
        /// fresh controller on reconnect, same scope as HardSnapTotal), and N (sample count over
        /// EVERY Reconcile() call) - SC-56 (b)'s "max 는 언제나 n 과 함께" discipline applied to
        /// this gauge too.</summary>
        public double RebaseJumpMaxM { get; private set; }
        public long RebaseJumpN { get; private set; }

        /// <summary>F-27 (architect R10 §1.3): how many ticks the client was behind the snapshot
        /// it just rebased on (Flight.ReconcileRebaseJumpBudget.BehindTicks), and the portion of
        /// LastRebaseJumpM elapsed time alone explains (ExplainedM). Both from the MOST RECENT
        /// Reconcile() call - callers (event logs) read these immediately after Reconcile()
        /// returns, before the next call overwrites them.</summary>
        public long LastBehindTicks { get; private set; }
        public double LastExplainedM { get; private set; }

        /// <summary>SC-56 (c4): count of Reconcile() calls where LastRebaseJumpM exceeded
        /// LastExplainedM plus the existing reconcile_hard_snap_threshold_m slack (Flight.
        /// ReconcileRebaseJumpBudget.IsUnexplained) - a jump elapsed time cannot account for.
        /// Distinct from HardSnapTotal (that one only fires when HasError == true; this one is
        /// defined whether or not step 1 had a comparison) and distinct from the R21-round-1
        /// design this supersedes (a plain "HasError == false" count would have conflated "client
        /// implementation drifted" with "client was asleep for N ticks", the same defect qa r10
        /// found in reconcile_tick_drift_max / F-28). Contract gate: must be 0.</summary>
        public long UnexplainedJumpTotal { get; private set; }

        /// <summary>SC-56 (c4) reporting (architect: "== 0 을 걸지 않는다" - Editor domain
        /// reloads/GC/focus loss make this common regardless of product code, so gating on it
        /// would FAIL every Editor session for reasons unrelated to a bug). Count of reconciles
        /// where LastBehindTicks &gt; 0, the largest LastBehindTicks seen, and the largest
        /// LastRebaseJumpM seen AT one of those behind-reconciles specifically (not the
        /// unconditional RebaseJumpMaxM above, which also counts jumps at behindTicks == 0).</summary>
        public long ClientBehindTotal { get; private set; }
        public long ClientBehindMaxTicks { get; private set; }
        public double ClientBehindMaxJumpM { get; private set; }

        /// <summary>H-16 (K-1 (4)): prediction_history_overflow_total. Must stay 0 on any
        /// normal path - the cap (carry_forward_max_ticks + M) is a worst-case safety net, not
        /// a value normal play should ever reach.</summary>
        public long PredictionHistoryOverflowTotal { get; private set; }

        /// <summary>H-16 cap: carry_forward_max_ticks (10) + M (20) - the worst case of the two
        /// bounded exit paths from an un-sent carry-forward run (K-1 (4)).</summary>
        public const int MaxHistoryEntries = 30;

        public PredictedShipController(ShipClassStats ship, ShipIntegrator.Boundary boundary, double dt, ShipSimState initialState, long initialTick = 0)
        {
            _ship = ship;
            _boundary = boundary;
            _dt = dt;
            CurrentState = initialState;
            CurrentTickIndex = initialTick;
        }

        /// <summary>One input sent = one tick predicted (ADR-0012 section 6). Call this exactly
        /// once per SET_SHIP_CONTROL the client sends, with the SAME quantized input
        /// (SetShipControlBuilder.ToDequantizedInput), never with a fresh raw-float value.
        /// <paramref name="derivedFromSeq"/>: null when this tick's own input_seq was actually
        /// sent; the source seq (H-15) when this is a carry-forward/dormant prediction that was
        /// not sent (TickCatchUp rules 4/5). Enforces the H-16 history cap after appending.</summary>
        public void ApplyInput(ShipControlInputD input, uint? derivedFromSeq = null)
        {
            long tick = CurrentTickIndex + 1;
            (ShipSimState next, List<InputRecord> newHistory) = PredictionHistory.ApplyInput(
                _history, CurrentState, input, _ship, _boundary, _dt, tick, derivedFromSeq);
            CurrentState = next;
            _history = newHistory;
            CurrentTickIndex = tick;
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
        /// hard-snap bookkeeping from the result. D-1 (ADR-0012 section 6.4 point 3):
        /// <paramref name="snapshotTick"/> is both the alignment key Reconcile uses and the
        /// floor CurrentTickIndex is re-pinned to - never rewound, since any retained (replayed)
        /// entry already sits above it by construction (Reconcile step 3/4).</summary>
        public Reconciliation.Result Reconcile(ShipSimState confirmedState, long snapshotTick, SyncTuningData tuning)
        {
            Vec3d preReconcilePosition = CurrentState.Position;
            double preReconcileSpeedMps = CurrentState.Velocity.Length();
            long currentTickIndexBeforeReconcile = CurrentTickIndex;

            Reconciliation.Result result = Reconciliation.Reconcile(_history, confirmedState, snapshotTick, _ship, _boundary, _dt);

            // F-27 (architect R10 §1.4): measured for EVERY Reconcile() call, before the
            // HasError gate below - the jump this reconcile actually put on screen, whether or
            // not step 1 had a comparison to report.
            LastRebaseJumpM = (preReconcilePosition - result.State.Position).Length();
            RebaseJumpN++;
            if (LastRebaseJumpM > RebaseJumpMaxM) RebaseJumpMaxM = LastRebaseJumpM;

            // F-27 (architect R10 §1.3): "explained by elapsed time" budget - a comparison-less
            // reconcile is only a defect (SC-56 (c4)) when the jump exceeds what behind_ticks x
            // speed x dt (plus the existing hard-snap slack) can account for.
            LastBehindTicks = ReconcileRebaseJumpBudget.BehindTicks(snapshotTick, currentTickIndexBeforeReconcile);
            LastExplainedM = ReconcileRebaseJumpBudget.ExplainedM(preReconcileSpeedMps, LastBehindTicks, _dt);
            if (ReconcileRebaseJumpBudget.IsUnexplained(LastRebaseJumpM, LastExplainedM, tuning.ReconcileHardSnapThresholdM))
                UnexplainedJumpTotal++;

            if (LastBehindTicks > 0)
            {
                ClientBehindTotal++;
                if (LastBehindTicks > ClientBehindMaxTicks) ClientBehindMaxTicks = LastBehindTicks;
                if (LastRebaseJumpM > ClientBehindMaxJumpM) ClientBehindMaxJumpM = LastRebaseJumpM;
            }

            CurrentState = result.State;
            _history = new List<InputRecord>(result.RetainedHistory);
            if (CurrentTickIndex < snapshotTick) CurrentTickIndex = snapshotTick;
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
        public void Reset(ShipSimState newBaseline, long baselineTick = 0)
        {
            _history = new List<InputRecord>();
            CurrentState = newBaseline;
            CurrentTickIndex = baselineTick;
            LastReconcileHadError = false;
            LastPositionErrorM = 0.0;
            LastOrientationErrorDeg = 0.0;
            LastRebaseJumpM = 0.0;
            LastBehindTicks = 0;
            LastExplainedM = 0.0;
            // RebaseJumpMaxM/RebaseJumpN, UnexplainedJumpTotal, ClientBehind*, HardSnapTotal:
            // NOT reset here - same scope as HardSnapTotal above (session-scope, tied to this
            // controller instance; GreyboxSession/ObserverSession construct a FRESH
            // PredictedShipController on reconnect, which is what actually zeroes them, matching
            // HardSnapTotal's existing behaviour rather than adding a second, inconsistent reset
            // rule).
        }
    }
}
