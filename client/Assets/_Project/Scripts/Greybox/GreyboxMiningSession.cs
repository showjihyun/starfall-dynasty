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
        RealtimeClient _client;
        Func<ShipSimState?> _controlledShipStateProvider;
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
        public void Init(RealtimeClient client, string starSystemId, Func<ShipSimState?> controlledShipStateProvider = null,
            GreyboxMiningData? dataOverride = null)
        {
            _client = client ?? throw new ArgumentNullException(nameof(client));
            _controlledShipStateProvider = controlledShipStateProvider;
            _data = dataOverride ?? GreyboxDataLoader.LoadMining("starfall.mining: ", starSystemId);
            _discoveries = new DiscoveryFeedState(_data.Minerals.Count);

            _client.Register<DepositFieldStateMessage>("DEPOSIT_FIELD_STATE", OnDepositFieldState);
            _client.Register<HistoricalEventNoticeMessage>("HISTORICAL_EVENT_NOTICE", OnHistoricalEventNotice);
            _client.Register<InventoryStateMessage>("INVENTORY_STATE", OnInventoryState);
            _client.CommandResultReceived += OnCommandResultReceived;
        }

        void OnDestroy()
        {
            if (_client != null) _client.CommandResultReceived -= OnCommandResultReceived;
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

            long? myTick = _lastAcceptedTick;
            string margin = myTick.HasValue ? " " + DiscoveryTiming.Describe(pilotTag, entry.Tick, myTick.Value) : "";

            _discoveryBanner = mineralName + " 발견! " + pilotTag + " (" + entry.DepositId + ")" + margin;
            Debug.Log("starfall.mining: DISCOVERY " + _discoveryBanner);
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
            if (Keyboard.current == null || !Keyboard.current.eKey.wasPressedThisFrame) return; // E = mine nearest in-range deposit

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

        // ------------------------------------------------------------------ HUD (grey-box: plain text, same style as GreyboxSession.OnGUI)

        void OnGUI()
        {
            if (GUI.skin == null || _data.Deposits == null) return;

            var lines = new List<string> { "-- mining (E = mine nearest in-range deposit) --" };

            foreach (DepositTableEntry deposit in _data.Deposits.Deposits)
                lines.Add(FormatMarkerLine(deposit));

            lines.Add("inventory: " + FormatInventory());
            if (_yieldNotice != null) lines.Add(_yieldNotice);
            if (_lastRejectSentence != null) lines.Add("REJECTED: " + _lastRejectSentence);
            if (_discoveryBanner != null) lines.Add("*** " + _discoveryBanner + " ***");
            lines.Add(_discoveries.FormatSummary(MineralDisplayNames()));

            var rows = GreyboxSession.BuildHudRows(lines);
            // Fixed y=420: placed below GreyboxSession's own HUD box, which grows with its own
            // (much longer) field list. Overlap at very small window heights is a known greybox
            // limitation, not a defect this slice fixes - see 03_client_impl.md "사람이 확인할 항목".
            GUI.Box(new Rect(8, 420, GreyboxSession.HudRowPixelWidth + 16, 20 + rows.Count * 18), "");
            for (int i = 0; i < rows.Count; i++)
                GUI.Label(new Rect(16, 424 + i * 18, GreyboxSession.HudRowPixelWidth, 18), rows[i]);
        }

        /// <summary>The HUD line for one deposit - joins the local table entry (position/radius
        /// only), the latest DEPOSIT_FIELD_STATE for it, and any known discoverer. Shared by
        /// OnGUI and tests (GreyboxMiningSessionPlayModeTests.cs, SC-71(B)) so the test asserts on
        /// the SAME code path the screen draws from, not a re-implementation of it.</summary>
        string FormatMarkerLine(DepositTableEntry deposit)
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

        // ------------------------------------------------------------------ test access (EditMode/PlayMode)

        /// <summary>Exposed for tests only - InventoryPanelState has exactly one mutator
        /// (ApplyInventoryState), so reading this cannot itself change anything.</summary>
        public InventoryPanelState InventoryForTests => _inventory;
        public DiscoveryFeedState DiscoveriesForTests => _discoveries;
        public string LastRejectSentenceForTests => _lastRejectSentence;
        public string DiscoveryBannerForTests => _discoveryBanner;
    }
}
