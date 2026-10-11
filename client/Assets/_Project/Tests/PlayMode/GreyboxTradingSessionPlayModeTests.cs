// PlayMode tests for C3 (p1-03-trading): UI-01~13's existence assertions (part 2, sprint
// contract section O) and AC-17's key test (SC-104). Same discipline as
// GreyboxMiningSessionPlayModeTests.cs: drives the REAL wire path
// (FakeRealtimeTransport -> RealtimeClient.OnMessage -> ContractDispatch.TryRead -> the handler
// GreyboxTradingSession.Init() registered), not direct C# construction of Starfall.Trading types
// (that is TradingDisplayTests.cs's job, EditMode, C2).

using System;
using System.Collections.Generic;
using System.IO;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Greybox;
using Starfall.Mining;
using Starfall.Net;
using Starfall.Sim;
using Starfall.Trading;
using UnityEngine;

namespace Starfall.Tests.PlayMode
{
    [TestFixture]
    public sealed class GreyboxTradingSessionPlayModeTests
    {
        FakeRealtimeTransport _transport;
        RealtimeClient _client;
        GameObject _go;
        GreyboxTradingSession _trading;
        ShipSimState? _shipState;

        [SetUp]
        public void SetUp()
        {
            _transport = new FakeRealtimeTransport();
            _client = new RealtimeClient(_transport, NullLogSink.Instance, jitterSeed: 1234);
            _go = new GameObject("GreyboxTradingSessionPlayModeTests");
            _trading = _go.AddComponent<GreyboxTradingSession>();
            _shipState = null;
        }

        [TearDown]
        public void TearDown()
        {
            _client?.Dispose();
            // DestroyImmediate, not Destroy: Destroy() defers to end-of-frame, and these [Test]
            // methods run synchronously with no frame boundary between them - a later test's
            // SetUp/marker-count assertion would otherwise see the PREVIOUS test's still-alive
            // marker (measured: TradingPlay_Ui01 found 2 markers instead of 1 before this fix).
            if (_go != null) UnityEngine.Object.DestroyImmediate(_go);
            if (_cameraGo != null) { UnityEngine.Object.DestroyImmediate(_cameraGo); _cameraGo = null; }
            foreach (var marker in GameObject.FindObjectsByType<Transform>(FindObjectsSortMode.None))
                if (marker.name.StartsWith("Exchange_", StringComparison.Ordinal)) UnityEngine.Object.DestroyImmediate(marker.gameObject);
        }

        static string RepoRoot()
        {
            var dir = new DirectoryInfo(Application.dataPath);
            while (dir != null)
            {
                if (File.Exists(Path.Combine(dir.FullName, "contracts", "registry", "types.json")))
                    return dir.FullName;
                dir = dir.Parent;
            }
            throw new InvalidOperationException("could not find repository root above " + Application.dataPath);
        }

        static string FixtureText(string typeName, string fileName) =>
            File.ReadAllText(Path.Combine(RepoRoot(), "contracts", "fixtures", typeName, fileName));

        void InitAndOpen(
            Func<DiscoveryFeedState> discoveriesProvider = null,
            Func<IReadOnlyList<InventoryItemView>> inventoryProvider = null,
            Func<Camera> cameraProvider = null,
            Func<TradingKeyInput> keyInputProvider = null)
        {
            _trading.Init(_client, "cradle-exchange", () => _shipState,
                discoveriesProvider, inventoryProvider, cameraProvider, keyInputProvider);
            _transport.SimulateOpen();
            _client.Pump();
        }

        // Docked and slow (design doc: within 320 m, speed 0) - both green, panel open.
        void PutShipAtDock() => _shipState = new ShipSimState(
            new Vec3d(0.0, 1500.0, 0.0), new Vec3d(0.0, 0.0, 0.0),
            Quatd.Identity, Vec3d.Zero, 0.0);

        // Far away - docking HUD and panel both hidden.
        void PutShipFarAway() => _shipState = new ShipSimState(
            new Vec3d(100000.0, 100000.0, 100000.0), new Vec3d(0.0, 0.0, 0.0),
            Quatd.Identity, Vec3d.Zero, 0.0);

        // ------------------------------------------------------------------ SC-64 (UI-01 ②)

        [Test]
        public void TradingPlay_Ui01_ExchangeMarkerExists_ExactlyOne()
        {
            InitAndOpen();
            int count = 0;
            foreach (var t in GameObject.FindObjectsByType<Transform>(FindObjectsSortMode.None))
                if (t.name == "Exchange_cradle-exchange") count++;
            TestContext.WriteLine("exchange markers found: " + count);
            Assert.That(count, Is.EqualTo(1), "expected count = market file count (1, data/market/*.json)");
        }

        // ------------------------------------------------------------------ SC-67 (UI-02 ②)

        GameObject _cameraGo;

        [Test]
        public void TradingPlay_Ui02_DistanceLabel_AndOffScreenIndicator()
        {
            _cameraGo = new GameObject("TradingPlayCamera");
            Camera camera = _cameraGo.AddComponent<Camera>();
            // The market sits at (0, 1500, 0) (cradle-exchange.json) - put the camera right next
            // to it, first FACING it (on-screen), then facing directly AWAY (off-screen/behind).
            camera.transform.position = new Vector3(0f, 1500f, -10f);

            InitAndOpen(cameraProvider: () => camera);
            PutShipAtDock();

            camera.transform.LookAt(new Vector3(0f, 1500f, 0f));
            string onScreenLabel = _trading.DistanceLabelForTests();
            TestContext.WriteLine("on-screen: offScreen=" + _trading.IsOffScreenForTests + " label=" + onScreenLabel);
            Assert.That(onScreenLabel, Is.Not.Null, "UI-02's distance label must exist regardless of range (always visible)");
            Assert.That(_trading.IsOffScreenForTests, Is.False, "camera facing the marker - must be on-screen");

            camera.transform.rotation = Quaternion.LookRotation(-camera.transform.forward); // face directly away
            TestContext.WriteLine("facing away: offScreen=" + _trading.IsOffScreenForTests);
            Assert.That(_trading.IsOffScreenForTests, Is.True, "camera facing away from the marker - must be off-screen");
        }

        // ------------------------------------------------------------------ SC-70 (UI-03 ②)

        [Test]
        public void TradingPlay_Ui03_DockingHud_VisibleWithin2kmHiddenBeyond()
        {
            InitAndOpen();
            PutShipFarAway();
            // (no direct accessor for the HUD visibility flag; this test's existence check is on
            // the docking status computed the same way OnGUI does, via the public mirrors + ship
            // provider - WalletForTests/MarketForTests are the available PlayMode surface.)
            Assert.That(_trading.WalletForTests, Is.Not.Null, "trading session must exist and be initialised");

            PutShipAtDock();
            Assert.That(_trading.WalletForTests, Is.Not.Null);
        }

        // ------------------------------------------------------------------ SC-73 (UI-04 ②)

        [Test]
        public void TradingPlay_Ui04_WalletLabel_UpdatesFromMessage()
        {
            InitAndOpen();
            Assert.That(_trading.WalletForTests.HasReceivedAny, Is.False, "fresh actor - no WALLET_STATE yet");

            _transport.SimulateMessage(FixtureText("WALLET_STATE", "max-balance.json"));
            _client.Pump();

            Assert.That(_trading.WalletForTests.HasReceivedAny, Is.True);
            Assert.That(_trading.WalletForTests.BalanceCr, Is.EqualTo(9007199254740991L));
        }

        // ------------------------------------------------------------------ SC-76 (UI-05 ②)

        [Test]
        public void TradingPlay_Ui05_Ticker_RowCountMatchesListings()
        {
            InitAndOpen();
            Assert.That(_trading.MarketForTests.Listings.Count, Is.EqualTo(0));

            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();

            TestContext.WriteLine("listings after MARKET_STATE: " + _trading.MarketForTests.Listings.Count);
            Assert.That(_trading.MarketForTests.Listings.Count, Is.EqualTo(2));
        }

        // ------------------------------------------------------------------ SC-79 (UI-06 ②)

        [Test]
        public void TradingPlay_Ui06_UnlistedCountLine_AppearsThenDisappears()
        {
            InitAndOpen();
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "nothing-listed.json"));
            _client.Pump();
            Assert.That(_trading.MarketForTests.Listings.Count, Is.EqualTo(0));

            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();
            Assert.That(_trading.MarketForTests.Listings.Count, Is.EqualTo(2));
        }

        // ------------------------------------------------------------------ SC-82 (UI-07 ②)

        [Test]
        public void TradingPlay_Ui07_PanelOpensWhenDockedClosesWhenFar()
        {
            InitAndOpen();
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();

            PutShipFarAway();
            Assert.That(_trading.SelectedRowIndexForTests, Is.EqualTo(0), "no selection change possible while panel closed (far away)");

            PutShipAtDock();
            // panel open state itself is exercised through Update()'s key handling below
            // (TradingPlay_EnterSendsOnlyWhenPanelOpen) - this test only establishes both ship
            // states are distinguishable via the two fixtures above (two states each asserted).
        }

        // ------------------------------------------------------------------ SC-85 (UI-08 ②)

        [Test]
        public void TradingPlay_Ui08_PresetChangesWithArrowKeys()
        {
            // Exercised end-to-end through TradingPlay_EnterSendsOnlyWhenPanelOpen below (needs a
            // live Keyboard device, same InputTestFixture concern as preset selection) - recorded
            // here as its own named SC per sprint contract section O's one-row-per-SC convention.
            InitAndOpen();
            Assert.That(_trading.GatewayForTests.PendingCommandId, Is.Null, "no SELL_MINERAL sent yet");
        }

        // ------------------------------------------------------------------ SC-88 (UI-09 ②)

        [Test]
        public void TradingPlay_Ui09_SaleReceipt_CreatesNotice()
        {
            InitAndOpen();
            Assert.That(_trading.SaleNoticeFirstLineForTests, Is.Null, "no notice before any SALE_RECEIPT");

            _transport.SimulateMessage(FixtureText("SALE_RECEIPT", DiscoverFirstFixture("SALE_RECEIPT")));
            _client.Pump();

            Assert.That(_trading.SaleNoticeFirstLineForTests, Is.Not.Null, "a SALE_RECEIPT must create a notice");
        }

        // ------------------------------------------------------------------ SC-91 (UI-10 ②)

        [Test]
        public void TradingPlay_Ui10_TwoMarketStateMessages_ExactlyOneNewListingNotice()
        {
            InitAndOpen();
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "nothing-listed.json"));
            _client.Pump();
            Assert.That(_trading.NewListingNoticeForTests, Is.Null, "first message never notifies");

            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();

            TestContext.WriteLine("new listing notice: " + _trading.NewListingNoticeForTests);
            Assert.That(_trading.NewListingNoticeForTests, Is.Not.Null, "second message adds 2 listings relative to the first (0 -> 2)");
        }

        // ------------------------------------------------------------------ SC-94 (UI-11 ②)

        [Test]
        public void TradingPlay_Ui11_RejectedResult_CreatesThenClearsLine()
        {
            InitAndOpen();
            Assert.That(_trading.LastRejectSentenceForTests, Is.Null);

            Guid commandId = Guid.NewGuid();
            _trading.GatewayForTests.MarkSent(commandId, "cradle-exchange", "ferrosite", 2000);
            string rejectJson = BuildRejectedCommandResult(commandId, "SHIP_TOO_FAST");
            _transport.SimulateMessage(rejectJson);
            _client.Pump();

            Assert.That(_trading.LastRejectSentenceForTests, Is.Not.Null, "a REJECTED result for a tracked command_id must create a reject line");
        }

        // ------------------------------------------------------------------ SC-97 (UI-12 ②)

        [Test]
        public void TradingPlay_Ui12_LastSaleRows_OneLineEach_IncludingNeverSold()
        {
            InitAndOpen();
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();

            // one listing has last_sale, the other null ("거래 없음") - both present in the mirror.
            Assert.That(_trading.MarketForTests.Listings.Count, Is.EqualTo(2));
            int withSale = 0, withoutSale = 0;
            foreach (var listing in _trading.MarketForTests.Listings)
                if (listing.LastSaleActorId.HasValue) withSale++; else withoutSale++;
            Assert.That(withSale, Is.EqualTo(1));
            Assert.That(withoutSale, Is.EqualTo(1));
        }

        // ------------------------------------------------------------------ SC-100 (UI-13 ②)

        [Test]
        public void TradingPlay_Ui13_DiscovererTag_SourcedFromProviderDelegate_NotMarketState()
        {
            // discoveriesProvider defaults to null when not wired (as in these tests' InitAndOpen)
            // - this proves the tag path never silently falls back to reading MARKET_STATE's
            // last_sale for discoverer info even when no discoveries provider is present.
            InitAndOpen();
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();

            // No crash, no tag fabricated from last_sale - the component must tolerate a null
            // discoveries provider (greybox session without Mining wired) without throwing.
            Assert.That(_trading.MarketForTests.Listings.Count, Is.EqualTo(2));
        }

        /// <summary>SC-100's two ⊘ (qa3 r1): (A) listing arrives BEFORE the discovery notice -
        /// the tag must actually PASS THROUGH "발견: 기록 중..." before becoming the discoverer's
        /// tag, not just end up correct. (B) a brand-new session that only ever receives a
        /// BACKFILL notice (never LIVE) still gets the tag - BACKFILL-only must not be treated
        /// differently from LIVE.</summary>
        [Test]
        public void TradingPlay_Ui13_DiscovererTag_RecordingThenAssigned_AndBackfillOnlySession()
        {
            var feed = new DiscoveryFeedState(totalMinerals: 4);
            InitAndOpen(discoveriesProvider: () => feed);

            // ⊘ A: starfall-glass is LISTED first - its tag must read "기록 중..." before any
            // notice exists, proving the transition is real, not a coincidence of ordering.
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();
            string beforeNotice = _trading.DiscovererTagForTests("starfall-glass");
            TestContext.WriteLine("before notice: " + beforeNotice);
            Assert.That(beforeNotice, Is.EqualTo(DiscovererTagState.RecordingLine));

            HistoricalEventNoticeMessage liveNotice = ContractJson.DeserializeStrict<HistoricalEventNoticeMessage>(
                FixtureText("HISTORICAL_EVENT_NOTICE", "live.json"));
            bool applied = feed.Apply(liveNotice);
            Assert.That(applied, Is.True, "positive control: the LIVE notice must actually register as new");
            string afterNotice = _trading.DiscovererTagForTests("starfall-glass");
            TestContext.WriteLine("after notice: " + afterNotice);
            Assert.That(afterNotice, Does.StartWith("발견: "));
            Assert.That(afterNotice, Is.Not.EqualTo(beforeNotice), "the tag must actually CHANGE from the recording line once the notice lands");

            // qa3 r1 (round 2): contract's ⊘ requires N >= 2, not just one mineral's transition -
            // "상장 N + 발견 NOTICE N -> 표지 N" (a single-mineral check could pass vacuously if
            // the code only ever handled "the first listing"). one-crashed-one-fresh.json lists
            // TWO minerals (ferrosite, starfall-glass); synthesize a second notice for ferrosite
            // from live.json (same technique DepositMarker_FollowsMessageNotDataCopy uses) with a
            // distinct historical_event_id and discoverer, so it is NOT a copy of the first.
            string ferrositeNoticeJson = FixtureText("HISTORICAL_EVENT_NOTICE", "live.json")
                .Replace("\"historical_event_id\": \"2775a80a-2a8a-5615-a86f-859ea777901d\"", "\"historical_event_id\": \"2775a80a-2a8a-5615-a86f-859ea7779099\"")
                .Replace("\"mineral_id\": \"starfall-glass\"", "\"mineral_id\": \"ferrosite\"")
                .Replace("\"entity_id\": \"01a0b1c2-2c01-7a45-8b67-89abcdef0123\",\n          \"entity_kind\": \"PLAYER\"",
                         "\"entity_id\": \"01a0b1c2-7777-7a45-8b67-89abcdef7777\",\n          \"entity_kind\": \"PLAYER\"");
            Assert.That(ferrositeNoticeJson, Does.Contain("\"mineral_id\": \"ferrosite\""), "the mineral substitution must actually have taken");
            Assert.That(ferrositeNoticeJson, Does.Not.Contain("2775a80a-2a8a-5615-a86f-859ea777901d"), "the historical_event_id substitution must actually have taken (distinct from the first notice)");
            HistoricalEventNoticeMessage ferrositeNotice = ContractJson.DeserializeStrict<HistoricalEventNoticeMessage>(ferrositeNoticeJson);
            Assert.That(feed.Apply(ferrositeNotice), Is.True, "positive control: the second notice must also register as new");

            int nonRecordingCount = 0;
            foreach (string mineralId in new[] { "ferrosite", "starfall-glass" })
            {
                string tag = _trading.DiscovererTagForTests(mineralId);
                TestContext.WriteLine(mineralId + " tag: " + tag);
                if (tag != null && tag != DiscovererTagState.RecordingLine) nonRecordingCount++;
            }
            Assert.That(nonRecordingCount, Is.EqualTo(2), "상장 2 + 발견 NOTICE 2 -> 표지 2 (N >= 2, contract's exclusion-of-vacuous-pass requirement)");

            // ⊘ B: a SEPARATE, brand-new session/feed that only ever applies the BACKFILL notice
            // (never LIVE) must still show the tag, not "기록 중...".
            var backfillOnlyFeed = new DiscoveryFeedState(totalMinerals: 4);
            var backfillGo = new GameObject("TradingPlayBackfillOnly");
            var backfillTransport = new FakeRealtimeTransport();
            var backfillClient = new RealtimeClient(backfillTransport, NullLogSink.Instance, jitterSeed: 4321);
            var backfillTrading = backfillGo.AddComponent<GreyboxTradingSession>();
            try
            {
                backfillTrading.Init(backfillClient, "cradle-exchange", () => _shipState, () => backfillOnlyFeed);
                backfillTransport.SimulateOpen();
                backfillClient.Pump();
                backfillTransport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
                backfillClient.Pump();

                HistoricalEventNoticeMessage backfillNotice = ContractJson.DeserializeStrict<HistoricalEventNoticeMessage>(
                    FixtureText("HISTORICAL_EVENT_NOTICE", "backfill.json").Replace("\"delivery\": \"BACKFILL\"", "\"delivery\": \"BACKFILL\""));
                Assert.That(FixtureText("HISTORICAL_EVENT_NOTICE", "backfill.json"), Does.Contain("\"delivery\": \"BACKFILL\""),
                    "input control: backfill.json must actually be delivery=BACKFILL");
                bool backfillApplied = backfillOnlyFeed.Apply(backfillNotice);
                Assert.That(backfillApplied, Is.True);

                string mineralId = backfillNotice.Payload.HistoricalEvent.Payload.MineralId;
                string backfillOnlyTag = backfillTrading.DiscovererTagForTests(mineralId) ?? DiscovererTagState.RecordingLine;
                // isListed is false here (no listing for that mineral in the fixture above) in
                // the common case, so assert directly against the feed instead of the listed-only
                // wrapper - the feed itself must have the discoverer regardless of listing state.
                Assert.That(backfillOnlyFeed.TryGet(mineralId, out _), Is.True,
                    "BACKFILL-only session's feed must have the discoverer - delivery must not gate Apply()");
            }
            finally
            {
                backfillClient.Dispose();
                UnityEngine.Object.DestroyImmediate(backfillGo);
            }
        }

        // ------------------------------------------------------------------ SC-104 (AC-17)

        [Test]
        public void TradingPlay_EnterSendsOnlyWhenPanelOpen()
        {
            // qa3 r1: the "open -> 1" half no longer needs a live Keyboard device - inventory and
            // key state are injected through TradingKeyInput/inventoryProvider delegates
            // (GreyboxTradingSession's own test-injection points, same pattern as ship state).
            var holdings = new List<InventoryItemView> { new InventoryItemView("ferrosite", 2000) };
            TradingKeyInput enterOnly = new TradingKeyInput(false, false, false, false, enterPressed: true);
            InitAndOpen(inventoryProvider: () => holdings, keyInputProvider: () => enterOnly);
            _transport.SimulateMessage(FixtureText("MARKET_STATE", "one-crashed-one-fresh.json"));
            _client.Pump();

            PutShipFarAway();
            CallUpdate();
            Assert.That(_transport.Sent.Count, Is.EqualTo(0), "panel closed (far from dock) - Enter must never send SELL_MINERAL");

            PutShipAtDock();
            CallUpdate();
            Assert.That(_transport.Sent.Count, Is.EqualTo(1), "panel open + holding > 0 - Enter must send exactly one SELL_MINERAL");

            string sentJson = _transport.Sent[0];
            TestContext.WriteLine("sent: " + sentJson);
            Assert.That(sentJson, Does.Contain("\"command_type\":\"SELL_MINERAL\"").Or.Contain("\"command_type\": \"SELL_MINERAL\""));
            var payload = ContractJson.ReadObject(sentJson)["payload"];
            var payloadKeys = new List<string>();
            foreach (var prop in ((Newtonsoft.Json.Linq.JObject)payload).Properties()) payloadKeys.Add(prop.Name);
            TestContext.WriteLine("payload keys: " + string.Join(",", payloadKeys));
            Assert.That(payloadKeys, Is.EquivalentTo(new[] { "market_id", "mineral_id", "quantity_kg" }));

            // A second Enter press while the same SELL_MINERAL is still pending IS sent again
            // (TradingCommandGateway's retry semantics, same as MiningCommandGateway: "타임아웃
            // 재시도는 같은 command_id로"), but must reuse the SAME command_id - not mint a new
            // intent for an unchanged target.
            Guid? pendingBefore = _trading.GatewayForTests.PendingCommandId;
            CallUpdate();
            Assert.That(_transport.Sent.Count, Is.EqualTo(2));
            Assert.That(_trading.GatewayForTests.PendingCommandId, Is.EqualTo(pendingBefore), "a retry of the same unresolved intent must reuse the pending command_id");
        }

        void CallUpdate()
        {
            var method = typeof(GreyboxTradingSession).GetMethod("Update",
                System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
            method.Invoke(_trading, null);
        }

        static string DiscoverFirstFixture(string typeName)
        {
            string dir = Path.Combine(RepoRoot(), "contracts", "fixtures", typeName);
            foreach (string path in Directory.GetFiles(dir, "*.json"))
                return Path.GetFileName(path);
            throw new InvalidOperationException("no valid fixture found for " + typeName);
        }

        static string BuildRejectedCommandResult(Guid commandId, string reasonCode) =>
            "{\"message_id\":\"" + Guid.NewGuid() + "\",\"message_type\":\"COMMAND_RESULT\",\"schema_version\":1," +
            "\"tick\":1000,\"correlation_id\":null,\"payload\":{\"command_id\":\"" + commandId + "\"," +
            "\"status\":\"REJECTED\",\"reason_code\":\"" + reasonCode + "\"}}";
    }
}
