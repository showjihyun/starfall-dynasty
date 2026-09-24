// Hand-written. The greybox bootstrap: no hand-authored scene, no hand-authored prefab -
// everything here is built from primitives at runtime, the same "no scene, code creates it"
// pattern p0-02's StarfallNetHost established (that file's own header explains why: hand-editing
// scene/prefab YAML breaks GUID references, and this slice's visuals are simple shapes anyway -
// task doc: "단순 도형으로 충분").
//
// This ties together every other C3-C5 piece: Starfall.Sim (prediction math), Starfall.Flight
// (input quantisation, reconciliation), Starfall.Remote (other-ship interpolation), and
// Starfall.Net (RealtimeClient, already running via StarfallNetHost). It is the ONLY place
// double (Sim) state is converted to float (UnityEngine.Vector3/Quaternion) - ADR-0012 section
// 2: "렌더링할 때만 float32로 변환한다".

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Flight;
using Starfall.Net;
using Starfall.Remote;
using Starfall.Sim;
using Unity.Profiling;
using UnityEngine;

namespace Starfall.Greybox
{
    [DisallowMultipleComponent]
    public sealed class GreyboxSession : MonoBehaviour
    {
        /// <summary>Set to 1 to build the greybox scene at play mode start. Mirrors
        /// StarfallNetHost.AutoConnectVariable's pattern - opt-in, so headless contract-only
        /// test runs never pay for scene construction.</summary>
        public const string AutoBuildVariable = "STARFALL_GREYBOX_AUTOBUILD";

        // F-1 (05_qa_report_r5.md §6.1): HUD row layout is derived from these two constants plus
        // HudTextWrap.Wrap(), never from a fixed per-line Rect - see HudTextWrap.cs header for
        // why that is what prevents this bug from recurring as fields are added.
        public const int HudMaxCharsPerLine = 110;
        public const float HudRowPixelWidth = 900f;

        static GreyboxSession _instance;
        public static GreyboxSession Instance => _instance;

        RealtimeClient _client;
        ShipClassCatalog _shipClasses;
        StarSystemData _starSystem;
        SyncTuningData _tuning;
        double _tickDurationSeconds = 1.0 / 20.0;

        PredictedShipController _controller;
        ShipClassStats _controlledShipClass;
        uint _localInputSeq = 1;
        readonly Dictionary<Guid, uint> _pendingInputSeqByCommandId = new Dictionary<Guid, uint>();

        // H-3''/H-4' (architect R4 판정 + 보충 판정 2, K-1): carry-forward/dormant prediction
        // state. _lastSentPayload holds the exact quantized wire ints of the most recent
        // successful send (I-36: reused verbatim, never resampled from raw input -
        // ADR-0012 section 2). _ticksSinceLastSend counts consecutive UNSENT predicted ticks;
        // past TickCatchUp.ProductionCarryForwardMaxTicks it switches to the dormant input
        // (ADR-0011 section 6.1), same as the server's own fallback.
        SetShipControlCommand.SetShipControlPayload _lastSentPayload;
        uint? _lastSentInputSeq;
        int _ticksSinceLastSend;

        // H-2 (RebaseHold, T-2): how long (real seconds) this session has been holding off a
        // rebase because the retained history has an un-sent entry the latest snapshot's
        // ack_input_seq cannot yet resolve. 0 when not holding.
        double _rebaseHoldSeconds;
        float _rebaseHoldLastRealTime;
        long _reconcileForcedAfterHitchTotal;

        // T-7/H-8 observations (catch-up path counters).
        long _catchupCarryForwardTicksTotal;
        long _catchupDormantTicksTotal;
        long _catchupTruncatedTotal;

        readonly SendBurstStats _sendBurstStats = new SendBurstStats();

        readonly RemoteShipRegistry _remoteRegistry = new RemoteShipRegistry();
        readonly Dictionary<Guid, GameObject> _remoteViews = new Dictionary<Guid, GameObject>();

        long _latestSnapshotTick = -1;
        float _latestSnapshotArrivalRealTime;
        int? _lastAckInputSeq;

        GameObject _localShipView;
        ShipInputSampler _input;
        Transform _chaseCameraTransform;

        double _lastPositionErrorM;
        double _lastOrientationErrorDeg;
        long _hardSnapTotal; // session-scope (resets with a fresh _controller on reconnect - already true before H-10')
        int _visibleShipCount;
        double _nearestShipDistanceM = double.PositiveInfinity;

        // H-10' (architect K-4): run-scope counterparts, never reset by OnSessionReady. Only
        // these three exist - "qa가 손으로 더하지 않아도 되도록 셋만". _hardSnapTotalRun
        // accumulates HardSnapTotal deltas across controller instances (each new controller
        // restarts its own HardSnapTotal at 0, so the delta since the last read is what must be
        // added, not the raw value). H-17: max values always carry their own sample count.
        long _hardSnapTotalRun;
        long _hardSnapTotalRunLastControllerValue;
        double _reconcileErrorMMaxRun = double.NaN;
        int _reconcileErrorMMaxRunN;
        double _reconcileErrorDegMaxRun = double.NaN;
        int _reconcileErrorDegMaxRunN;

        // SC-56(b): "재조정 직전 위치 오차 p50/p99/최대를 HUD·로그에" - qa2 flagged (2026-09-22,
        // block 6 pre-flight) that only the LAST error was ever kept, so a whole session read as
        // one number with no distribution. These accumulate every HasError=true reconcile for
        // the session's life; ReconcileErrorStats.Compute (pure, tested in
        // ReconcileErrorStatsTests.cs) is only re-run here at reconcile rate, not per OnGUI frame
        // - the HUD/log below just read the cached PercentileStats.
        readonly List<double> _reconcilePositionErrorsM = new List<double>();
        readonly List<double> _reconcileOrientationErrorsDeg = new List<double>();
        PercentileStats _reconcilePositionErrorStats = PercentileStats.Empty;
        PercentileStats _reconcileOrientationErrorStats = PercentileStats.Empty;

        // CL-2 (architect ruling, QA round 2): AC-13(b)'s p50/p99/max alone read as 0/0/0 when
        // Reconcile() has never validly fired against real server data, which passes the letter
        // of AC-13(b) without answering "did the reconcile path actually run against a live
        // server". These three counters are the observations architect added to close that gap.
        // Fixture cannot fill them - Reconcile()'s "one input per tick" precondition (see
        // ReconciliationTests.cs's Reconcile_RealS6Replay_... header comment) is only ever met
        // by a live 20 Hz client, so only a real block-6 run can move them off zero.
        long _reconcileHasErrorTotal;
        long _reconcileReplayedNonZeroInputsTotal;
        long _reconcileBothOmegaNonZeroTotal;

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void AutoBuildIfRequested()
        {
            if (Environment.GetEnvironmentVariable(AutoBuildVariable) != "1") return;
            Ensure();
        }

        public static GreyboxSession Ensure()
        {
            if (_instance != null) return _instance;
            var go = new GameObject("GreyboxSession");
            DontDestroyOnLoad(go);
            return go.AddComponent<GreyboxSession>();
        }

        void Awake()
        {
            if (_instance != null && _instance != this) { Destroy(gameObject); return; }
            _instance = this;

            LoadData();
            BuildMarkers();
            BuildLocalShipView();
            BuildCamera();

            var host = StarfallNetHost.Ensure();
            _client = host.Client;
            _client.SessionReady += OnSessionReady;
            _client.SessionEnded += OnSessionEnded;
            _client.SupersededElsewhere += OnSupersededElsewhere;
            _client.CommandResultReceived += OnCommandResultReceived;
            _client.Register<WorldSnapshotMessage>("WORLD_SNAPSHOT", OnWorldSnapshot);

            _input = gameObject.AddComponent<ShipInputSampler>();
        }

        void OnDestroy()
        {
            if (_client != null)
            {
                _client.SessionReady -= OnSessionReady;
                _client.SessionEnded -= OnSessionEnded;
                _client.SupersededElsewhere -= OnSupersededElsewhere;
                _client.CommandResultReceived -= OnCommandResultReceived;
            }
            if (_instance == this) _instance = null;
        }

        // ------------------------------------------------------------------ data

        void LoadData()
        {
            string dataRoot = Path.Combine(Application.dataPath, "_Project", "Data");
            _shipClasses = ShipClassCatalog.LoadFromDirectory(Path.Combine(dataRoot, "ships"));

            string cradlePath = Path.Combine(dataRoot, "world", "systems", "cradle.json");
            _starSystem = File.Exists(cradlePath) ? StarSystemData.FromJson(File.ReadAllText(cradlePath)) : null;

            string tuningPath = Path.Combine(dataRoot, "movement", "sync-tuning.json");
            _tuning = File.Exists(tuningPath) ? SyncTuningData.FromJson(File.ReadAllText(tuningPath)) : null;

            if (_starSystem == null || _tuning == null || _shipClasses.Count == 0)
            {
                Debug.LogWarning("starfall.greybox: data/ copy incomplete under " + dataRoot +
                                 " - ship classes=" + _shipClasses.Count +
                                 ", star system=" + (_starSystem != null) + ", tuning=" + (_tuning != null));
            }
        }

        ShipIntegrator.Boundary CurrentBoundary()
        {
            if (_starSystem == null) return new ShipIntegrator.Boundary(10_000.0, 12_000.0, 25.0);
            return new ShipIntegrator.Boundary(_starSystem.SoftBoundaryRadiusM, _starSystem.HardBoundaryRadiusM, _starSystem.BoundaryPullMps2);
        }

        // ------------------------------------------------------------------ scene construction (primitives only)

        void BuildMarkers()
        {
            if (_starSystem == null) return;
            var root = new GameObject("ReferenceMarkers (client-only, never sent, never collided - design doc reference_markers_note)");

            foreach (ReferenceMarker marker in _starSystem.ReferenceMarkers)
            {
                GameObject go = GameObject.CreatePrimitive(PrimitiveType.Sphere);
                go.name = "Marker_" + marker.Id;
                go.transform.SetParent(root.transform, false);
                go.transform.position = ToUnity(marker.PositionM);
                float diameter = (float)marker.VisualRadiusM * 2.0f;
                go.transform.localScale = new Vector3(diameter, diameter, diameter);
                // Grey box: no material assignment (default Unity primitive material is enough
                // to be visible and distinguishable by silhouette/size - "단순 도형으로 충분").
                Destroy(go.GetComponent<Collider>()); // markers never collide (design doc note)
            }

            // F-7's focus-independent half. The HUD line above is what SC-59 (b2) asks for, but
            // it can only be read off a recording whose Game View had focus - the failure mode
            // that cost four shoots in R4. Marker positions are static, so one line at build time
            // carries the same information for a log-only evidence trail, measured from the
            // origin rather than from a ship that does not exist yet.
            Debug.Log("starfall.greybox: reference_markers " +
                      MarkerHudLine.Format(_starSystem.ReferenceMarkers, default));
        }

        void BuildLocalShipView()
        {
            _localShipView = GameObject.CreatePrimitive(PrimitiveType.Capsule);
            _localShipView.name = "LocalShip (controlled, predicted)";
            _localShipView.transform.localScale = new Vector3(4f, 4f, 12f); // long axis suggests +Z forward
            // Unity's capsule primitive's long axis is local Y; rotate it so local Y aligns
            // with the ship's forward (+Z, ADR-0009 section 1) once, at construction time.
            _localShipView.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            _localShipView.SetActive(false); // shown once the first snapshot confirms our ship exists
        }

        GameObject BuildRemoteShipView(Guid shipId)
        {
            GameObject go = GameObject.CreatePrimitive(PrimitiveType.Capsule);
            go.name = "RemoteShip_" + shipId;
            go.transform.localScale = new Vector3(4f, 4f, 12f);
            go.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            return go;
        }

        void BuildCamera()
        {
            var camGo = new GameObject("GreyboxChaseCamera");
            Camera cam = camGo.AddComponent<Camera>();
            cam.nearClipPlane = 0.3f;
            cam.farClipPlane = 50_000f; // play area hard boundary is 12,000 m (ADR-0009 section 3)
            _chaseCameraTransform = camGo.transform;
        }

        // ------------------------------------------------------------------ session lifecycle

        void OnSessionReady(SessionIdentity identity)
        {
            // architect's confirmed gap (재개 section 6.2): session/input state starts fresh on
            // EVERY SESSION_READY, including a resume - only world state (held server-side)
            // carries over. The client must not try to be clever and keep old prediction state.
            _tickDurationSeconds = 1.0 / identity.TickHz;
            _controller = null;
            _controlledShipClass = null;
            _localInputSeq = 1;
            _pendingInputSeqByCommandId.Clear();
            _lastAckInputSeq = null;

            _lastSentPayload = null;
            _lastSentInputSeq = null;
            _ticksSinceLastSend = 0;
            _rebaseHoldSeconds = 0.0;

            // H-9: a rebase queued from the PREVIOUS session/resume must never apply against
            // the fresh _controller instance the next WORLD_SNAPSHOT creates above.
            _pendingRebaseTick = null;
            _pendingRebaseConfirmed = default;
            _pendingRebaseAckInputSeq = null;

            // H-10' (architect K-4): "정본은 세션 범위다. 모든 재조정 지표를 OnSessionReady에서
            // 리셋한다." R5's error lists/cached percentiles and the CL-2 counters were added
            // after this comment block existed and were missed - fixed here. The three _run
            // counterparts (hardSnapTotalRun, reconcileError{M,Deg}MaxRun) are deliberately NOT
            // touched - that is the whole point of a run-scoped metric.
            _reconcilePositionErrorsM.Clear();
            _reconcileOrientationErrorsDeg.Clear();
            _reconcilePositionErrorStats = PercentileStats.Empty;
            _reconcileOrientationErrorStats = PercentileStats.Empty;
            _reconcileHasErrorTotal = 0;
            _reconcileReplayedNonZeroInputsTotal = 0;
            _reconcileBothOmegaNonZeroTotal = 0;
            _hardSnapTotalRunLastControllerValue = 0;

            foreach (GameObject view in _remoteViews.Values) Destroy(view);
            _remoteViews.Clear();

            if (_localShipView != null) _localShipView.SetActive(false);

            Debug.Log("starfall.greybox: session ready, waiting for first WORLD_SNAPSHOT (world state carries over on resume, prediction state does not)");

            // AC-15(e): the interpolation delay must be derived from snapshot_interval_ticks
            // and sync-tuning, not a hardcoded guess - and logged, so a human can confirm it
            // actually came from those values rather than a stale constant.
            double delayMs = _tuning?.RemoteInterpDelayMs ?? 200.0;
            double delayTicks = delayMs / 1000.0 / _tickDurationSeconds;
            Debug.Log("starfall.greybox: remote interpolation delay = " + delayMs + " ms = " +
                      delayTicks.ToString("F2", CultureInfo.InvariantCulture) + " ticks at tick_hz=" + identity.TickHz +
                      " (from sync-tuning.json remote_interp_delay_ms, ADR-0012 section 5)");
        }

        void OnSessionEnded(int droppedCommands)
        {
            if (_localShipView != null) _localShipView.SetActive(false);

            // CL-2: written to the log (client/Logs/Editor.log), not just the HUD, so QA can grep
            // it after the session ends instead of needing a live screenshot of three numbers.
            Debug.Log("starfall.greybox: CL-2 session-end reconcile observations - " +
                      "reconcile_has_error_total=" + _reconcileHasErrorTotal +
                      ", reconcile_replayed_nonzero_total=" + _reconcileReplayedNonZeroInputsTotal +
                      ", reconcile_both_omega_nonzero_total=" + _reconcileBothOmegaNonZeroTotal);

            // SC-56(b)(c) (qa2, block 6 pre-flight, 2026-09-22): p50/p99/max over the WHOLE
            // session, not the last value, plus N so a 0-sample distribution cannot be mistaken
            // for a measured zero (§7a). Grep tag is fixed: "SC-56 session-end reconcile error
            // stats".
            Debug.Log("starfall.greybox: SC-56 session-end reconcile error stats - " +
                      "position_error_m(p50=" + FormatStat(_reconcilePositionErrorStats.P50) +
                      ", p99=" + FormatStat(_reconcilePositionErrorStats.P99) +
                      ", max=" + FormatStat(_reconcilePositionErrorStats.Max) +
                      ", n=" + _reconcilePositionErrorStats.N + "), " +
                      "orientation_error_deg(p50=" + FormatStat(_reconcileOrientationErrorStats.P50) +
                      ", p99=" + FormatStat(_reconcileOrientationErrorStats.P99) +
                      ", max=" + FormatStat(_reconcileOrientationErrorStats.Max) +
                      ", n=" + _reconcileOrientationErrorStats.N + "), " +
                      "reconcile_hard_snap_total=" + _hardSnapTotal);

            // H-10'/H-17 (architect K-4): run-scope (never reset) counterparts, each max with
            // its own n so "no bigger sample landed" and "stopped measuring" cannot be confused.
            Debug.Log("starfall.greybox: SC-56 run-scope reconcile stats (whole Editor Play " +
                      "session, spans every reconnect - NOT the SC-56 judging value, context only) - " +
                      "reconcile_hard_snap_total_run=" + _hardSnapTotalRun +
                      ", reconcile_error_m_max_run=" + FormatStat(_reconcileErrorMMaxRun) + " (n=" + _reconcileErrorMMaxRunN + ")" +
                      ", reconcile_error_deg_max_run=" + FormatStat(_reconcileErrorDegMaxRun) + " (n=" + _reconcileErrorDegMaxRunN + ")");

            // SC-89 (leader-approved 2026-09-23; extended per architect K-7): session worst-case,
            // not a per-frame trickle - written once at session end the same way SC-56 is, so QA
            // can grep it after the fact instead of needing a live screenshot at the exact
            // moment of a burst. Grep tag is fixed: "SC-89 session-end send burst stats".
            // max_sends_per_frame is the K-7 pairing value: a background PAUSE (one huge
            // catch-up frame) would still show max_ticks_drained_per_update > 1 on the OLD
            // (pre-fix) binary, but max_sends_per_frame would ALSO be > 1 there - only the fixed
            // binary holds it at 1 regardless of how large the drain was.
            Debug.Log("starfall.greybox: SC-89 session-end send burst stats - " +
                      "max_ticks_drained_per_update=" + FormatCount(_sendBurstStats.MaxTicksDrainedPerUpdate) +
                      ", max_sends_per_frame=" + FormatCount(_sendBurstStats.MaxSendsPerFrame) +
                      ", max_sends_per_trailing_1s=" + FormatCount(_sendBurstStats.MaxSendsInTrailingOneSecond) +
                      ", catchup_carry_forward_ticks_total=" + _catchupCarryForwardTicksTotal +
                      ", catchup_dormant_ticks_total=" + _catchupDormantTicksTotal +
                      ", catchup_truncated_total=" + _catchupTruncatedTotal +
                      ", reconcile_forced_after_hitch_total=" + _reconcileForcedAfterHitchTotal +
                      ", prediction_history_overflow_total=" + (_controller?.PredictionHistoryOverflowTotal ?? 0) +
                      // H-14 (K-2): observation only, NOT a SC-89 pass/fail condition - nonzero
                      // is a separate potential defect (transport/server receive lag), not a
                      // protocol-violation symptom.
                      ", outbound_queue_full_total=" + (_client?.OutboundQueueFullTotal ?? 0));
        }

        /// <summary>NaN (the N=0 sentinel, <see cref="PercentileStats.Empty"/>) prints as "n/a",
        /// never as a number - a 0-sample stat must not be readable as a measured 0 (§7a).</summary>
        static string FormatStat(double value) =>
            double.IsNaN(value) ? "n/a" : value.ToString("F4", CultureInfo.InvariantCulture);

        // SC-89: same "unmeasured, not measured-and-zero" discipline as FormatStat/
        // PercentileStats.Empty above, for the nullable-int SendBurstStats counters.
        static string FormatCount(int? value) =>
            value.HasValue ? value.Value.ToString(CultureInfo.InvariantCulture) : "n/a";

        /// <summary>R3 decision 5 / client task 1: greybox-level HUD for "connected elsewhere" -
        /// RealtimeClient already stopped reconnecting by the time this fires (close code 4001),
        /// so this is display only, not a decision point.</summary>
        bool _supersededElsewhere;

        void OnSupersededElsewhere()
        {
            _supersededElsewhere = true;
            Debug.LogWarning("starfall.greybox: SUPERSEDED - another session for this actor took over " +
                              "the ship (close code 4001), not reconnecting");
        }

        void OnCommandResultReceived(CommandResultMessage message)
        {
            Guid commandId = message.Payload.CommandId;
            uint inputSeq;
            if (!_pendingInputSeqByCommandId.TryGetValue(commandId, out inputSeq)) return;
            _pendingInputSeqByCommandId.Remove(commandId);

            if (!string.Equals(message.Payload.Status, "ACCEPTED", StringComparison.Ordinal))
            {
                // ADR-0012 section 3: the server never applied this input - stop replaying it.
                _controller?.DropRejected(inputSeq);
            }
        }

        // ------------------------------------------------------------------ WORLD_SNAPSHOT

        /// <summary>SC-58 (sprint contract section H, AC-13(e)): the marker name is fixed by
        /// the contract so QA finds it by searching the Profiler Hierarchy for exactly
        /// "Starfall.Snapshot.Handle". Deep Profile must be OFF when measuring (contract note)
        /// - it inflates every marker's GC Alloc and the per-snapshot number becomes unusable.
        /// Wraps the whole handler (deserialized-message in, prediction/interpolation state
        /// updated out) since that is the unit "1건" refers to in "스냅샷 1건당 할당량".</summary>
        static readonly ProfilerMarker SnapshotHandleMarker = new ProfilerMarker("Starfall.Snapshot.Handle");

        void OnWorldSnapshot(WorldSnapshotMessage message)
        {
            using (SnapshotHandleMarker.Auto())
            {
                OnWorldSnapshotCore(message);
            }
        }

        void OnWorldSnapshotCore(WorldSnapshotMessage message)
        {
            _latestSnapshotTick = message.Tick;
            _latestSnapshotArrivalRealTime = Time.unscaledTime;

            WorldSnapshotMessage.WorldSnapshotPayload payload = message.Payload;
            _lastAckInputSeq = payload.AckInputSeq.HasValue ? (int)payload.AckInputSeq.Value : (int?)null;

            Guid? controlledId = payload.ControlledShipId;
            WorldSnapshotMessage.WorldSnapshotPayload.ShipState controlledWire = null;
            var others = new List<(Guid ShipId, ShipSimState State, string Presence)>();

            for (int i = 0; i < payload.Ships.Length; i++)
            {
                WorldSnapshotMessage.WorldSnapshotPayload.ShipState ship = payload.Ships[i];
                if (controlledId.HasValue && ship.ShipId == controlledId.Value)
                {
                    controlledWire = ship;
                }
                else
                {
                    others.Add((ship.ShipId, ShipStateWire.ToSimState(ship), ship.Presence));
                }
            }

            _visibleShipCount = payload.Ships.Length;
            _remoteRegistry.OnSnapshot(message.Tick, others);

            double nearest = double.PositiveInfinity;
            if (controlledWire != null)
            {
                Vec3d selfPos = ShipStateWire.ToSimState(controlledWire).Position;
                foreach (var other in others)
                {
                    double d = (other.State.Position - selfPos).Length();
                    if (d < nearest) nearest = d;
                }
            }
            _nearestShipDistanceM = nearest;

            if (controlledWire == null) return; // our ship is not in this snapshot (rare: pre-spawn tick)

            if (_controlledShipClass == null || _controlledShipClass.Id != controlledWire.ShipClassId)
                _shipClasses.TryGet(controlledWire.ShipClassId, out _controlledShipClass);

            ShipSimState confirmed = ShipStateWire.ToSimState(controlledWire);

            if (_controller == null)
            {
                // First snapshot of a session (fresh connect or resume): trust it unconditionally,
                // nothing to reconcile against yet (ADR-0012 section 3 / section 7).
                _controller = new PredictedShipController(_controlledShipClass, CurrentBoundary(), _tickDurationSeconds, confirmed);
                if (_localShipView != null) _localShipView.SetActive(true);
                return;
            }

            // H-9 (수신측 스냅샷 중복 처리, 리더 메시지 2026-09-23): RealtimeClient.Pump() drains
            // every queued WORLD_SNAPSHOT synchronously, so more than one can reach this method
            // in the same frame before GreyboxSession.Update() runs again. Only the batch's
            // LATEST tick may drive a rebase - applying an older one after a newer one already
            // landed would rewind CurrentState. Queue it instead of reconciling inline;
            // ApplyPendingRebase() (called once at the top of Update(), after this frame's
            // Pump()-driven messages have all already synchronously landed here) does the
            // actual work exactly once per frame, against whichever snapshot won.
            // SnapshotRebaseBatch.ShouldReplacePending is the pure "which one wins" decision
            // (SnapshotRebaseBatch.cs), covered independently by SnapshotRebaseBatchTests.cs.
            // The interpolation buffer already got EVERY snapshot in this batch, older ones
            // included, via the unconditional _remoteRegistry.OnSnapshot(...) call above -
            // deduplication here applies only to the self-ship rebase path.
            if (SnapshotRebaseBatch.ShouldReplacePending(message.Tick, _pendingRebaseTick))
            {
                _pendingRebaseTick = message.Tick;
                _pendingRebaseConfirmed = confirmed;
                _pendingRebaseAckInputSeq = payload.AckInputSeq;
            }
        }

        long? _pendingRebaseTick;
        ShipSimState _pendingRebaseConfirmed;
        uint? _pendingRebaseAckInputSeq;

        /// <summary>H-9: applies at most one rebase/reconcile per frame, against the latest
        /// WORLD_SNAPSHOT this frame's Pump() drain produced for the controlled ship (see the
        /// queuing comment in OnWorldSnapshotCore). No-op when nothing is pending - the common
        /// case of exactly one (or zero) snapshot per frame.</summary>
        void ApplyPendingRebase()
        {
            if (!_pendingRebaseTick.HasValue) return;

            ShipSimState confirmed = _pendingRebaseConfirmed;
            uint? ackInputSeq = _pendingRebaseAckInputSeq;
            _pendingRebaseTick = null;
            _pendingRebaseConfirmed = default;
            _pendingRebaseAckInputSeq = null;

            // H-2 (RebaseHold, T-2): do not rebase on a snapshot that cannot yet resolve an
            // un-sent (carry-forward/dormant) entry in the history - architect R4 판정 section
            // 5. heldSeconds accumulates real time between snapshots while a hold is active,
            // exactly the way _tickAccumulator accumulates Time.unscaledDeltaTime - reset the
            // moment we are not holding.
            float now = Time.unscaledTime;
            double heldSeconds = _rebaseHoldSeconds > 0.0 ? _rebaseHoldSeconds + (now - _rebaseHoldLastRealTime) : 0.0;
            RebaseHold.Action holdAction = RebaseHold.Evaluate(_controller.History, ackInputSeq, heldSeconds);

            if (holdAction == RebaseHold.Action.HoldAndKeepPredicting)
            {
                _rebaseHoldSeconds = heldSeconds;
                _rebaseHoldLastRealTime = now;
                return; // keep predicting - do not touch _controller.CurrentState/history this snapshot
            }

            if (holdAction == RebaseHold.Action.ForceRebaseDiscardingUnsent)
            {
                _controller.DiscardUnsentHistory();
                _reconcileForcedAfterHitchTotal++;
            }
            _rebaseHoldSeconds = 0.0;
            _rebaseHoldLastRealTime = now;

            Reconciliation.Result result = _controller.Reconcile(confirmed, ackInputSeq, _tuning ?? DefaultTuning());
            if (result.HasError)
            {
                _lastPositionErrorM = result.PositionErrorM;
                _lastOrientationErrorDeg = result.OrientationErrorDeg;
                _reconcileHasErrorTotal++; // CL-2 observation 1: HasError actually measured something

                // SC-56(b): accumulate the distribution, not just the last value. Session-scope
                // (reset in OnSessionReady, H-10').
                _reconcilePositionErrorsM.Add(result.PositionErrorM);
                _reconcileOrientationErrorsDeg.Add(result.OrientationErrorDeg);
                _reconcilePositionErrorStats = ReconcileErrorStats.Compute(_reconcilePositionErrorsM);
                _reconcileOrientationErrorStats = ReconcileErrorStats.Compute(_reconcileOrientationErrorsDeg);

                // H-10'/H-17 run-scope counterparts (architect K-4): never reset, always carry n.
                _reconcileErrorMMaxRunN++;
                if (double.IsNaN(_reconcileErrorMMaxRun) || result.PositionErrorM > _reconcileErrorMMaxRun)
                    _reconcileErrorMMaxRun = result.PositionErrorM;
                _reconcileErrorDegMaxRunN++;
                if (double.IsNaN(_reconcileErrorDegMaxRun) || result.OrientationErrorDeg > _reconcileErrorDegMaxRun)
                    _reconcileErrorDegMaxRun = result.OrientationErrorDeg;
            }
            // CL-2 observation 2: step 3 replayed at least one unconfirmed input on top of the
            // rebase - distinguishes "rebased AND replayed" from "rebased only, nothing to replay".
            if (result.RetainedHistory.Count > 0) _reconcileReplayedNonZeroInputsTotal++;
            // CL-2 observation 3: same ">1 deg/s both fields" bar CL-1's real-S6-replay test uses
            // (ReconciliationTests.cs), but here against the CONFIRMED state of an actual reconcile
            // call, not an offline fixture replay.
            if (confirmed.AngularVelocityAim.Length() > 1.0 && Math.Abs(confirmed.AngularVelocityRoll) > 1.0)
                _reconcileBothOmegaNonZeroTotal++;

            _hardSnapTotal = _controller.HardSnapTotal;

            // H-10' run-scope hard snap (architect K-4): each new _controller instance restarts
            // its own HardSnapTotal at 0 (OnSessionReady/first-snapshot path above), so only the
            // DELTA since the last read carries forward into the run-scope accumulator.
            if (_hardSnapTotal > _hardSnapTotalRunLastControllerValue)
                _hardSnapTotalRun += _hardSnapTotal - _hardSnapTotalRunLastControllerValue;
            _hardSnapTotalRunLastControllerValue = _hardSnapTotal;
        }

        static SyncTuningData DefaultTuning() => new SyncTuningData
        {
            ReconcileIgnoreThresholdM = 0.005,
            ReconcileSmoothThresholdM = 0.25,
            ReconcileHardSnapThresholdM = 5.0,
            ReconcileOrientationIgnoreThresholdDeg = 0.02,
            ReconcileOrientationSmoothThresholdDeg = 1.0,
            ReconcileOrientationHardSnapDeg = 15.0,
            RemoteInterpDelayMs = 200,
            RemoteExtrapolateMaxMs = 250,
        };

        // ------------------------------------------------------------------ per-frame: input, send, render
        //
        // H-3''/H-4'/H-5/H-6 (architect R4 판정 + 보충 판정 + 보충 판정 2, T-3): drain judgement
        // lives entirely in TickCatchUp.Plan (Starfall.Flight, pure) - the loop that used to
        // drain the accumulator directly in this file is gone (SC-89 (c) greps this file for
        // that loop's condition and must find zero matches). This method is a thin adapter:
        // read the plan, execute it.

        double _tickAccumulator;

        void Update()
        {
            // H-9: apply at most one rebase/reconcile for whatever WORLD_SNAPSHOT batch this
            // frame's Pump() (StarfallNetHost.Update(), separate component, execution order vs.
            // this Update() unconfirmed) produced - see ApplyPendingRebase()'s doc comment.
            ApplyPendingRebase();

            if (_input != null) _input.Sample();

            _tickAccumulator += Time.unscaledDeltaTime;

            TickCatchUp.Plan plan = TickCatchUp.Compute(
                _tickAccumulator, _tickDurationSeconds,
                TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);

            _tickAccumulator = plan.RemainingAccumulatorSeconds;
            if (plan.Truncated) _catchupTruncatedTotal++;

            int sendsThisFrame = 0;
            for (int i = 0; i < plan.TicksToPredict; i++)
            {
                bool isSendSlot = i >= plan.TicksToPredict - plan.TicksToSend; // rule 2/3: only the LAST TicksToSend ticks are send attempts
                if (isSendSlot && TrySendCurrentInputForThisTick()) sendsThisFrame++;
                else PredictCarryForwardOrDormantTick();
            }

            _sendBurstStats.RecordUpdate(plan.TicksToPredict);
            _sendBurstStats.RecordFrameSendCount(sendsThisFrame);

            RenderLocalShip();
            RenderRemoteShips();

            MaybeLogPeriodicStatus();
        }

        // H-13 (OnGUI 외 주기적 로그): PeriodicStatusLog.cs has the "why" and the proof this
        // does not depend on window focus. This method is the other half - gate + wall-clock
        // interval + field wiring, called from Update() (never OnGUI, and unconditionally, not
        // behind any focus check) so it keeps producing evidence through exactly the background
        // window OnGUI cannot.
        const double PeriodicStatusLogIntervalSeconds = 5.0;
        double _lastPeriodicStatusLogRealTime = double.NegativeInfinity;

        void MaybeLogPeriodicStatus()
        {
            float now = Time.unscaledTime;
            if (now - _lastPeriodicStatusLogRealTime < PeriodicStatusLogIntervalSeconds) return;
            _lastPeriodicStatusLogRealTime = now;

            double speed = _controller != null ? _controller.CurrentState.Velocity.Length() : 0.0;
            double originDistance = _controller != null ? _controller.CurrentState.Position.Length() : 0.0;

            var line = new PeriodicStatusLog(
                tick: _latestSnapshotTick,
                ackInputSeq: _lastAckInputSeq,
                speedMps: speed,
                originDistanceM: originDistance,
                predictErrorM: _lastPositionErrorM,
                predictErrorDeg: _lastOrientationErrorDeg,
                reconcileHardSnapTotal: _hardSnapTotal,
                sendBurstMaxTicksDrainedPerUpdate: _sendBurstStats.MaxTicksDrainedPerUpdate,
                sendBurstMaxSendsPerFrame: _sendBurstStats.MaxSendsPerFrame,
                catchupCarryForwardTicksTotal: _catchupCarryForwardTicksTotal,
                catchupDormantTicksTotal: _catchupDormantTicksTotal,
                catchupTruncatedTotal: _catchupTruncatedTotal,
                reconcileForcedAfterHitchTotal: _reconcileForcedAfterHitchTotal,
                visibleShips: _visibleShipCount,
                applicationFocused: Application.isFocused,
                // F-8: the same quantised integers OnGUI prints and the wire carries - never a
                // re-derivation, so the log and the HUD can never disagree about what was sent.
                thrustX: _input != null ? Quantization.QuantizeControlAxis(_input.Thrust.X) : 0,
                thrustY: _input != null ? Quantization.QuantizeControlAxis(_input.Thrust.Y) : 0,
                thrustZ: _input != null ? Quantization.QuantizeControlAxis(_input.Thrust.Z) : 0,
                roll: _input != null ? Quantization.QuantizeControlAxis(_input.Roll) : 0,
                aimTargetX: _input != null ? Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.X) : 0,
                aimTargetY: _input != null ? Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.Y) : 0,
                aimTargetZ: _input != null ? Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.Z) : 0,
                aimTargetW: _input != null ? Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.W) : 0,
                // Current predicted orientation, NOT the aim target - the pair is what makes
                // auto-level readable (see PeriodicStatusLog's field block).
                // R16: with no controller these used to print (0,0,0,0) - not a unit quaternion,
                // not any rotation, and silently parsed as data by a reader (the R5 preflight log
                // is full of it). Identity is the honest stand-in and is self-evidently "nothing
                // has been reconciled yet" when paired with tick=-1 on the same line.
                attitudeX: _controller != null ? Quantization.QuantizeQuaternionComponent(_controller.CurrentState.Orientation.X) : 0,
                attitudeY: _controller != null ? Quantization.QuantizeQuaternionComponent(_controller.CurrentState.Orientation.Y) : 0,
                attitudeZ: _controller != null ? Quantization.QuantizeQuaternionComponent(_controller.CurrentState.Orientation.Z) : 0,
                attitudeW: _controller != null ? Quantization.QuantizeQuaternionComponent(_controller.CurrentState.Orientation.W) : Quantization.QuantizeQuaternionComponent(1.0),
                boundarySoftCrossed: originDistance > (_starSystem?.SoftBoundaryRadiusM ?? double.PositiveInfinity),
                flightAssist: _input != null && _input.FlightAssist,
                // R16: position as a VECTOR. origin_distance_m above is its magnitude and cannot
                // distinguish "moved along the bow" from "moved along the negated bow" - the R5
                // session closed SC-56/SC-89 and left exactly that open.
                positionX: _controller != null ? _controller.CurrentState.Position.X : 0.0,
                positionY: _controller != null ? _controller.CurrentState.Position.Y : 0.0,
                positionZ: _controller != null ? _controller.CurrentState.Position.Z : 0.0,
                // R16: raw device deltas, sampled BEFORE the mouse->aim mapping SC-59 tests.
                mouseDeltaXTotal: _input != null ? _input.MouseDeltaXTotal : 0.0,
                mouseDeltaYTotal: _input != null ? _input.MouseDeltaYTotal : 0.0,
                yawDeg: _input != null ? _input.YawDeg : 0.0,
                pitchDeg: _input != null ? _input.PitchDeg : 0.0);

            Debug.Log(line.Format());
        }

        /// <summary>Rule 2/3 (R4 판정): the freshest input, sampled this frame, sent as-is.
        /// Predicts with it only on success - K-1 (2): "송신을 먼저 시도하고, 성공하면 새
        /// 입력으로 예측하고, 실패하면 같은 tick을 이월 입력으로 예측한다."</summary>
        bool TrySendCurrentInputForThisTick()
        {
            if (_client == null || !_client.IsReady || _controller == null || _controlledShipClass == null || _input == null) return false;

            Guid commandId = UuidV7.NewGuid();
            uint seq = _localInputSeq++;

            SetShipControlCommand command = SetShipControlBuilder.Build(
                commandId, seq, _input.Thrust, _input.Roll, _input.AimTargetWorld,
                _input.Brake, _input.FlightAssist,
                DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture));

            string json = ContractJson.Serialize(command);
            if (!_client.TrySendJson(json)) return false; // K-1 (2): falls back to carry-forward,
                                                            // never resent (SET_SHIP_CONTROL is
                                                            // absolute state - ADR-0011 section 4).

            _sendBurstStats.RecordSend(Time.realtimeSinceStartupAsDouble);
            _pendingInputSeqByCommandId[commandId] = seq;

            ShipControlInputD dequantized = SetShipControlBuilder.ToDequantizedInput(command.Payload);
            _controller.ApplyInput(dequantized, derivedFromSeq: null);

            _lastSentPayload = command.Payload;
            _lastSentInputSeq = seq;
            _ticksSinceLastSend = 0;
            return true;
        }

        /// <summary>Rules 4/5 (R4 판정): never sent. Reuses the last actually-sent quantized
        /// wire input verbatim (I-36 - never resampled) while within
        /// TickCatchUp.ProductionCarryForwardMaxTicks of the last send; past that, switches to
        /// the dormant input (ADR-0011 section 6.1) the server itself falls back to. H-15: the
        /// resulting InputRecord carries DerivedFromSeq so DropRejected can drop it too if its
        /// source command is later rejected, and RebaseHold can tell it apart from a real
        /// send.</summary>
        void PredictCarryForwardOrDormantTick()
        {
            if (_controller == null || _controlledShipClass == null) return;

            uint seq = _localInputSeq++;
            _ticksSinceLastSend++;

            bool useDormant = _lastSentPayload == null || _ticksSinceLastSend > TickCatchUp.ProductionCarryForwardMaxTicks;
            ShipControlInputD input = useDormant ? BuildDormantInput(seq) : BuildCarryForwardInput(seq);

            // Sentinel 0u: no real source seq exists yet (session's first ticks, before any
            // send has ever succeeded). Real seqs start at 1 (_localInputSeq), so 0 never
            // collides with an actual command_id's seq in DropRejected/RebaseHold.
            uint derivedFromSeq = _lastSentInputSeq ?? 0u;
            _controller.ApplyInput(input, derivedFromSeq);

            if (useDormant) _catchupDormantTicksTotal++;
            else _catchupCarryForwardTicksTotal++;
        }

        ShipControlInputD BuildCarryForwardInput(uint seq) => new ShipControlInputD(
            seq,
            _lastSentPayload.ThrustXMilli, _lastSentPayload.ThrustYMilli, _lastSentPayload.ThrustZMilli,
            _lastSentPayload.RollMilli,
            _lastSentPayload.AimXMicro, _lastSentPayload.AimYMicro, _lastSentPayload.AimZMicro, _lastSentPayload.AimWMicro,
            _lastSentPayload.Brake, _lastSentPayload.FlightAssist);

        ShipControlInputD BuildDormantInput(uint seq)
        {
            // ADR-0011 section 6.1 dormant input: zero thrust, zero roll, aim held at whatever
            // attitude prediction has already reached, brake off, assist on. Built the same way
            // a real command is (quantize then dequantize) so this obeys I-36 exactly like every
            // other predicted tick, even though it is never sent.
            Quatd currentAim = _controller.CurrentState.Orientation;
            SetShipControlCommand synthetic = SetShipControlBuilder.Build(
                UuidV7.NewGuid(), seq, Vec3d.Zero, 0.0, currentAim, brake: false, flightAssist: true, clientSentAtIso8601OrNull: null);
            return SetShipControlBuilder.ToDequantizedInput(synthetic.Payload);
        }

        void RenderLocalShip()
        {
            if (_controller == null || _localShipView == null || !_localShipView.activeSelf) return;
            ShipSimState state = _controller.CurrentState;
            _localShipView.transform.SetPositionAndRotation(ToUnity(state.Position), ToUnity(state.Orientation));
        }

        void RenderRemoteShips()
        {
            double tuningDelayMs = _tuning?.RemoteInterpDelayMs ?? 200.0;
            double tuningExtrapolateMs = _tuning?.RemoteExtrapolateMaxMs ?? 250.0;
            double delayTicks = tuningDelayMs / 1000.0 / _tickDurationSeconds;
            double extrapolateMaxTicks = tuningExtrapolateMs / 1000.0 / _tickDurationSeconds;

            double elapsedSinceSnapshot = _latestSnapshotTick < 0 ? 0.0 : Time.unscaledTime - _latestSnapshotArrivalRealTime;
            double renderTick = _latestSnapshotTick < 0
                ? 0.0
                : _latestSnapshotTick + elapsedSinceSnapshot / _tickDurationSeconds - delayTicks;

            var seen = new HashSet<Guid>();
            foreach (KeyValuePair<Guid, RemoteShipBuffer> pair in _remoteRegistry.Buffers)
            {
                Guid shipId = pair.Key;
                seen.Add(shipId);

                RemoteShipBuffer.Display display = pair.Value.GetDisplay(renderTick, _tickDurationSeconds, extrapolateMaxTicks);
                if (display.Mode == RemoteShipBuffer.DisplayMode.NoData) continue;

                GameObject view;
                if (!_remoteViews.TryGetValue(shipId, out view))
                {
                    view = BuildRemoteShipView(shipId);
                    _remoteViews[shipId] = view;
                }
                view.transform.SetPositionAndRotation(ToUnity(display.State.Position), ToUnity(display.State.Orientation));
            }

            // Absence from the registry means despawned (RemoteShipRegistry.OnSnapshot already
            // applied that rule) - remove any view whose ship_id is no longer tracked.
            List<Guid> toRemove = null;
            foreach (Guid shipId in _remoteViews.Keys)
                if (!seen.Contains(shipId)) (toRemove ??= new List<Guid>()).Add(shipId);
            if (toRemove != null)
                foreach (Guid shipId in toRemove) { Destroy(_remoteViews[shipId]); _remoteViews.Remove(shipId); }
        }

        void LateUpdate()
        {
            if (_chaseCameraTransform == null || _localShipView == null || !_localShipView.activeSelf) return;

            Transform ship = _localShipView.transform;
            Vector3 desired = ship.position - ship.forward * 30f + ship.up * 8f; // 30 m behind, 8 m up (ADR-0009 section 3: safe well under the 20 km float32 budget)
            _chaseCameraTransform.position = desired;
            _chaseCameraTransform.rotation = Quaternion.LookRotation((ship.position - desired).normalized, ship.up);
        }

        // ------------------------------------------------------------------ HUD (AC-14 evidence: input state must be on screen)

        void OnGUI()
        {
            if (GUI.skin == null) return;

            double speed = _controller != null ? _controller.CurrentState.Velocity.Length() : 0.0;
            double originDistance = _controller != null ? _controller.CurrentState.Position.Length() : 0.0;
            double softRadius = _starSystem?.SoftBoundaryRadiusM ?? double.PositiveInfinity;

            var lines = new List<string>
            {
                "tick=" + _latestSnapshotTick,
                "ack_input_seq=" + (_lastAckInputSeq.HasValue ? _lastAckInputSeq.Value.ToString(CultureInfo.InvariantCulture) : "null"),
                "speed_mps=" + speed.ToString("F1", CultureInfo.InvariantCulture),
                "origin_distance_m=" + originDistance.ToString("F1", CultureInfo.InvariantCulture),
                "predict_error_m=" + _lastPositionErrorM.ToString("F4", CultureInfo.InvariantCulture),
                "predict_error_deg=" + _lastOrientationErrorDeg.ToString("F4", CultureInfo.InvariantCulture),
                "reconcile_hard_snap_total=" + _hardSnapTotal,
                // SC-56(b): p50/p99/max over the whole session so far, not just the last value
                // above - cached PercentileStats, recomputed at reconcile rate (see the
                // HasError branch in OnWorldSnapshotCore), never here in OnGUI.
                "reconcile_error_m p50=" + FormatStat(_reconcilePositionErrorStats.P50) +
                " p99=" + FormatStat(_reconcilePositionErrorStats.P99) +
                " max=" + FormatStat(_reconcilePositionErrorStats.Max) +
                " n=" + _reconcilePositionErrorStats.N,
                "reconcile_error_deg p50=" + FormatStat(_reconcileOrientationErrorStats.P50) +
                " p99=" + FormatStat(_reconcileOrientationErrorStats.P99) +
                " max=" + FormatStat(_reconcileOrientationErrorStats.Max) +
                " n=" + _reconcileOrientationErrorStats.N,
                // CL-2 (AC-13(b) observations, architect ruling): must be read at the END of a
                // block-6 real-server session, not at any one instant - "0" early in a session is
                // normal (first reconcile happens once traffic starts). A FAIL is all three still
                // 0 by the time the session ends.
                "cl2_reconcile_has_error_total=" + _reconcileHasErrorTotal +
                " cl2_reconcile_replayed_nonzero_total=" + _reconcileReplayedNonZeroInputsTotal +
                " cl2_reconcile_both_omega_nonzero_total=" + _reconcileBothOmegaNonZeroTotal,
                // SC-89 (R8/R9 finding, leader-approved 2026-09-23; extended per architect K-7):
                // measurement only, not a verdict - this session's worst-case catch-up burst so
                // far. "n/a" (not "0") until at least one Update()/send has actually happened
                // (§7a discipline). max_sends_per_frame is the K-7 pairing value (see
                // OnSessionEnded for why it must be read alongside max_ticks_drained_per_update).
                "send_burst_max_ticks_per_update=" + FormatCount(_sendBurstStats.MaxTicksDrainedPerUpdate) +
                " send_burst_max_sends_per_frame=" + FormatCount(_sendBurstStats.MaxSendsPerFrame) +
                " send_burst_max_sends_per_trailing_1s=" + FormatCount(_sendBurstStats.MaxSendsInTrailingOneSecond),
                // T-7/H-8 catch-up path observations - all four must be countable (M-17 discipline).
                "catchup_carry_forward_ticks_total=" + _catchupCarryForwardTicksTotal +
                " catchup_dormant_ticks_total=" + _catchupDormantTicksTotal +
                " catchup_truncated_total=" + _catchupTruncatedTotal +
                " reconcile_forced_after_hitch_total=" + _reconcileForcedAfterHitchTotal,
                // F-7 (qa r5): SC-59 (b2)'s "마커 4개의 ID·거리를 한 줄로". Drawn every frame,
                // not only on the opening one - a viewer scrubbing to any point in a recording
                // can then tell which markers the ship is being judged against. The wrap in
                // BuildHudRows is what keeps this (the longest line on the HUD by far) from
                // being the next field F-1 silently eats.
                MarkerHudLine.Format(
                    _starSystem?.ReferenceMarkers,
                    _controller != null ? _controller.CurrentState.Position : default),
                "visible_ships=" + _visibleShipCount,
                "nearest_ship_m=" + (double.IsInfinity(_nearestShipDistanceM) ? "-" : _nearestShipDistanceM.ToString("F1", CultureInfo.InvariantCulture)),
                originDistance > softRadius ? "BOUNDARY WARNING (soft crossed)" : "",
                // R3 decision 5: the only visible sign a player gets that this window lost its
                // ship to a second login elsewhere - the socket is closed and staying closed by
                // this point (RealtimeClient.OnDisconnected already set _wantConnected=false).
                _supersededElsewhere ? "SUPERSEDED - connected elsewhere, not reconnecting" : "",
                // Input state, verbatim quantised integers - the sign-bug detector (AC-14) needs
                // to see exactly what was sent, not an interpretation of it.
                _input != null
                    ? "thrust=(" + Quantization.QuantizeControlAxis(_input.Thrust.X) + "," +
                      Quantization.QuantizeControlAxis(_input.Thrust.Y) + "," +
                      Quantization.QuantizeControlAxis(_input.Thrust.Z) + ") roll=" +
                      Quantization.QuantizeControlAxis(_input.Roll) + " brake=" + _input.Brake +
                      " assist=" + _input.FlightAssist
                    : "no input sampler",
                // SC-59 requires "마우스 위치 또는 목표 자세 표시자" (mouse position or a target-
                // attitude indicator) on screen - without it a recording cannot show WHERE the
                // player aimed, only what thrust/roll they sent. The target attitude quaternion
                // (quantised, same integers the wire carries) is that indicator.
                _input != null
                    ? "aim_target=(" + Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.X) + "," +
                      Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.Y) + "," +
                      Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.Z) + "," +
                      Quantization.QuantizeQuaternionComponent(_input.AimTargetWorld.W) + ")"
                    : "",
            };

            // F-1 fix: rows (not `lines`) drive both the Box height and each Label's Rect, and
            // rows always contains 100% of every `lines` entry's characters (HudTextWrap.Wrap's
            // guarantee) - a line too long for one row becomes two rows instead of a clipped
            // one, so the panel structurally cannot drop a field again as more are added.
            var rows = BuildHudRows(lines);

            GUI.Box(new Rect(8, 8, HudRowPixelWidth + 16, 20 + rows.Count * 18), "");
            for (int i = 0; i < rows.Count; i++)
                GUI.Label(new Rect(16, 12 + i * 18, HudRowPixelWidth, 18), rows[i]);
        }

        /// <summary>Pure layout step, split out of OnGUI so it is unit-testable without a
        /// UnityEngine.GUI context (see HudTextWrap.cs and
        /// HudTextWrapTests.cs). Public, not internal: the EditMode test assembly is a separate
        /// asmdef with no InternalsVisibleTo, and HudMaxCharsPerLine above is public for the
        /// same reason - the two are read together or not at all.</summary>
        public static List<string> BuildHudRows(List<string> lines)
        {
            var rows = new List<string>();
            foreach (string line in lines)
                rows.AddRange(HudTextWrap.Wrap(line, HudMaxCharsPerLine));
            return rows;
        }

        // ------------------------------------------------------------------ double (Sim) -> float (Unity) conversion.
        // The ONLY place this happens (ADR-0012 section 2).

        static Vector3 ToUnity(Vec3d v) => new Vector3((float)v.X, (float)v.Y, (float)v.Z);
        static Quaternion ToUnity(Quatd q) => new Quaternion((float)q.X, (float)q.Y, (float)q.Z, (float)q.W);
    }
}
