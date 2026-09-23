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
        long _hardSnapTotal;
        int _visibleShipCount;
        double _nearestShipDistanceM = double.PositiveInfinity;

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
        }

        /// <summary>NaN (the N=0 sentinel, <see cref="PercentileStats.Empty"/>) prints as "n/a",
        /// never as a number - a 0-sample stat must not be readable as a measured 0 (§7a).</summary>
        static string FormatStat(double value) =>
            double.IsNaN(value) ? "n/a" : value.ToString("F4", CultureInfo.InvariantCulture);

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

            Reconciliation.Result result = _controller.Reconcile(confirmed, payload.AckInputSeq, _tuning ?? DefaultTuning());
            if (result.HasError)
            {
                _lastPositionErrorM = result.PositionErrorM;
                _lastOrientationErrorDeg = result.OrientationErrorDeg;
                _reconcileHasErrorTotal++; // CL-2 observation 1: HasError actually measured something

                // SC-56(b): accumulate the distribution, not just the last value.
                _reconcilePositionErrorsM.Add(result.PositionErrorM);
                _reconcileOrientationErrorsDeg.Add(result.OrientationErrorDeg);
                _reconcilePositionErrorStats = ReconcileErrorStats.Compute(_reconcilePositionErrorsM);
                _reconcileOrientationErrorStats = ReconcileErrorStats.Compute(_reconcileOrientationErrorsDeg);
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

        double _tickAccumulator;

        void Update()
        {
            if (_input != null) _input.Sample();

            _tickAccumulator += Time.unscaledDeltaTime;
            while (_tickAccumulator >= _tickDurationSeconds)
            {
                _tickAccumulator -= _tickDurationSeconds;
                SendAndPredictOneTick();
            }

            RenderLocalShip();
            RenderRemoteShips();
        }

        void SendAndPredictOneTick()
        {
            if (_client == null || !_client.IsReady || _controller == null || _controlledShipClass == null || _input == null) return;

            Guid commandId = UuidV7.NewGuid();
            uint seq = _localInputSeq++;

            SetShipControlCommand command = SetShipControlBuilder.Build(
                commandId, seq, _input.Thrust, _input.Roll, _input.AimTargetWorld,
                _input.Brake, _input.FlightAssist,
                DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture));

            string json = ContractJson.Serialize(command);
            if (!_client.TrySendJson(json)) return;

            _pendingInputSeqByCommandId[commandId] = seq;

            ShipControlInputD dequantized = SetShipControlBuilder.ToDequantizedInput(command.Payload);
            _controller.ApplyInput(dequantized);
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

            GUI.Box(new Rect(8, 8, 420, 20 + lines.Count * 18), "");
            for (int i = 0; i < lines.Count; i++)
                GUI.Label(new Rect(16, 12 + i * 18, 400, 18), lines[i]);
        }

        // ------------------------------------------------------------------ double (Sim) -> float (Unity) conversion.
        // The ONLY place this happens (ADR-0012 section 2).

        static Vector3 ToUnity(Vec3d v) => new Vector3((float)v.X, (float)v.Y, (float)v.Z);
        static Quaternion ToUnity(Quatd q) => new Quaternion((float)q.X, (float)q.Y, (float)q.Z, (float)q.W);
    }
}
