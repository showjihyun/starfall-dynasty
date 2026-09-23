// Hand-written. One WebSocket session + its own prediction (self ship) and interpolation
// (every other ship) pipeline + its own CSV writer (ObserverCsv). This is GreyboxSession's
// network/prediction/interpolation core with the scene visuals, camera and HUD stripped out and
// parameterised so TWO of these can run in one Unity process (SC-64/65, sprint contract section
// 0.11 / client ack issue 6) - each with its own actor_id, socket, buffers and CSV, exactly what
// that section requires and exactly what a bot cannot provide (a bot has no interpolation, so
// its view of another ship carries none of the render-delay this measures).
//
// Not a singleton (unlike GreyboxSession/StarfallNetHost) - TwoSessionHarness owns exactly two
// instances, "A" and "B", distinguished by ObserverLabel.

using System;
using System.Collections.Generic;
using System.Globalization;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Flight;
using Starfall.Net;
using Starfall.Remote;
using Starfall.Sim;
using UnityEngine;

namespace Starfall.Greybox
{
    [DisallowMultipleComponent]
    public sealed class ObserverSession : MonoBehaviour
    {
        public string ObserverLabel { get; private set; }

        /// <summary>True for the observer a human flies (ShipInputSampler-driven, like
        /// GreyboxSession). False for a hover-in-place observer (SC-62: "B의 함선은 스폰 위치
        /// 근처에 머문다") whose only job is to watch A.</summary>
        public bool IsInteractive { get; private set; }

        RealtimeClient _client;
        UnityLogSink _log;
        GreyboxData _data;
        ObserverCsvWriter _csv;
        ShipInputSampler _input;

        PredictedShipController _controller;
        ShipClassStats _controlledShipClass;
        uint _localInputSeq = 1;
        readonly Dictionary<Guid, uint> _pendingInputSeqByCommandId = new Dictionary<Guid, uint>();
        readonly RemoteShipRegistry _remoteRegistry = new RemoteShipRegistry();

        Guid _ownActorId;
        double _tickDurationSeconds = 1.0 / 20.0;
        double _tickAccumulator;
        bool _closed;

        /// <summary>Creates and connects one observer. Awake() is deliberately empty - a
        /// runtime-parameterised component needs its real setup after AddComponent, not in
        /// Awake, so two of these can be constructed with different arguments in the same
        /// frame (StarfallNetHost's Ensure()/Awake() split does not need this because it is a
        /// singleton with no per-instance parameters).</summary>
        public static ObserverSession Create(string label, string actorSubject, string csvPath, bool isInteractive)
        {
            var go = new GameObject("ObserverSession_" + label);
            DontDestroyOnLoad(go);
            ObserverSession session = go.AddComponent<ObserverSession>();
            session.Init(label, actorSubject, csvPath, isInteractive);
            return session;
        }

        void Init(string label, string actorSubject, string csvPath, bool isInteractive)
        {
            ObserverLabel = label;
            IsInteractive = isInteractive;

            _log = new UnityLogSink();
            _client = new RealtimeClient(new PcWebSocketTransport(_log), _log,
                jitterSeed: Environment.TickCount ^ label.GetHashCode());
            _client.SessionReady += OnSessionReady;
            _client.SessionEnded += OnSessionEnded;
            _client.CommandResultReceived += OnCommandResultReceived;
            _client.Register<WorldSnapshotMessage>("WORLD_SNAPSHOT", OnWorldSnapshot);

            _data = GreyboxDataLoader.Load("starfall.observer." + label + ": ");
            _csv = new ObserverCsvWriter(csvPath);

            if (isInteractive) _input = gameObject.AddComponent<ShipInputSampler>();

            Application.quitting += OnApplicationQuitting;
#if UNITY_EDITOR
            UnityEditor.AssemblyReloadEvents.beforeAssemblyReload += OnBeforeAssemblyReload;
            UnityEditor.EditorApplication.playModeStateChanged += OnPlayModeStateChanged;
#endif

            string secret = DevAuthToken.ResolveSecret();
            if (secret == null)
            {
                Debug.LogError("starfall.observer." + label + ": " + DevAuthToken.SecretVariable +
                               " is not set - cannot connect (see .env.example)");
                return;
            }

            string url = Environment.GetEnvironmentVariable(StarfallNetHost.UrlVariable);
            if (string.IsNullOrEmpty(url)) url = StarfallNetHost.DefaultUrl;

            string token = DevAuthToken.Create(actorSubject, secret);
            Debug.Log("starfall.observer." + label + ": connecting to " + url + " as subject " + actorSubject +
                      ", csv=" + csvPath);
            _client.Connect(url, token);
        }

        ShipIntegrator.Boundary CurrentBoundary()
        {
            if (_data.StarSystem == null) return new ShipIntegrator.Boundary(10_000.0, 12_000.0, 25.0);
            return new ShipIntegrator.Boundary(
                _data.StarSystem.SoftBoundaryRadiusM, _data.StarSystem.HardBoundaryRadiusM, _data.StarSystem.BoundaryPullMps2);
        }

        // ------------------------------------------------------------------ session lifecycle

        void OnSessionReady(SessionIdentity identity)
        {
            _tickDurationSeconds = 1.0 / identity.TickHz;
            _ownActorId = identity.ActorId;
            _controller = null;
            _controlledShipClass = null;
            _localInputSeq = 1;
            _pendingInputSeqByCommandId.Clear();
            Debug.Log("starfall.observer." + ObserverLabel + ": session ready, actor_id=" + identity.ActorId);
        }

        void OnSessionEnded(int droppedCommands) { }

        void OnCommandResultReceived(CommandResultMessage message)
        {
            Guid commandId = message.Payload.CommandId;
            uint inputSeq;
            if (!_pendingInputSeqByCommandId.TryGetValue(commandId, out inputSeq)) return;
            _pendingInputSeqByCommandId.Remove(commandId);

            if (!string.Equals(message.Payload.Status, "ACCEPTED", StringComparison.Ordinal))
                _controller?.DropRejected(inputSeq);
        }

        // ------------------------------------------------------------------ WORLD_SNAPSHOT -> CSV

        void OnWorldSnapshot(WorldSnapshotMessage message)
        {
            WorldSnapshotMessage.WorldSnapshotPayload payload = message.Payload;
            Guid? controlledId = payload.ControlledShipId;

            WorldSnapshotMessage.WorldSnapshotPayload.ShipState controlledWire = null;
            var others = new List<(Guid ShipId, ShipSimState State, string Presence)>();

            for (int i = 0; i < payload.Ships.Length; i++)
            {
                WorldSnapshotMessage.WorldSnapshotPayload.ShipState ship = payload.Ships[i];
                if (controlledId.HasValue && ship.ShipId == controlledId.Value) controlledWire = ship;
                else others.Add((ship.ShipId, ShipStateWire.ToSimState(ship), ship.Presence));
            }

            _remoteRegistry.OnSnapshot(message.Tick, others);

            if (controlledWire != null)
            {
                if (_controlledShipClass == null || _controlledShipClass.Id != controlledWire.ShipClassId)
                    _data.ShipClasses.TryGet(controlledWire.ShipClassId, out _controlledShipClass);

                ShipSimState confirmed = ShipStateWire.ToSimState(controlledWire);

                if (_controller == null)
                {
                    // First snapshot of a session: trust it unconditionally (ADR-0012 section 3/7),
                    // same as GreyboxSession.
                    _controller = new PredictedShipController(_controlledShipClass, CurrentBoundary(), _tickDurationSeconds, confirmed);
                }
                else
                {
                    _controller.Reconcile(confirmed, payload.AckInputSeq, _data.Tuning ?? DefaultTuning());
                }

                // Own ship: what THIS observer's screen shows for its own ship right now -
                // predicted, not the raw wire value (SC-64/65 measure the two observers'
                // SCREENS, not the two observers' raw snapshots - the latter is SC-63's job and
                // is intentionally tautological, see two_client_view.py's own comment).
                WriteRow(message.Tick, controlledWire.ShipId, controlledWire.Presence, _controller.CurrentState);
            }

            // Every other ship: interpolated at (latest known tick - interpolation delay), the
            // exact quantity SC-65's expected 28 m comes from (200 ms of travel at 140 m/s).
            double delayMs = _data.Tuning?.RemoteInterpDelayMs ?? 200.0;
            double extrapolateMs = _data.Tuning?.RemoteExtrapolateMaxMs ?? 250.0;
            double delayTicks = delayMs / 1000.0 / _tickDurationSeconds;
            double extrapolateMaxTicks = extrapolateMs / 1000.0 / _tickDurationSeconds;
            double renderTick = message.Tick - delayTicks;

            foreach (KeyValuePair<Guid, RemoteShipBuffer> pair in _remoteRegistry.Buffers)
            {
                RemoteShipBuffer.Display display = pair.Value.GetDisplay(renderTick, _tickDurationSeconds, extrapolateMaxTicks);
                if (display.Mode == RemoteShipBuffer.DisplayMode.NoData) continue;
                WriteRow(message.Tick, pair.Key, display.Presence, display.State);
            }
        }

        void WriteRow(long tick, Guid shipId, string presence, ShipSimState state)
        {
            _csv.Write(new ObserverCsvRow(
                tick, _ownActorId, shipId, presence,
                Quantization.QuantizePosition(state.Position.X),
                Quantization.QuantizePosition(state.Position.Y),
                Quantization.QuantizePosition(state.Position.Z),
                Quantization.QuantizeVelocity(state.Velocity.X),
                Quantization.QuantizeVelocity(state.Velocity.Y),
                Quantization.QuantizeVelocity(state.Velocity.Z)));
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

        // ------------------------------------------------------------------ per-frame: input, send, predict

        void Update()
        {
            if (_client != null) _client.Pump();
            if (_input != null) _input.Sample();

            _tickAccumulator += Time.unscaledDeltaTime;
            while (_tickAccumulator >= _tickDurationSeconds)
            {
                _tickAccumulator -= _tickDurationSeconds;
                SendAndPredictOneTick();
            }
        }

        void SendAndPredictOneTick()
        {
            if (_client == null || !_client.IsReady || _controller == null || _controlledShipClass == null) return;

            Guid commandId = UuidV7.NewGuid();
            uint seq = _localInputSeq++;

            Vec3d thrust;
            double roll;
            Quatd aim;
            bool brake;
            bool flightAssist;

            if (IsInteractive && _input != null)
            {
                thrust = _input.Thrust;
                roll = _input.Roll;
                aim = _input.AimTargetWorld;
                brake = _input.Brake;
                flightAssist = _input.FlightAssist;
            }
            else
            {
                // B: hold current attitude, no thrust/roll - SC-62's "스폰 위치 근처에 머문다".
                thrust = Vec3d.Zero;
                roll = 0.0;
                aim = _controller.CurrentState.Orientation;
                brake = false;
                flightAssist = true;
            }

            SetShipControlCommand command = SetShipControlBuilder.Build(
                commandId, seq, thrust, roll, aim, brake, flightAssist,
                DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture));

            string json = ContractJson.Serialize(command);
            if (!_client.TrySendJson(json)) return;

            _pendingInputSeqByCommandId[commandId] = seq;
            ShipControlInputD dequantized = SetShipControlBuilder.ToDequantizedInput(command.Payload);
            _controller.ApplyInput(dequantized);
        }

        // ------------------------------------------------------------------ shutdown (mirrors StarfallNetHost)

        public void Shutdown() => CloseOnce(ClientCloseReason.ClientClosed);

        void OnApplicationQuitting() => CloseOnce(ClientCloseReason.AppQuit);

        void OnDestroy()
        {
            CloseOnce(ClientCloseReason.AppQuit);
            Application.quitting -= OnApplicationQuitting;
#if UNITY_EDITOR
            UnityEditor.AssemblyReloadEvents.beforeAssemblyReload -= OnBeforeAssemblyReload;
            UnityEditor.EditorApplication.playModeStateChanged -= OnPlayModeStateChanged;
#endif
            if (_client != null) { _client.Dispose(); _client = null; }
            _csv?.Dispose();
        }

#if UNITY_EDITOR
        void OnBeforeAssemblyReload() => CloseOnce(ClientCloseReason.EditorReload);

        void OnPlayModeStateChanged(UnityEditor.PlayModeStateChange change)
        {
            if (change == UnityEditor.PlayModeStateChange.ExitingPlayMode) CloseOnce(ClientCloseReason.PlayModeExit);
        }
#endif

        void CloseOnce(ClientCloseReason reason)
        {
            if (_closed || _client == null) return;
            _closed = true;

            bool completed = _client.DisconnectBlocking(reason, StarfallNetHost.CloseBudgetMs);
            if (!completed)
            {
                Debug.LogWarning("starfall.observer." + ObserverLabel + ": close did not finish within " +
                                 StarfallNetHost.CloseBudgetMs.ToString(CultureInfo.InvariantCulture) +
                                 " ms (reason=" + StarfallNetLog.Name(reason) + ")");
            }
        }
    }
}
