// Hand-written. C3 (p1-02-mining) - the greybox connection point GreyboxSession.Awake() creates.
// Owns nothing GreyboxSession already owns (transport, ship prediction); reads them through
// Init()'s parameters and drives Starfall.Mining's pure state machines (InventoryPanelState,
// DepositMarkerState, DiscoveryFeedState, MiningCommandGateway). Kept as a separate component/
// file rather than folded into GreyboxSession's own 1300+ lines - same reasoning as
// ShipInputSampler being its own component.
//
// Init() takes a RealtimeClient and an optional ship-state provider directly - NOT a
// GreyboxSession reference - specifically so PlayMode tests (GreyboxMiningSessionPlayModeTests.cs,
// SC-70/SC-71(B)) can drive it against a bare RealtimeClient + FakeRealtimeTransport without
// constructing a full GreyboxSession (which builds a real scene, camera, ship view and connects
// to the real StarfallNetHost transport - the wrong transport for a test).
//
// Deliberately does NOT read Starfall.Sim.DepositFieldTable's mineral_id or initial_reserve_kg
// (it cannot - that type has no such fields, see DepositFieldTable.cs) and does NOT let a
// MINE_RESOURCE accepted result touch InventoryPanelState - only INVENTORY_STATE does that
// (design doc / principle 1, MiningViewModelTests.cs verifies the state type structurally).
//
// C4 (SC-68 human session, 1st attempt FAIL, 2026-10-04 - see
// _workspace/p1-02-mining/evidence/sc68/notes.md): the design doc §8 six elements were mostly
// backed by pure logic (Starfall.Mining) but never actually drawn - markers were HUD text only
// (no 3D position/distance a human could navigate by), no range/cooldown traffic lights, no
// banner/notice distinction, and E collided with roll (ShipInputSampler.cs). This revision adds:
//   1. 3D cube markers (BuildDepositMarkers) + screen-projected distance labels (OnGUI).
//   2. Nearest-deposit range/speed traffic-light lines (MiningRangeStatus/MiningRangeEvaluator).
//   3. Cooldown countdown since the last ACCEPTED COMMAND_RESULT (CooldownDisplay).
//   4. Yield notice vs discovery banner, with own-discovery vs system-wide banner split
//      (MiningNoticeKind/MiningNoticeClassifier), drawn in a different screen region/style.
//   5. Mining key moved from E to G (ShipInputSampler owns E/Q for roll - see that file).

using System;
using System.Collections.Generic;
using System.Globalization;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Mining;
using Starfall.Net;
using Starfall.Sim;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Starfall.Greybox
{
    [DisallowMultipleComponent]
    public sealed class GreyboxMiningSession : MonoBehaviour
    {
        /// <summary>C4: moved off E (ShipInputSampler.cs:106 owns E/Q for roll - SC-68 1차 FAIL
        /// 기타 항목). G is otherwise unused by ShipInputSampler's W/S/A/D/R/F/Q/E/X/Z set.</summary>
        const Key MineKey = Key.G;

        /// <summary>C4: how long a discovery banner (own or system-wide) stays on screen once
        /// shown - design doc §8 item 5 "일정 시간 유지". Does not affect DiscoveryFeedState or
        /// the discovery list (item 6), only this transient on-screen banner.</summary>
        const float DiscoveryBannerHoldSeconds = 6f;

        RealtimeClient _client;
        Func<ShipSimState?> _controlledShipStateProvider;
        Func<Camera> _cameraProvider;
        GreyboxMiningData _data;

        readonly InventoryPanelState _inventory = new InventoryPanelState();
        readonly DepositMarkerState _markers = new DepositMarkerState();
        DiscoveryFeedState _discoveries;
        readonly MiningCommandGateway _gateway = new MiningCommandGateway();

        /// <summary>Most recent MINE_RESOURCE rejection sentence, or null - cleared on the next
        /// attempt so a stale rejection does not linger forever on screen.</summary>
        string _lastRejectSentence;

        /// <summary>Design doc section 3.3 "두 박자": a yield notice (COMMAND_RESULT accepted +
        /// INVENTORY_STATE) is shown immediately and separately from the discovery banner, which
        /// fires ONLY on a NEW HISTORICAL_EVENT_NOTICE (DiscoveryFeedState.Apply returning true) -
        /// never inferred from an inventory delta (principle 1/2).</summary>
        string _yieldNotice;
        string _discoveryBanner;
        MiningNoticeKind _discoveryBannerKind;
        float _discoveryBannerHideAtRealTime;

        /// <summary>C4: real-clock stamp (Time.realtimeSinceStartup) of the last ACCEPTED
        /// COMMAND_RESULT for our own MINE_RESOURCE - cooldown display base (CooldownDisplay.cs
        /// header: client estimate, display-only, server is still the sole judge).</summary>
        float? _lastAcceptedRealTime;

        /// <param name="controlledShipStateProvider">Polled once per Update() for mining-range
        /// checks; may be null (tests that never call Update(), or a session with no ship yet -
        /// FindNearestDepositIdInRange treats "no state" the same as "no ship").</param>
        /// <param name="dataOverride">Test-only escape hatch (team-lead request, SC-71(B) round 3):
        /// when set, skips GreyboxDataLoader.LoadMining entirely and uses this instead - so a
        /// PlayMode test that needs "the local deposit table disagrees with the incoming message"
        /// can build a DepositFieldTable from an in-memory JSON string (DepositFieldTable.FromJson)
        /// instead of writing to and restoring the real client/Assets/_Project/Data/ copy on disk.
        /// The earlier version of this test wrote the real file and restored it in a finally block
        /// - correct when it runs to completion, but a test killed mid-run (crashed Editor, CLI
        /// timeout) would leave the shared data copy corrupted for every other test and for
        /// SC-72's hash check. Null in every real (non-test) call site.</param>
        /// <param name="cameraProvider">C4: polled for the 3D marker screen-projection labels.
        /// May be null (markers are still built in 3D either way; only the OnGUI screen labels
        /// are skipped without a camera - e.g. a test that never builds a scene camera).</param>
        public void Init(RealtimeClient client, string starSystemId, Func<ShipSimState?> controlledShipStateProvider = null,
            GreyboxMiningData? dataOverride = null, Func<Camera> cameraProvider = null)
        {
            _client = client ?? throw new ArgumentNullException(nameof(client));
            _controlledShipStateProvider = controlledShipStateProvider;
            _cameraProvider = cameraProvider;
            _data = dataOverride ?? GreyboxDataLoader.LoadMining("starfall.mining: ", starSystemId);
            _discoveries = new DiscoveryFeedState(_data.Minerals.Count);

            _client.Register<DepositFieldStateMessage>("DEPOSIT_FIELD_STATE", OnDepositFieldState);
            _client.Register<HistoricalEventNoticeMessage>("HISTORICAL_EVENT_NOTICE", OnHistoricalEventNotice);
            _client.Register<InventoryStateMessage>("INVENTORY_STATE", OnInventoryState);
            _client.CommandResultReceived += OnCommandResultReceived;

            BuildDepositMarkers();
        }

        void OnDestroy()
        {
            if (_client != null) _client.CommandResultReceived -= OnCommandResultReceived;
        }

        // ------------------------------------------------------------------ 3D markers (design doc §8 item 1)

        readonly Dictionary<string, Transform> _markerTransforms = new Dictionary<string, Transform>(StringComparer.Ordinal);

        /// <summary>One cube primitive per deposit, scaled by its radius, deliberately a
        /// different shape from GreyboxSession.BuildMarkers' reference-marker spheres so a human
        /// can tell "deposit" from "reference marker" by silhouette alone (SC-68 1차 FAIL item
        /// 1: markers were HUD text only, no 3D position). Never collides (same reasoning as the
        /// reference markers).</summary>
        void BuildDepositMarkers()
        {
            if (_data.Deposits == null) return;
            var root = new GameObject("DepositMarkers (client-only, never sent, never collided)");

            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
            {
                GameObject go = GameObject.CreatePrimitive(PrimitiveType.Cube);
                go.name = "Deposit_" + deposit.Id;
                go.transform.SetParent(root.transform, false);
                go.transform.position = ToUnity(deposit.PositionM);
                float diameter = (float)deposit.RadiusM * 2.0f;
                go.transform.localScale = new Vector3(diameter, diameter, diameter);
                Collider collider = go.GetComponent<Collider>();
                if (collider != null) Destroy(collider); // deposits never collide (grey box)

                Renderer renderer = go.GetComponent<Renderer>();
                if (renderer != null && renderer.material != null)
                    renderer.material.color = new Color(0.85f, 0.65f, 0.15f); // amber - distinct from the default-grey reference-marker spheres

                _markerTransforms[deposit.Id] = go.transform;
            }

            Debug.Log("starfall.mining: deposit_markers built=" + _data.Deposits.Deposits.Count);
        }

        // ------------------------------------------------------------------ message handlers

        void OnDepositFieldState(DepositFieldStateMessage message) => _markers.ApplyDepositFieldState(message);

        void OnHistoricalEventNotice(HistoricalEventNoticeMessage message)
        {
            bool isNew = _discoveries.Apply(message);
            if (!isNew) return; // LIVE/BACKFILL duplicate of an already-known discovery - no banner (02_client_ack.md C1)

            HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent evt = message.Payload.HistoricalEvent;
            string mineralName = _data.Minerals.TryGet(evt.Payload.MineralId, out MineralStats stats) ? stats.DisplayName : evt.Payload.MineralId;
            _discoveries.TryGet(evt.Payload.MineralId, out DiscoveryEntry entry);
            string pilotTag = PilotTag.From(entry.DiscovererActorId);

            // C4: own discovery vs system-wide banner (design doc §8 item 5 / §3.4) - same
            // HISTORICAL_EVENT_NOTICE fact, two different sentences so they read as distinct from
            // each other and from the small yield notice (SC-68 1차 FAIL item 5).
            // _client.Session is set by SESSION_READY (RealtimeClient.cs) - by the time any
            // HISTORICAL_EVENT_NOTICE can arrive the session is already open, so this is never
            // null in practice; a defensive default(Guid) just means "never matches as own"
            // rather than throwing, which degrades to the (still correct) system-wide banner.
            Guid selfActorId = _client.Session?.ActorId ?? default;
            _discoveryBannerKind = MiningNoticeClassifier.ClassifyDiscovery(entry.DiscovererActorId, selfActorId);
            _discoveryBanner = MiningNoticeClassifier.FormatBanner(_discoveryBannerKind, mineralName, pilotTag, entry.DepositId);
            _discoveryBannerHideAtRealTime = Time.realtimeSinceStartup + DiscoveryBannerHoldSeconds;

            Debug.Log("starfall.mining: DISCOVERY kind=" + _discoveryBannerKind + " " + _discoveryBanner);
        }

        void OnInventoryState(InventoryStateMessage message)
        {
            _inventory.ApplyInventoryState(message);
            _yieldNotice = "인벤토리 갱신: " + message.Payload.Items.Length + "종";
        }

        long? _lastAcceptedTick;

        void OnCommandResultReceived(CommandResultMessage message)
        {
            Guid commandId = message.Payload.CommandId;
            if (!_gateway.TryResolve(commandId)) return; // not this session's pending MINE_RESOURCE

            if (string.Equals(message.Payload.Status, "ACCEPTED", StringComparison.Ordinal))
            {
                _lastAcceptedTick = message.Tick;
                _lastAcceptedRealTime = Time.realtimeSinceStartup; // C4: cooldown display base
                _lastRejectSentence = null;
                // Yield notice fires here too (not only on INVENTORY_STATE) so "accepted" is
                // acknowledged immediately - but the PANEL NUMBERS themselves still wait for
                // INVENTORY_STATE (InventoryPanelState has no other mutator).
                _yieldNotice = "채굴 접수됨 (tick " + message.Tick + ") - 인벤토리 갱신 대기 중";
            }
            else
            {
                _lastRejectSentence = RejectReasonText.Resolve(message.Payload.ReasonCode);
                Debug.Log("starfall.mining: MINE_RESOURCE rejected - " + _lastRejectSentence);
            }
        }

        // ------------------------------------------------------------------ input

        void Update()
        {
            ShipSimState? shipState = _controlledShipStateProvider != null ? _controlledShipStateProvider() : null;
            if (_data.Deposits == null || shipState == null) return;
            if (Keyboard.current == null || !Keyboard.current[MineKey].wasPressedThisFrame) return; // G = mine nearest in-range deposit (moved off E, see MineKey)

            string nearestId = FindNearestDepositIdInRange(shipState.Value);
            if (nearestId == null)
            {
                _lastRejectSentence = "사거리 안에 광맥이 없습니다.";
                return;
            }

            Guid commandId = _gateway.ResolveCommandIdFor(nearestId);
            var command = new MineResourceCommand
            {
                CommandId = commandId,
                Payload = new MineResourceCommand.MineResourcePayload { DepositId = nearestId },
            };
            string json = ContractJson.Serialize(command);
            if (_client.TrySendJson(json)) _gateway.MarkSent(commandId, nearestId);
        }

        string FindNearestDepositIdInRange(ShipSimState ship)
        {
            double rangeM = _data.Rules?.MiningRangeFromSurfaceM ?? 150.0;

            string best = null;
            double bestDist = double.PositiveInfinity;
            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
            {
                double limit = deposit.RadiusM + rangeM;
                double dist = (deposit.PositionM - ship.Position).Length();
                if (dist <= limit && dist < bestDist)
                {
                    bestDist = dist;
                    best = deposit.Id;
                }
            }
            return best;
        }

        /// <summary>The single nearest deposit regardless of range (design doc §8 item 2: "가장
        /// 가까운 광맥까지" - not "가장 가까운 사거리 안 광맥"). Null only when there are no
        /// deposits loaded at all.</summary>
        DepositTableEntry FindNearestDeposit(Vec3d shipPosition, out double distanceToCenterM)
        {
            DepositTableEntry best = null;
            double bestDist = double.PositiveInfinity;
            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
            {
                double dist = (deposit.PositionM - shipPosition).Length();
                if (dist < bestDist)
                {
                    bestDist = dist;
                    best = deposit;
                }
            }
            distanceToCenterM = bestDist;
            return best;
        }

        // ------------------------------------------------------------------ HUD (grey-box: plain text, same style as GreyboxSession.OnGUI)

        void OnGUI()
        {
            if (GUI.skin == null || _data.Deposits == null) return;

            ShipSimState? shipState = _controlledShipStateProvider != null ? _controlledShipStateProvider() : null;

            var lines = new List<string> { "-- mining (" + MineKey + " = mine nearest in-range deposit) --" };

            // Design doc §8 item 2: nearest-deposit range/speed traffic lights.
            if (shipState.HasValue)
            {
                double speed = shipState.Value.Velocity.Length();
                DepositTableEntry nearest = FindNearestDeposit(shipState.Value.Position, out double distanceToCenterM);
                if (nearest != null)
                {
                    double rangeM = _data.Rules?.MiningRangeFromSurfaceM ?? 150.0;
                    double maxSpeedMps = _data.Rules?.MaxShipSpeedMps ?? 10.0;
                    MiningRangeStatus status = MiningRangeEvaluator.Evaluate(distanceToCenterM, nearest.RadiusM, speed, rangeM, maxSpeedMps);
                    lines.Add("nearest: " + nearest.DisplayName + " (" + nearest.Id + ")");
                    lines.AddRange(MiningRangeEvaluator.FormatStatusLines(status, speed));
                    lines.Add(status.ReadyToMine ? "[" + MineKey + "] 채굴 가능" : "[" + MineKey + "] 채굴 불가 (위 조건을 먼저 맞춘다)");
                }
            }

            // Design doc §8 item 3: cooldown countdown since the last accepted MINE_RESOURCE.
            if (_lastAcceptedRealTime.HasValue)
            {
                double cooldownS = _data.Rules?.CooldownS ?? 3.0;
                double elapsed = Time.realtimeSinceStartup - _lastAcceptedRealTime.Value;
                double remaining = CooldownDisplay.RemainingSeconds(cooldownS, elapsed);
                string cooldownLine = CooldownDisplay.FormatOrNull(remaining);
                if (cooldownLine != null) lines.Add(cooldownLine);
            }

            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
                lines.Add(FormatMarkerLine(deposit, shipState));

            lines.Add("inventory: " + FormatInventory());
            // C4: yield notice (small, item 5 first half) kept separate from the discovery
            // banner below - no longer a single "*** ... ***" line mixing both (SC-68 1차 FAIL).
            if (_yieldNotice != null) lines.Add("알림: " + _yieldNotice);
            if (_lastRejectSentence != null) lines.Add("REJECTED: " + _lastRejectSentence);
            lines.Add(_discoveries.FormatSummary(MineralDisplayNames()));

            var rows = GreyboxSession.BuildHudRows(lines);
            // Fixed y=420: placed below GreyboxSession's own HUD box, which grows with its own
            // (much longer) field list. Overlap at very small window heights is a known greybox
            // limitation, not a defect this slice fixes - see 03_client_impl.md "사람이 확인할 항목".
            GUI.Box(new Rect(8, 420, GreyboxSession.HudRowPixelWidth + 16, 20 + rows.Count * 18), "");
            for (int i = 0; i < rows.Count; i++)
                GUI.Label(new Rect(16, 424 + i * 18, GreyboxSession.HudRowPixelWidth, 18), rows[i]);

            DrawDiscoveryBanner();
            DrawDepositScreenLabels(shipState);
        }

        /// <summary>Design doc §8 item 5 second half: the discovery banner is "크게, 다른 색·
        /// 위치" from the small yield notice above - top-center, large font, a color that marks
        /// own vs system-wide discovery, and only while DiscoveryBannerHoldSeconds has not
        /// elapsed (SC-68 1차 FAIL: there was no banner at all, only a HUD text line).</summary>
        void DrawDiscoveryBanner()
        {
            if (_discoveryBanner == null) return;
            if (Time.realtimeSinceStartup > _discoveryBannerHideAtRealTime)
            {
                _discoveryBanner = null;
                return;
            }

            var style = new GUIStyle(GUI.skin.box)
            {
                fontSize = 28,
                fontStyle = FontStyle.Bold,
                alignment = TextAnchor.MiddleCenter,
            };
            // Own discovery: gold. System-wide (someone else's): cyan. Distinct from each other
            // and from the plain-white small HUD text above.
            style.normal.textColor = _discoveryBannerKind == MiningNoticeKind.OwnDiscovery
                ? new Color(1f, 0.85f, 0.2f)
                : new Color(0.3f, 0.9f, 1f);

            float width = Mathf.Min(900f, Screen.width - 32f);
            var rect = new Rect((Screen.width - width) / 2f, 24f, width, 64f);
            GUI.Box(rect, _discoveryBanner, style);
        }

        /// <summary>Design doc §8 item 1's "화면 투영 OnGUI 라벨" - one per deposit, hidden when
        /// the camera has no WorldToScreenPoint for it (behind the camera, point.z <= 0). Reuses
        /// FormatMarkerLine (the same string the text HUD list shows) plus a distance suffix, so
        /// a far-away marker is still readable at a glance (team-lead instruction).</summary>
        void DrawDepositScreenLabels(ShipSimState? shipState)
        {
            Camera camera = _cameraProvider != null ? _cameraProvider() : null;
            if (camera == null) return;

            var style = new GUIStyle(GUI.skin.label) { alignment = TextAnchor.MiddleCenter };

            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
            {
                if (!_markerTransforms.TryGetValue(deposit.Id, out Transform marker)) continue;
                Vector3 screenPoint = camera.WorldToScreenPoint(marker.position);
                if (screenPoint.z <= 0f) continue; // behind the camera - hidden (team-lead instruction)

                double distanceToCenterM = shipState.HasValue
                    ? (deposit.PositionM - shipState.Value.Position).Length()
                    : double.NaN;
                string text = double.IsNaN(distanceToCenterM)
                    ? FormatMarkerLine(deposit, shipState)
                    : FormatMarkerLine(deposit, shipState) + " · " + DistanceLabel.Format(distanceToCenterM);

                // GUI's Y origin is top-left, Camera's screen point Y origin is bottom-left.
                float guiY = Screen.height - screenPoint.y;
                var rect = new Rect(screenPoint.x - 150f, guiY - 10f, 300f, 20f);
                GUI.Label(rect, text, style);
            }
        }

        /// <summary>The HUD line for one deposit - joins the local table entry (position/radius
        /// only), the latest DEPOSIT_FIELD_STATE for it, and any known discoverer. Shared by
        /// OnGUI and tests (GreyboxMiningSessionPlayModeTests.cs, SC-71(B)) so the test asserts on
        /// the SAME code path the screen draws from, not a re-implementation of it.</summary>
        string FormatMarkerLine(DepositTableEntry deposit, ShipSimState? shipState = null)
        {
            DepositFieldStateMessage.DepositFieldStatePayload.DepositState wire = null;
            _markers.TryGetWireState(deposit.Id, out wire);

            string mineralName = null;
            DiscoveryEntry? discovery = null;
            if (wire != null && wire.MineralId != null)
            {
                mineralName = _data.Minerals.TryGet(wire.MineralId, out MineralStats stats) ? stats.DisplayName : null;
                if (_discoveries.TryGet(wire.MineralId, out DiscoveryEntry entry)) discovery = entry;
            }

            if (shipState.HasValue)
            {
                double distanceToCenterM = (deposit.PositionM - shipState.Value.Position).Length();
                return DepositMarkerState.FormatLineWithDistance(deposit, wire, discovery, mineralName, distanceToCenterM);
            }

            return DepositMarkerState.FormatLine(deposit, wire, discovery, mineralName);
        }

        /// <summary>Test access only (SC-71(B)) - the same FormatMarkerLine the HUD draws, looked
        /// up by deposit_id instead of iterated.</summary>
        public string MarkerLineForTests(string depositId)
        {
            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
                if (deposit.Id == depositId) return FormatMarkerLine(deposit);
            throw new ArgumentException("no deposit '" + depositId + "' in the loaded deposit table");
        }

        string FormatInventory()
        {
            if (_inventory.Items.Count == 0) return "(empty)";
            var parts = new List<string>();
            foreach (InventoryItemView item in _inventory.Items)
                parts.Add(item.MineralId + "=" + item.QuantityKg.ToString(CultureInfo.InvariantCulture) + "kg");
            return string.Join(", ", parts);
        }

        Dictionary<string, string> MineralDisplayNames()
        {
            var map = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
            {
                if (_markers.TryGetWireState(deposit.Id, out var wire) && wire.MineralId != null &&
                    _data.Minerals.TryGet(wire.MineralId, out MineralStats stats))
                    map[wire.MineralId] = stats.DisplayName;
            }
            return map;
        }

        // ------------------------------------------------------------------ double (Sim) -> float (Unity) conversion, same rule as GreyboxSession (ADR-0012 section 2).

        static Vector3 ToUnity(Vec3d v) => new Vector3((float)v.X, (float)v.Y, (float)v.Z);

        // ------------------------------------------------------------------ test access (EditMode/PlayMode)

        /// <summary>Exposed for tests only - InventoryPanelState has exactly one mutator
        /// (ApplyInventoryState), so reading this cannot itself change anything.</summary>
        public InventoryPanelState InventoryForTests => _inventory;
        public DiscoveryFeedState DiscoveriesForTests => _discoveries;
        public string LastRejectSentenceForTests => _lastRejectSentence;
        public string DiscoveryBannerForTests => _discoveryBanner;
        public MiningNoticeKind DiscoveryBannerKindForTests => _discoveryBannerKind;
    }
}
