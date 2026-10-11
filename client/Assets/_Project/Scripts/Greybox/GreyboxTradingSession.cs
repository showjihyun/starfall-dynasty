// Hand-written. C3 (p1-03-trading) - the greybox connection point GreyboxSession.Awake() creates,
// the same pattern as GreyboxMiningSession (separate component/file, own Update()/OnGUI(), reads
// the host session's RealtimeClient and ship-state provider through Init()'s parameters rather
// than a GreyboxSession reference, so PlayMode tests can drive it against a bare RealtimeClient +
// FakeRealtimeTransport without building a full scene).
//
// Message-handler ownership (Net.RealtimeClient.Register is a single-slot dictionary, NOT a
// multi-subscriber event - a second Register call for an already-registered type SILENTLY
// REPLACES the first): WALLET_STATE, MARKET_STATE and SALE_RECEIPT belong to this component alone
// (nobody else registers them). INVENTORY_STATE and HISTORICAL_EVENT_NOTICE are already owned by
// GreyboxMiningSession (p1-02) - this component reads their already-applied state through
// GreyboxMiningSession's own test-named-but-production-usable accessors (InventoryForTests,
// DiscoveriesForTests) instead of registering a second handler that would silently clobber
// Mining's. CommandResultReceived IS a plain C# event (+=), so this component's own handler
// coexists safely with GreyboxSession's and GreyboxMiningSession's.
//
// Key moved off the Mining key (G) and ShipInputSampler's W/S/A/D/R/F/Q/E/X/Z/H set entirely:
// UpArrow/DownArrow select a market panel row, LeftArrow/RightArrow switch the quantity preset,
// Enter sends SELL_MINERAL - and ALL FOUR make a command/selection change ONLY while the panel is
// open (design doc section 8.3 UI-07/08, sprint contract AC-17/SC-104: "닫혀 있을 때 Enter ->
// 송신 0").

using System;
using System.Collections.Generic;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Mining;
using Starfall.Net;
using Starfall.Sim;
using Starfall.Trading;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Starfall.Greybox
{
    [DisallowMultipleComponent]
    public sealed class GreyboxTradingSession : MonoBehaviour
    {
        const float NoticeHoldSeconds = 4f; // UI-09 "4 s 유지(가정)"
        const float NewListingNoticeHoldSeconds = 6f; // UI-10 "6 s 유지" (discovery banner와 같은 길이)

        RealtimeClient _client;
        Func<ShipSimState?> _controlledShipStateProvider;
        Func<DiscoveryFeedState> _discoveriesProvider;
        Func<IReadOnlyList<InventoryItemView>> _inventoryProvider;
        Func<Camera> _cameraProvider;
        Func<TradingKeyInput> _keyInputProvider;
        GreyboxTradingData _data;

        readonly WalletMirrorState _wallet = new WalletMirrorState();
        readonly MarketMirrorState _market = new MarketMirrorState();
        readonly TradingCommandGateway _gateway = new TradingCommandGateway();

        int _selectedRowIndex;
        bool _useAllPreset; // false = "이 값까지"(있으면), true = "전량"
        long _expectedProceedsAtSendCr;

        string _lastRejectSentence;
        string _saleNoticeFirstLine;
        string _saleNoticeSecondLine;
        float _saleNoticeHideAtRealTime;
        string _newListingNotice;
        float _newListingNoticeHideAtRealTime;

        /// <param name="marketId">"cradle-exchange" for the vertical slice.</param>
        /// <param name="cameraProvider">C3 (SC-67/UI-02): polled for the off-screen indicator's
        /// viewport projection of the marker. May be null (the indicator is simply skipped - a
        /// test that never builds a scene camera, same convention as GreyboxMiningSession's own
        /// cameraProvider).</param>
        /// <param name="keyInputProvider">C3 (qa3 Q-4/SC-104): the per-frame key state Update()
        /// acts on. Defaults to reading the real Keyboard device when null is passed AND a
        /// Keyboard is actually present; a PlayMode test with no live Keyboard device supplies
        /// its own delegate instead of needing the InputSystem test-tools package.</param>
        public void Init(RealtimeClient client, string marketId,
            Func<ShipSimState?> controlledShipStateProvider = null,
            Func<DiscoveryFeedState> discoveriesProvider = null,
            Func<IReadOnlyList<InventoryItemView>> inventoryProvider = null,
            Func<Camera> cameraProvider = null,
            Func<TradingKeyInput> keyInputProvider = null,
            GreyboxTradingData? dataOverride = null)
        {
            _client = client ?? throw new ArgumentNullException(nameof(client));
            _controlledShipStateProvider = controlledShipStateProvider;
            _discoveriesProvider = discoveriesProvider;
            _inventoryProvider = inventoryProvider;
            _cameraProvider = cameraProvider;
            _keyInputProvider = keyInputProvider ?? ReadRealKeyboard;
            _data = dataOverride ?? GreyboxDataLoader.LoadTrading("starfall.trading: ", marketId);

            _client.Register<WalletStateMessage>("WALLET_STATE", OnWalletState);
            _client.Register<MarketStateMessage>("MARKET_STATE", OnMarketState);
            _client.Register<SaleReceiptMessage>("SALE_RECEIPT", OnSaleReceipt);
            _client.CommandResultReceived += OnCommandResultReceived;

            BuildExchangeMarker();
        }

        void OnDestroy()
        {
            if (_client != null) _client.CommandResultReceived -= OnCommandResultReceived;
        }

        // ------------------------------------------------------------------ UI-01: 3D marker

        Transform _markerTransform;
        Renderer _dockingZoneRenderer; // UI-01: "도킹 구역 구는 ... 둘 다 초록이면 색이 바뀐다" - greybox stand-in uses the same marker renderer (no separate wire sphere mesh)

        void BuildExchangeMarker()
        {
            if (_data.Market == null) return;
            GameObject go = GameObject.CreatePrimitive(PrimitiveType.Cylinder); // ring+pillar silhouette stand-in (greybox) - distinct from Mining's cubes and GreyboxSession's reference spheres
            go.name = "Exchange_" + _data.Market.Id;
            go.transform.position = ToUnity(_data.Market.PositionM);
            float diameter = (float)_data.Market.RadiusM * 2.0f;
            go.transform.localScale = new Vector3(diameter, diameter * 0.3f, diameter);
            Collider collider = go.GetComponent<Collider>();
            if (collider != null) Destroy(collider);
            Renderer renderer = go.GetComponent<Renderer>();
            if (renderer != null && renderer.material != null)
                renderer.material.color = Color.yellow; // design doc UI-01: "노란색"
            _markerTransform = go.transform;
            _dockingZoneRenderer = renderer;

            Debug.Log("starfall.trading: exchange_marker built id=" + _data.Market.Id);
        }

        /// <summary>UI-01 colour rule (ExchangeMarkerColorRule, C2) applied to the greybox stand-in
        /// renderer every frame - cyan when both docking conditions are green, back to the base
        /// yellow otherwise.</summary>
        void UpdateDockingZoneColor(DockingRangeStatus docking)
        {
            if (_dockingZoneRenderer == null || _dockingZoneRenderer.material == null) return;
            DockingZoneColor color = ExchangeMarkerColorRule.Resolve(docking);
            _dockingZoneRenderer.material.color = color == DockingZoneColor.BothConditionsGreen ? Color.cyan : Color.yellow;
        }

        // ------------------------------------------------------------------ message handlers

        void OnWalletState(WalletStateMessage message) => _wallet.ApplyWalletState(message);

        void OnMarketState(MarketStateMessage message)
        {
            // Apply FIRST, then read PreviousMineralIds - it reports "the ids before THIS call"
            // only once ApplyMarketState has run (MarketMirrorState's own contract/doc comment).
            _market.ApplyMarketState(message);

            var currentIds = new List<string>();
            foreach (MarketListingView listing in _market.Listings) currentIds.Add(listing.MineralId);

            IReadOnlyList<string> newListings = NewListingDetector.Detect(_market.PreviousMineralIds, currentIds);
            if (newListings.Count > 0)
            {
                _newListingNotice = "새 상장: " + string.Join(", ", newListings);
                _newListingNoticeHideAtRealTime = Time.realtimeSinceStartup + NewListingNoticeHoldSeconds;
            }
        }

        void OnSaleReceipt(SaleReceiptMessage message)
        {
            var payload = message.Payload;
            string mineralName = ResolveMineralDisplayName(payload.MineralId);
            // find the quantity from the sale: SALE_RECEIPT does not carry quantity_before/after,
            // so this uses the gateway's last-sent quantity (recorded at MarkSent) as the display
            // quantity - the actual accepted sale always matches what was just sent (same
            // command_id round trip).
            var notice = SaleResultNoticeBuilder.Build(mineralName, _lastSentQuantityKg, payload.PriceAfterCrPerKg, payload.ProceedsCr, _expectedProceedsAtSendCr);
            _saleNoticeFirstLine = notice.FirstLine;
            _saleNoticeSecondLine = notice.SecondLine;
            _saleNoticeHideAtRealTime = Time.realtimeSinceStartup + NoticeHoldSeconds;
        }

        void OnCommandResultReceived(CommandResultMessage message)
        {
            Guid commandId = message.Payload.CommandId;
            if (!_gateway.TryResolve(commandId)) return; // not this session's pending SELL_MINERAL

            if (string.Equals(message.Payload.Status, "ACCEPTED", StringComparison.Ordinal))
            {
                _lastRejectSentence = null; // SALE_RECEIPT (above) carries the actual result
            }
            else
            {
                _lastRejectSentence = TradingRejectReasonText.Resolve(message.Payload.ReasonCode);
                Debug.Log("starfall.trading: SELL_MINERAL rejected - " + _lastRejectSentence);
            }
        }

        // ------------------------------------------------------------------ input (UI-07/08: ↑↓←→ Enter, panel open only)

        int _lastSentQuantityKg;

        /// <summary>Default _keyInputProvider - reads the real Keyboard device. Returns
        /// TradingKeyInput.None (nothing pressed) when no Keyboard is present, rather than null,
        /// so Update() never needs its own separate "no device" branch.</summary>
        static TradingKeyInput ReadRealKeyboard()
        {
            if (Keyboard.current == null) return TradingKeyInput.None;
            return new TradingKeyInput(
                Keyboard.current.upArrowKey.wasPressedThisFrame,
                Keyboard.current.downArrowKey.wasPressedThisFrame,
                Keyboard.current.leftArrowKey.wasPressedThisFrame,
                Keyboard.current.rightArrowKey.wasPressedThisFrame,
                Keyboard.current.enterKey.wasPressedThisFrame);
        }

        void Update()
        {
            if (_data.Market == null) return;

            DockingRangeStatus docking = ComputeDockingStatus();
            UpdateDockingZoneColor(docking); // UI-01: independent of key handling below - must update even with no Keyboard device

            bool panelOpen = docking.BothGreen;
            if (!panelOpen) return; // AC-17/SC-104: no key makes a command or selection change while the panel is closed

            SellPanelRow[] rows = BuildRows();
            if (rows.Length == 0) return;

            TradingKeyInput keys = _keyInputProvider();

            if (keys.UpPressed)
                _selectedRowIndex = SellPanelSelection.MoveUp(_selectedRowIndex, rows.Length);
            if (keys.DownPressed)
                _selectedRowIndex = SellPanelSelection.MoveDown(_selectedRowIndex, rows.Length);
            _selectedRowIndex = Mathf.Clamp(_selectedRowIndex, 0, rows.Length - 1);

            SellPanelRow selected = rows[_selectedRowIndex];
            MarketListingData listing = _data.Market.FindListing(selected.MineralId);
            if (listing == null) return;

            SellQuantityPresets presets = SellPanelCalculator.ComputePresets(
                selected.SaturationKg, selected.HoldingKg, listing, _data.Market.PriceFloorPct, _data.Market.PriceDropPctPerBand);

            if (keys.LeftPressed || keys.RightPressed)
                _useAllPreset = !_useAllPreset || !presets.HasDistinctUpToBandPreset;

            if (keys.EnterPressed)
                TrySendSell(selected, presets);
        }

        void TrySendSell(SellPanelRow row, SellQuantityPresets presets)
        {
            if (!presets.CanSell) return; // "팔 것 없음" - Enter is a no-op

            bool usingAll = _useAllPreset || !presets.HasDistinctUpToBandPreset;
            int quantityKg = usingAll ? presets.AllKg : presets.UpToBandKg;
            long expectedProceedsCr = usingAll ? presets.ExpectedProceedsAllCr : presets.ExpectedProceedsUpToBandCr;

            Guid commandId = _gateway.ResolveCommandIdFor(_data.Market.Id, row.MineralId, quantityKg);
            var command = new SellMineralCommand
            {
                CommandId = commandId,
                Payload = new SellMineralCommand.SellMineralPayload
                {
                    MarketId = _data.Market.Id,
                    MineralId = row.MineralId,
                    QuantityKg = quantityKg,
                },
            };
            string json = ContractJson.Serialize(command);
            if (_client.TrySendJson(json))
            {
                _gateway.MarkSent(commandId, _data.Market.Id, row.MineralId, quantityKg);
                _lastSentQuantityKg = quantityKg;
                _expectedProceedsAtSendCr = expectedProceedsCr;
                _lastRejectSentence = null; // design doc / p1-02 convention: next attempt clears the stale rejection
            }
        }

        // ------------------------------------------------------------------ display helpers

        DockingRangeStatus ComputeDockingStatus()
        {
            ShipSimState? shipState = _controlledShipStateProvider != null ? _controlledShipStateProvider() : null;
            if (!shipState.HasValue || _data.Market == null) return default;

            double distance = (_data.Market.PositionM - shipState.Value.Position).Length();
            double speed = shipState.Value.Velocity.Length();
            return DockingRangeEvaluator.Evaluate(distance, _data.Market.RadiusM, speed, _data.Market.DockRangeFromSurfaceM, _data.Market.MaxShipSpeedMps);
        }

        /// <summary>UI-02 (SC-67): is the marker off-screen or behind the camera, from the
        /// camera provider's viewport projection (ExchangeDistanceLabel.IsOffScreen, C2). False
        /// when there is no camera or no market data - the indicator is simply skipped, same
        /// convention as GreyboxMiningSession's own cameraProvider.</summary>
        bool ComputeIsOffScreen()
        {
            Camera camera = _cameraProvider != null ? _cameraProvider() : null;
            if (camera == null || _data.Market == null) return false;
            Vector3 viewport = camera.WorldToViewportPoint(ToUnity(_data.Market.PositionM));
            return ExchangeDistanceLabel.IsOffScreen(viewport.x, viewport.y, viewport.z);
        }

        SellPanelRow[] BuildRows()
        {
            IReadOnlyList<InventoryItemView> inventory = _inventoryProvider != null ? _inventoryProvider() : null;
            var rows = new List<SellPanelRow>();
            foreach (MarketListingView listing in _market.Listings)
            {
                int holding = 0;
                if (inventory != null)
                    foreach (InventoryItemView item in inventory)
                        if (item.MineralId == listing.MineralId) { holding = item.QuantityKg; break; }

                MarketListingData constants = _data.Market?.FindListing(listing.MineralId);
                int cap = constants != null
                    ? PriceRule.SaturationCap(PriceRule.FloorBand(_data.Market.PriceFloorPct, _data.Market.PriceDropPctPerBand), constants.BandKg)
                    : 0;

                rows.Add(new SellPanelRow(listing.MineralId, ResolveMineralDisplayName(listing.MineralId),
                    listing.PriceCrPerKg, listing.SaturationKg, cap, holding));
            }
            return rows.ToArray();
        }

        string ResolveMineralDisplayName(string mineralId) => mineralId; // greybox: no mineral display-name catalog wired into this component yet; raw id is acceptable for this slice's HUD

        static Vector3 ToUnity(Vec3d v) => new Vector3((float)v.X, (float)v.Y, (float)v.Z);

        // ------------------------------------------------------------------ HUD (greybox: plain text, same style as GreyboxSession.OnGUI)

        void OnGUI()
        {
            if (GUI.skin == null || _data.Market == null) return;

            DockingRangeStatus docking = ComputeDockingStatus();
            var lines = new List<string> { "-- trading (UpArrow/DownArrow select, LeftArrow/RightArrow preset, Enter sell) --" };
            lines.Add(WalletLabel.Format(_wallet.BalanceCr));

            if (_market.Listings.Count == 0)
                lines.Add(TickerFormat.NoListingsLine);
            foreach (MarketListingView listing in _market.Listings)
            {
                MarketListingData constants = _data.Market.FindListing(listing.MineralId);
                if (constants != null)
                    lines.Add(TickerFormat.Line(ResolveMineralDisplayName(listing.MineralId), listing.PriceCrPerKg, constants.BasePriceCrPerKg));

                string tag = DiscovererTagLine(listing.MineralId);
                if (tag != null) lines.Add("  " + tag);

                double elapsedSeconds = listing.LastSaleActorId.HasValue
                    ? Math.Max(0, _market.CurrentTick - listing.LastSaleTick) / (double)(_client.Session?.TickHz ?? 20)
                    : 0;
                Guid myActorId = _client.Session?.ActorId ?? default;
                lines.Add("  " + LastSaleLine.Format(listing.LastSaleActorId, listing.LastSaleQuantityKg, elapsedSeconds, myActorId));
            }
            int unlistedCount = (_data.Market.Listings?.Count ?? 0) - _market.Listings.Count;
            if (unlistedCount > 0) lines.Add(TickerFormat.UnlistedCountLine(_data.Market.Listings.Count, _market.Listings.Count));

            // UI-02: distance label is ALWAYS visible (design doc: "언제나 보인다" - unlike UI-03's
            // docking HUD, which gates on the 2 km Visible flag below).
            lines.Add(ExchangeDistanceLabel.Format(_data.Market.DisplayName ?? _data.Market.Id, docking.SurfaceDistanceM));
            if (ComputeIsOffScreen())
                lines.Add("→ " + ExchangeDistanceLabel.Format(_data.Market.DisplayName ?? _data.Market.Id, docking.SurfaceDistanceM));

            if (docking.Visible)
            {
                lines.Add("도킹: 거리 " + (docking.WithinDockingZone ? "[OK]" : "[NG]") +
                          " " + docking.SurfaceDistanceM.ToString("F0") + " / " + _data.Market.DockRangeFromSurfaceM.ToString("F0") + " m" +
                          " · 속도 " + (docking.SpeedOk ? "[OK]" : "[NG]"));
            }

            if (docking.BothGreen)
            {
                SellPanelRow[] rows = BuildRows();
                for (int i = 0; i < rows.Length; i++)
                {
                    SellPanelRow row = rows[i];
                    string marker = i == _selectedRowIndex ? "> " : "  ";
                    MarketListingData constants = _data.Market.FindListing(row.MineralId);
                    string bandPhrase = "";
                    string presetLine = "";
                    if (constants != null)
                    {
                        int amountToNextBand = PriceRule.AmountToNextBand(row.SaturationKg, constants, _data.Market.PriceFloorPct, _data.Market.PriceDropPctPerBand);
                        bandPhrase = " (" + SellPanelBandPhrase.Format(amountToNextBand) + ")";

                        SellQuantityPresets presets = SellPanelCalculator.ComputePresets(row.SaturationKg, row.HoldingKg, constants, _data.Market.PriceFloorPct, _data.Market.PriceDropPctPerBand);
                        if (i == _selectedRowIndex) presetLine = " :: " + SellPanelCalculator.FormatLine(presets);
                    }
                    lines.Add(marker + row.DisplayName + bandPhrase + " 보유 " + row.HoldingKg + " kg" + presetLine);
                }
            }

            if (_lastRejectSentence != null) lines.Add("REJECTED: " + _lastRejectSentence);

            if (Time.realtimeSinceStartup < _saleNoticeHideAtRealTime && _saleNoticeFirstLine != null)
            {
                lines.Add(_saleNoticeFirstLine);
                if (_saleNoticeSecondLine != null) lines.Add(_saleNoticeSecondLine);
            }
            if (Time.realtimeSinceStartup < _newListingNoticeHideAtRealTime && _newListingNotice != null)
                lines.Add(_newListingNotice);

            var rowsText = GreyboxSession.BuildHudRows(lines);
            GUI.Box(new Rect(8, 620, GreyboxSession.HudRowPixelWidth + 16, 20 + rowsText.Count * 18), "");
            for (int i = 0; i < rowsText.Count; i++)
                GUI.Label(new Rect(16, 624 + i * 18, GreyboxSession.HudRowPixelWidth, 18), rowsText[i]);
        }

        string DiscovererTagLine(string mineralId)
        {
            DiscoveryFeedState discoveries = _discoveriesProvider != null ? _discoveriesProvider() : null;
            if (discoveries == null) return null;
            if (!discoveries.TryGet(mineralId, out DiscoveryEntry entry)) return DiscovererTagState.RecordingLine;
            Guid myActorId = _client.Session?.ActorId ?? default;
            return "발견: " + (entry.DiscovererActorId == myActorId ? "나" : TradingPilotTag.From(entry.DiscovererActorId));
        }

        // ------------------------------------------------------------------ test access (EditMode/PlayMode)

        public WalletMirrorState WalletForTests => _wallet;
        public MarketMirrorState MarketForTests => _market;
        public string LastRejectSentenceForTests => _lastRejectSentence;
        public string SaleNoticeFirstLineForTests => _saleNoticeFirstLine;
        public string SaleNoticeSecondLineForTests => _saleNoticeSecondLine;
        public string NewListingNoticeForTests => _newListingNotice;
        public int SelectedRowIndexForTests => _selectedRowIndex;
        public TradingCommandGateway GatewayForTests => _gateway;
        public bool IsOffScreenForTests => ComputeIsOffScreen();
        public string DistanceLabelForTests() =>
            _data.Market == null ? null : ExchangeDistanceLabel.Format(_data.Market.DisplayName ?? _data.Market.Id, ComputeDockingStatus().SurfaceDistanceM);
        public string DiscovererTagForTests(string mineralId) => DiscovererTagLine(mineralId);
    }
}
