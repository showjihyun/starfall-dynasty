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

        // R23 (architect R10 후속 판정 §1, 01_architect_decisions.md "## R10 후속 판정 (R22
        // 이후)"): F-33 render smoothing, applied to the own-ship CSV row - SC-64/65 compare
        // this observer's screen against the other observer's screen, and the other observer's
        // row is already a pure presentation value (RemoteShipBuffer interpolation, no sim state
        // for a remote ship exists at all). Without this, the own-ship row stays at the sim
        // layer and the comparison measures an unnamed quantity (architect's exact wording).
        Vec3d _renderPositionOffset = Vec3d.Zero;
        Quatd _renderOrientationOffset = Quatd.Identity;
        long _smoothedReconcileTotal;         // SC-56 (e) item 1
        long _renderOffsetNonZeroFrameTotal;  // SC-56 (e) item 2
        double _renderOffsetMaxM;             // SC-56 (e) item 3
        long _renderOffsetMaxN;
        long _renderOffsetDecayFrameTotal;    // SC-56 (e) item 4 (12차 신설)

        /// <summary>Same purpose as GreyboxSession's field of the same name - set false at the
        /// top of every ApplyPendingRebase() call, true only if it actually reached
        /// _controller.Reconcile() this frame, so DecayRenderOffset() (called later the SAME
        /// Update()) can tell "shrank because of decay" apart from "just got replaced".</summary>
        bool _reconciledThisFrame;

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
            _pendingRebase.Clear();
            _driftPrevAck = null; // F-21: new session, no drift baseline yet
            _driftPrevTick = 0;

            // R23 (architect R10 후속 판정 §1): render smoothing state is per-session, same rule
            // as GreyboxSession's OnSessionReady reset.
            _renderPositionOffset = Vec3d.Zero;
            _renderOrientationOffset = Quatd.Identity;
            _smoothedReconcileTotal = 0;
            _renderOffsetNonZeroFrameTotal = 0;
            _renderOffsetMaxM = 0.0;
            _renderOffsetMaxN = 0;
            _renderOffsetDecayFrameTotal = 0;

            Debug.Log("starfall.observer." + ObserverLabel + ": session ready, actor_id=" + identity.ActorId);
        }

        void OnSessionEnded(int droppedCommands)
        {
            // R23 (architect R10 후속 판정 §1 "감쇠 함정"): Observer has no HUD and no
            // RenderLocalShip() to fold this into, so this is the ONLY place a reader can confirm
            // the decay loop actually ran this session - without it, a frozen offset shows up
            // nowhere except as quietly-wrong numbers in the CSV.
            Debug.Log("starfall.observer." + ObserverLabel + ": SC-56 (e) session-end render smoothing stats - " +
                      "render_smooth_band_total=" + _smoothedReconcileTotal + ", " +
                      "render_offset_nonzero_frames_total=" + _renderOffsetNonZeroFrameTotal + ", " +
                      "render_offset_max_m=" + _renderOffsetMaxM.ToString("F4", CultureInfo.InvariantCulture) +
                      " (n=" + _renderOffsetMaxN + "), " +
                      "render_offset_decay_frames_total=" + _renderOffsetDecayFrameTotal);
        }

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
                    // same as GreyboxSession. No batching concern yet - nothing pending to race
                    // against - so the row for it is written immediately.
                    // D-1 (ADR-0012 section 6.4 point 3): seed the local tick index from this
                    // first snapshot's own tick, same as GreyboxSession.
                    _controller = new PredictedShipController(_controlledShipClass, CurrentBoundary(), _tickDurationSeconds, confirmed, message.Tick);

                    // Own ship: what THIS observer's screen shows for its own ship right now -
                    // predicted, not the raw wire value (SC-64/65 measure the two observers'
                    // SCREENS, not the two observers' raw snapshots - the latter is SC-63's job
                    // and is intentionally tautological, see two_client_view.py's own comment).
                    // R23: no render offset yet (_renderPositionOffset starts at Vec3d.Zero,
                    // OnSessionReady/Init) - the first snapshot is trusted unconditionally, there
                    // is nothing to smooth on top of.
                    WriteRow(message.Tick, controlledWire.ShipId, controlledWire.Presence,
                        position: _controller.CurrentState.Position, velocity: _controller.CurrentState.Velocity);
                }
                else
                {
                    // F-2 (05_qa_report_r5.md §4(c)): this used to call Reconcile() inline here,
                    // the exact defect H-9 fixed in GreyboxSession - RealtimeClient.Pump() can
                    // drain more than one WORLD_SNAPSHOT for the controlled ship in one Update()
                    // (this session's own Pump() call in Update(), below), and reconciling each
                    // one in arrival order lets an older snapshot's Reconcile() run after a newer
                    // one already landed, rewinding CurrentState - and because WriteRow() below
                    // fed straight off CurrentState, that rewind used to be etched into the QA
                    // CSV evidence file itself. Queue it instead; ApplyPendingRebase() (called
                    // once per Update(), right after Pump()) reconciles and writes exactly once,
                    // against whichever snapshot in the batch had the highest tick - identical
                    // selection rule to GreyboxSession's OnWorldSnapshotCore, via the same pure
                    // SnapshotRebaseBatch.ShouldReplacePending decision (SnapshotRebaseBatchTests.cs).
                    _pendingRebase.TryQueue(message.Tick, confirmed, payload.AckInputSeq,
                                            controlledWire.ShipId, controlledWire.Presence);
                }
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
                // R23: remote ship rows are already pure presentation values (interpolated) -
                // no render offset of their own to add (architect R10 후속 판정 §1).
                WriteRow(message.Tick, pair.Key, display.Presence, display.State.Position, display.State.Velocity);
            }
        }

        // ------------------------------------------------------------------ F-2 / H-9-style batched rebase (see the queuing comment in OnWorldSnapshot)

        // qa r6 F-2: was five loose fields plus an inline copy of the take-and-clear sequence,
        // none of it reachable by any test. PendingRebaseSlot is the same state machine as a
        // testable object - see that file's header.
        readonly PendingRebaseSlot _pendingRebase = new PendingRebaseSlot();

        // F-21 (team-lead R18): same drift-event path as GreyboxSession (ReconcileTickDrift.cs /
        // ReconcileTickDriftEvent.cs) - this session has its own independent ack/tick baseline
        // since it runs its own socket (SC-64/65's two-observer harness). Session-scope, reset in
        // OnSessionReady.
        uint? _driftPrevAck;
        long _driftPrevTick;

        /// <summary>Applies at most one rebase/reconcile + CSV row per frame, against the latest
        /// WORLD_SNAPSHOT this frame's Pump() drain produced for the controlled ship (see the
        /// queuing comment in OnWorldSnapshot). No-op when nothing is pending - the common case
        /// of exactly one (or zero) snapshot per frame.</summary>
        void ApplyPendingRebase()
        {
            // R23: reset every frame BEFORE any early return - see _reconciledThisFrame's doc comment.
            _reconciledThisFrame = false;

            if (!_pendingRebase.TryTake(out long tick, out ShipSimState confirmed,
                                        out uint? ackInputSeq, out Guid shipId, out string presence)) return;

            // F-21: measure BEFORE Reconcile, same order/reasoning as GreyboxSession's C-1 site.
            long? drift = ReconcileTickDrift.Compute(_driftPrevAck, _driftPrevTick, ackInputSeq, tick);
            long deltaTickForEvent = tick - _driftPrevTick; // R19: see GreyboxSession's identical comment
            _driftPrevAck = ackInputSeq;
            _driftPrevTick = tick;

            // R23 (architect R10 후속 판정 §1): what this observer's SCREEN shows for its own
            // ship just before this correction - sim position/orientation BEFORE Reconcile()
            // overwrites them, plus whatever render offset was still decaying from the last one.
            // Same capture GreyboxSession does before its own Reconcile() call.
            Vec3d renderPositionBeforeCorrection = _controller.CurrentState.Position + _renderPositionOffset;
            Quatd renderOrientationBeforeCorrection = _renderOrientationOffset * _controller.CurrentState.Orientation;

            // D-1 (ADR-0012 section 6.4 point 2): keyed by tick now, not ackInputSeq - the queued
            // ackInputSeq is retained on PendingRebaseSlot for diagnostics only (unchanged shape).
            SyncTuningData tuningUsed = _data.Tuning ?? DefaultTuning();
            Reconciliation.Result result = _controller.Reconcile(confirmed, tick, tuningUsed);
            _reconciledThisFrame = true; // R23: a real reconcile happened this Update()

            // R23: classify AFTER Reconcile() (needs result.PositionErrorM/OrientationErrorDeg),
            // only when HasError - a comparison-less reconcile has no error to classify, so it is
            // architecturally impossible for one to land in a smoothed band (same reasoning as
            // GreyboxSession - this is what keeps F-27 jumps from ever being smoothed).
            ReconcileBand positionBand = ReconcileBand.HardSnap; // sentinel "not classified" - IsSmoothed() false, same as a real HardSnap
            ReconcileBand orientationBand = ReconcileBand.HardSnap;
            if (result.HasError)
            {
                positionBand = RenderOffset.ClassifyPosition(result.PositionErrorM, tuningUsed);
                orientationBand = RenderOffset.ClassifyOrientation(result.OrientationErrorDeg, tuningUsed);
                if (RenderOffset.IsSmoothed(positionBand) || RenderOffset.IsSmoothed(orientationBand))
                    _smoothedReconcileTotal++;
            }
            _renderPositionOffset = RenderSmoothing.ComputePositionOffset(positionBand, renderPositionBeforeCorrection, result.State.Position);
            _renderOrientationOffset = RenderSmoothing.ComputeOrientationOffset(orientationBand, renderOrientationBeforeCorrection, result.State.Orientation);

            // R19 (architect R9): _input is null for a non-interactive observer (IsInteractive
            // false, SC-62's "B의 함선은 스폰 위치 근처에 머문다") - 0 in that case, same
            // fallback GreyboxSession uses when its sampler has not been created yet.
            // R21 (architect R10 판정 §1.4 table row 4): rebase_jump_m/behind_ticks ride along on
            // this line too, always real numbers (never n/a).
            ReconcileTickDriftEvent? driftEvent = ReconcileTickDriftEvent.TryCreate(
                drift, deltaTickForEvent, tick, result.State.Velocity.Length(), result.HasError,
                result.PositionErrorM, result.OrientationErrorDeg,
                thrustX: _input != null ? Quantization.QuantizeControlAxis(_input.Thrust.X) : 0,
                thrustY: _input != null ? Quantization.QuantizeControlAxis(_input.Thrust.Y) : 0,
                thrustZ: _input != null ? Quantization.QuantizeControlAxis(_input.Thrust.Z) : 0,
                roll: _input != null ? Quantization.QuantizeControlAxis(_input.Roll) : 0,
                rebaseJumpM: _controller.LastRebaseJumpM, behindTicks: _controller.LastBehindTicks);
            if (driftEvent.HasValue) Debug.Log("starfall.observer." + ObserverLabel + ": " + driftEvent.Value.Format());
            // ^ e.g. "starfall.observer.A: reconcile_tick_drift_event drift=-1 tick=... " -
            // module prefix stays consistent with every other line this session logs.

            // SC-56 (c4) (architect R10 판정 §1.4): same gap-filling event as GreyboxSession's
            // site - this session runs its own independent controller/reconcile path (own
            // socket). Gated on behind_ticks, not HasError.
            bool unexplainedThisReconcile = ReconcileRebaseJumpBudget.IsUnexplained(
                _controller.LastRebaseJumpM, _controller.LastExplainedM, tuningUsed.ReconcileHardSnapThresholdM);
            ReconcileClientBehindEvent? clientBehindEvent = ReconcileClientBehindEvent.TryCreate(
                _controller.LastBehindTicks, _controller.LastRebaseJumpM, tick, deltaTickForEvent,
                result.State.Velocity.Length(), _controller.LastExplainedM, unexplainedThisReconcile);
            if (clientBehindEvent.HasValue) Debug.Log("starfall.observer." + ObserverLabel + ": " + clientBehindEvent.Value.Format());

            // F-29 (qa r10 §H, team-lead R21): same as GreyboxSession's site - reuses the same
            // tuning constants already in scope for this Reconcile() call.
            ReconcileErrorThresholdEvent? errorThresholdEvent = ReconcileErrorThresholdEvent.TryCreate(
                result.HasError, tick, result.State.Velocity.Length(),
                result.PositionErrorM, result.OrientationErrorDeg,
                tuningUsed.ReconcileSmoothThresholdM, tuningUsed.ReconcileOrientationSmoothThresholdDeg);
            if (errorThresholdEvent.HasValue) Debug.Log("starfall.observer." + ObserverLabel + ": " + errorThresholdEvent.Value.Format());

            // R23 (architect R10 후속 판정 §1.1): the own-ship row is written at the reconcile
            // instant, BEFORE this frame's Update()-driven decay runs (below) - the offset here
            // is the freshly computed, undecayed value, i.e. the PEAK of the smoothing curve for
            // this correction. Documented, not a bug: "이 CSV는 평활화 곡선의 꼭대기만 표집하며
            // ... 보수적 상한 읽기다" (architect §1.1).
            WriteRow(tick, shipId, presence,
                position: _controller.CurrentState.Position + _renderPositionOffset,
                velocity: _controller.CurrentState.Velocity,
                renderOffsetM: _renderPositionOffset.Length(),
                renderOffsetDeg: Quatd.AngleDegrees(_renderOrientationOffset, Quatd.Identity));
        }

        /// <summary>R23: <paramref name="renderOffsetM"/>/<paramref name="renderOffsetDeg"/>
        /// default to 0 - every call site except the own-ship row above (which passes the
        /// measured offset explicitly) is a remote ship's already-pure-presentation row, which
        /// has no offset of its own (architect R10 후속 판정 §1: "타 함선 행은 그대로다").
        /// <paramref name="position"/> is expected to ALREADY have any render offset added
        /// (quantised here, after adding - never the reverse, since the offset is mm-scale and
        /// would be lost to rounding if added post-quantisation).</summary>
        void WriteRow(long tick, Guid shipId, string presence, Vec3d position, Vec3d velocity,
            double renderOffsetM = 0.0, double renderOffsetDeg = 0.0)
        {
            _csv.Write(new ObserverCsvRow(
                tick, _ownActorId, shipId, presence,
                Quantization.QuantizePosition(position.X),
                Quantization.QuantizePosition(position.Y),
                Quantization.QuantizePosition(position.Z),
                Quantization.QuantizeVelocity(velocity.X),
                Quantization.QuantizeVelocity(velocity.Y),
                Quantization.QuantizeVelocity(velocity.Z),
                renderOffsetMm: Quantization.QuantizePosition(renderOffsetM),
                renderOffsetDeg: renderOffsetDeg));
        }

        static SyncTuningData DefaultTuning() => new SyncTuningData
        {
            ReconcileIgnoreThresholdM = 0.005,
            ReconcileSmoothThresholdM = 0.25,
            ReconcileSmoothDurationMs = 200,
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

            // F-2: apply at most one rebase/reconcile for whatever WORLD_SNAPSHOT batch the
            // Pump() call just above produced this frame - see ApplyPendingRebase()'s doc
            // comment. Must run after Pump() (unlike GreyboxSession, whose Pump() lives on a
            // separate component/Update()), since this is the call that populates the pending
            // state for this frame.
            ApplyPendingRebase();

            // R23 (architect R10 후속 판정 §1, "⚠ 감쇠 함정"): GreyboxSession decays inside
            // RenderLocalShip() because it draws every frame - Observer has no such call, so the
            // decay must be EXPLICIT here or the offset freezes at whatever ApplyPendingRebase()
            // last set it to. A frozen offset would not show up as a visual bug (nothing is
            // drawn) - it would only show up as silently wrong render_offset_mm/px_mm values in
            // the CSV, which is exactly the failure mode this call exists to prevent.
            DecayRenderOffset();

            if (_input != null) _input.Sample();

            _tickAccumulator += Time.unscaledDeltaTime;
            while (_tickAccumulator >= _tickDurationSeconds)
            {
                _tickAccumulator -= _tickDurationSeconds;
                SendAndPredictOneTick();
            }
        }

        /// <summary>R23: per-frame half of F-33 render smoothing for this observer's own ship -
        /// see the field block's doc comments and Update()'s call site comment for why this must
        /// be explicit (Observer has no RenderLocalShip()-equivalent to hide it inside).</summary>
        void DecayRenderOffset()
        {
            double durationMs = _data.Tuning?.ReconcileSmoothDurationMs ?? 200.0;
            double dt = Time.unscaledDeltaTime;
            double offsetBeforeDecayM = _renderPositionOffset.Length();
            _renderPositionOffset = RenderSmoothing.DecayPositionOffset(_renderPositionOffset, dt, durationMs);
            _renderOrientationOffset = RenderSmoothing.DecayOrientationOffset(_renderOrientationOffset, dt, durationMs);

            // SC-56 (e) items 2/3/4 - same sampling discipline as GreyboxSession.RenderLocalShip().
            double offsetAfterDecayM = _renderPositionOffset.Length();
            _renderOffsetMaxN++;
            if (offsetAfterDecayM > _renderOffsetMaxM) _renderOffsetMaxM = offsetAfterDecayM;
            if (offsetAfterDecayM > 0.0) _renderOffsetNonZeroFrameTotal++;
            if (!_reconciledThisFrame && offsetAfterDecayM < offsetBeforeDecayM) _renderOffsetDecayFrameTotal++;
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
