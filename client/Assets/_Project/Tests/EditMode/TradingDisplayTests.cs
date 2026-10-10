// EditMode tests for C2 (p1-03-trading): UI-01~UI-13's display-logic (part 1) and AC-16's
// "client never computes" structural checks. Sprint contract section O (SC-63, 66, 69, 72, 75,
// 78, 81, 84, 87, 90, 93, 96, 99) and section P (SC-102, 103).
//
// Expected values come from design doc section 8.3's wording/rules and section 3.6's number
// table independently of the implementation (contract section 0.6) - the UI-08 cases in
// particular reuse the exact P1/P9/P10 figures sprint contract section 0.6 lists (qa3's
// independent hand-recalculation), the same inputs server SC-14 asserts against (AC-15 UI-08
// requires the two to agree on the same input).

using System;
using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;
using Starfall.Sim;
using Starfall.Trading;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class TradingDisplayTests
    {
        // cradle-exchange.json's listing constants (design doc section 3.6) - used directly
        // (not through MarketTable.FromJson) so these tests do not depend on data/ at all.
        static readonly MarketListingData Ferrosite = new MarketListingData
        { MineralId = "ferrosite", BasePriceCrPerKg = 10, BandKg = 2000, RecoveryKg = 1000 };
        static readonly MarketListingData Glacine = new MarketListingData
        { MineralId = "glacine", BasePriceCrPerKg = 14, BandKg = 1500, RecoveryKg = 750 };
        const int PriceFloorPct = 30;
        const int PriceDropPctPerBand = 10;

        // ------------------------------------------------------------------ SC-63 (UI-01 ①)

        [Test]
        public void TradingUi01_MarkerGeometry_DockingZoneIsRadiusPlusDockRange()
        {
            var geometry = new ExchangeMarkerGeometry(radiusM: 120.0, dockRangeFromSurfaceM: 200.0);
            Assert.That(geometry.RadiusM, Is.EqualTo(120.0));
            Assert.That(geometry.DockingZoneRadiusM, Is.EqualTo(320.0));

            // input-dependent, not hardcoded - a second input must change the output.
            var other = new ExchangeMarkerGeometry(radiusM: 50.0, dockRangeFromSurfaceM: 10.0);
            Assert.That(other.DockingZoneRadiusM, Is.EqualTo(60.0));

            // qa3 r1 gap: "도킹 표시 두 ●이 초록이면 구역 색 변경" (design doc section 8.3 UI-01).
            var bothGreen = new DockingRangeStatus(0.0, withinDockingZone: true, speedOk: true, visible: true);
            var oneRed = new DockingRangeStatus(0.0, withinDockingZone: true, speedOk: false, visible: true);
            Assert.That(ExchangeMarkerColorRule.Resolve(bothGreen), Is.EqualTo(DockingZoneColor.BothConditionsGreen));
            Assert.That(ExchangeMarkerColorRule.Resolve(oneRed), Is.EqualTo(DockingZoneColor.Default));
        }

        // ------------------------------------------------------------------ SC-66 (UI-02 ①)

        [Test]
        public void TradingUi02_DistanceLabel_MetresBelow2000_KilometresAtAndAbove()
        {
            Assert.That(ExchangeDistanceLabel.Format("Cradle Exchange", 1999.0), Is.EqualTo("Cradle Exchange · 1999 m"));
            Assert.That(ExchangeDistanceLabel.Format("Cradle Exchange", 2000.0), Is.EqualTo("Cradle Exchange · 2.0 km"));
            Assert.That(ExchangeDistanceLabel.Format("Cradle Exchange", 4200.0), Is.EqualTo("Cradle Exchange · 4.2 km"));

            // qa3 r1 gap: "화면 밖·뒤일 때 가장자리에 화살표" - front/side/behind, each asserted.
            Assert.That(ExchangeDistanceLabel.IsOffScreen(0.5, 0.5, 1.0), Is.False, "centre of the screen, in front - on-screen");
            Assert.That(ExchangeDistanceLabel.IsOffScreen(1.5, 0.5, 1.0), Is.True, "in front but past the right edge - off-screen");
            Assert.That(ExchangeDistanceLabel.IsOffScreen(0.5, 0.5, -1.0), Is.True, "behind the camera - off-screen regardless of x/y");
        }

        // ------------------------------------------------------------------ SC-69 (UI-03 ①)

        [Test]
        public void TradingUi03_DockingRangeStatus_BoundaryExactly320And10()
        {
            // 319.9 / 320 / 320.1 m at zero speed; 10.0 / 10.1 m/s at the docking distance.
            var at3199 = DockingRangeEvaluator.Evaluate(319.9 + 0.0, 0.0, 0.0, 320.0, 10.0);
            var at320 = DockingRangeEvaluator.Evaluate(320.0, 0.0, 0.0, 320.0, 10.0);
            var at3201 = DockingRangeEvaluator.Evaluate(320.1, 0.0, 0.0, 320.0, 10.0);
            Assert.That(at3199.WithinDockingZone, Is.True);
            Assert.That(at320.WithinDockingZone, Is.True, "320 m exactly is still within the docking zone (<=, design doc section 2.1 check 4)");
            Assert.That(at3201.WithinDockingZone, Is.False);

            var speed10 = DockingRangeEvaluator.Evaluate(0.0, 0.0, 10.0, 320.0, 10.0);
            var speed101 = DockingRangeEvaluator.Evaluate(0.0, 0.0, 10.1, 320.0, 10.0);
            Assert.That(speed10.SpeedOk, Is.True);
            Assert.That(speed101.SpeedOk, Is.False);

            // > 2 km surface distance hides the HUD entirely.
            var farAway = DockingRangeEvaluator.Evaluate(2000.1, 0.0, 0.0, 320.0, 10.0);
            Assert.That(farAway.Visible, Is.False);
        }

        // ------------------------------------------------------------------ SC-72 (UI-04 ①)

        [Test]
        public void TradingUi04_WalletLabel_ThousandsSeparatorAndZero()
        {
            Assert.That(WalletLabel.Format(20000), Is.EqualTo("₵ 20,000"));
            Assert.That(WalletLabel.Format(0), Is.EqualTo("₵ 0"));

            // AC-16(a)/I-77: the label only changes through WalletMirrorState.ApplyWalletState.
            var wallet = new WalletMirrorState();
            Assert.That(wallet.HasReceivedAny, Is.False);
            Assert.That(WalletLabel.Format(wallet.BalanceCr), Is.EqualTo("₵ 0"));

            WalletStateMessage message = ContractJson.DeserializeStrict<WalletStateMessage>(
                ContractFixtures.RequireValid("WALLET_STATE", "max-balance.json").ReadText());
            wallet.ApplyWalletState(message);
            Assert.That(wallet.HasReceivedAny, Is.True);
            Assert.That(wallet.BalanceCr, Is.EqualTo(9007199254740991L), "positive control: the fixture's real value must land in the mirror");
            TestContext.WriteLine("wallet after WALLET_STATE: " + WalletLabel.Format(wallet.BalanceCr));
        }

        // ------------------------------------------------------------------ SC-75 (UI-05 ①)

        [Test]
        public void TradingUi05_TickerLine_PercentDownFromBase()
        {
            // design doc section 8.3 example: 10 -> 3 cr/kg is 70% down.
            Assert.That(TickerFormat.Line("Ferrosite", 3, 10), Is.EqualTo("Ferrosite  3 cr/kg ▼70%"));

            // a second mineral with different numbers, independent of the first.
            Assert.That(TickerFormat.Line("Starfall Glass", 250, 250), Is.EqualTo("Starfall Glass  250 cr/kg ▼0%"));

            Assert.That(TickerFormat.NoListingsLine, Is.EqualTo("상장된 광물 없음 — 먼저 캐야 한다"));

            // qa3 r1 gap: "행 순서 = mineral_id 오름차순" (design doc section 8.3 UI-05). Synthetic
            // input deliberately out of order (starfall-glass, cobaltine, ferrosite) - the schema
            // promises the SERVER sends ascending, but the DISPLAY must not simply trust that;
            // MarketMirrorState.ApplyMarketState re-sorts independently of input order.
            string unsortedJson = @"{
              ""message_id"": ""01a0b1c2-6c43-7124-9235-4a5b6c7d8ec2"",
              ""message_type"": ""MARKET_STATE"",
              ""schema_version"": 1,
              ""tick"": 100,
              ""correlation_id"": null,
              ""payload"": {
                ""market_id"": ""cradle-exchange"",
                ""listings"": [
                  { ""mineral_id"": ""starfall-glass"", ""saturation_kg"": 0, ""price_cr_per_kg"": 250, ""last_sale"": null },
                  { ""mineral_id"": ""cobaltine"", ""saturation_kg"": 0, ""price_cr_per_kg"": 40, ""last_sale"": null },
                  { ""mineral_id"": ""ferrosite"", ""saturation_kg"": 0, ""price_cr_per_kg"": 10, ""last_sale"": null }
                ]
              }
            }";
            var market = new MarketMirrorState();
            market.ApplyMarketState(ContractJson.DeserializeStrict<MarketStateMessage>(unsortedJson));

            var orderedIds = new List<string>();
            foreach (MarketListingView listing in market.Listings) orderedIds.Add(listing.MineralId);
            TestContext.WriteLine("row order: " + string.Join(", ", orderedIds));
            Assert.That(orderedIds, Is.EqualTo(new[] { "cobaltine", "ferrosite", "starfall-glass" }),
                "input order was starfall-glass/cobaltine/ferrosite - the mirror must re-sort ascending regardless");
        }

        // ------------------------------------------------------------------ SC-78 (UI-06 ①)

        [Test]
        public void TradingUi06_UnlistedCount_NameNeverAppearsInLines()
        {
            Assert.That(TickerFormat.UnlistedCountLine(totalMineralCount: 4, listedCount: 1), Is.EqualTo("미상장 3종"));

            var market = new MarketMirrorState();
            MarketStateMessage message = ContractJson.DeserializeStrict<MarketStateMessage>(
                ContractFixtures.RequireValid("MARKET_STATE", "one-crashed-one-fresh.json").ReadText());
            market.ApplyMarketState(message);

            string[] unlistedMineralDisplayNames = { "Glacine", "Cobaltine" };
            string ticker = TickerFormat.Line("Ferrosite", 3, 10) + "\n" + TickerFormat.Line("Starfall Glass", 250, 250) +
                             "\n" + TickerFormat.UnlistedCountLine(4, market.Listings.Count);
            foreach (string name in unlistedMineralDisplayNames)
                Assert.That(ticker, Does.Not.Contain(name), name + " must not appear anywhere in the ticker - it has no listing");
        }

        // ------------------------------------------------------------------ SC-81 (UI-07 ①)

        [Test]
        public void TradingUi07_PanelRow_BarRangeAndDimmedWhenNoHolding()
        {
            var floorRow = new SellPanelRow("ferrosite", "Ferrosite", 3, 14000, 14000, 0);
            var nonFloorRow = new SellPanelRow("glacine", "Glacine", 12, 1000, 10500, 2000);
            var noHoldingRow = new SellPanelRow("cobaltine", "Cobaltine", 40, 0, 4200, 0);

            Assert.That(floorRow.SaturationKg, Is.EqualTo(floorRow.SaturationCapKg), "floor row sits at the cap");
            Assert.That(nonFloorRow.SaturationKg, Is.LessThan(nonFloorRow.SaturationCapKg));
            Assert.That(noHoldingRow.IsDimmed, Is.True);
            Assert.That(nonFloorRow.IsDimmed, Is.False);

            // qa3 r1 gap 1: per-row band phrase ("이 값으로 N kg 더" / "바닥가" at the floor).
            Assert.That(SellPanelBandPhrase.Format(1400), Is.EqualTo("이 값으로 1,400 kg 더"));
            Assert.That(SellPanelBandPhrase.Format(-1), Is.EqualTo("바닥가"));

            // qa3 r1 gap 2: "열림 = 두 표시 조건이 참일 때" - both green opens the panel, one red closes it.
            var bothGreen = DockingRangeEvaluator.Evaluate(100.0, 120.0, 0.0, 200.0, 10.0);
            var tooFast = DockingRangeEvaluator.Evaluate(100.0, 120.0, 15.0, 200.0, 10.0);
            Assert.That(bothGreen.BothGreen, Is.True, "within range and slow enough - panel must open");
            Assert.That(tooFast.BothGreen, Is.False, "too fast - panel must stay closed even though in range");

            // qa3 r1 gap 3: ↑/↓ row selection moves, wrapping at both ends (3 rows: 0,1,2).
            Assert.That(SellPanelSelection.MoveDown(0, 3), Is.EqualTo(1));
            Assert.That(SellPanelSelection.MoveDown(2, 3), Is.EqualTo(0), "wraps past the last row");
            Assert.That(SellPanelSelection.MoveUp(0, 3), Is.EqualTo(2), "wraps past the first row");
            Assert.That(SellPanelSelection.MoveUp(1, 3), Is.EqualTo(0));
        }

        // ------------------------------------------------------------------ SC-84 (UI-08 ①)

        [Test]
        public void TradingUi08_ExpectedProceeds_MatchesDesignTableP1P9P10()
        {
            // P1 (section 0.6): Ferrosite, saturation 0, 12,000 kg -> 90,000 cr.
            SaleProjection p1 = PriceRule.Sell(0, 12000, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(p1.ProceedsCr, Is.EqualTo(90000));

            // P9: Ferrosite, saturation 0, 2,000 kg -> 20,000 cr.
            SaleProjection p9 = PriceRule.Sell(0, 2000, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(p9.ProceedsCr, Is.EqualTo(20000));
            Assert.That(p9.SaturationAfterKg, Is.EqualTo(2000));

            // P10: Ferrosite, saturation 2,000 (post-P9), 2,000 kg -> 18,000 cr.
            SaleProjection p10 = PriceRule.Sell(p9.SaturationAfterKg, 2000, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(p10.ProceedsCr, Is.EqualTo(18000));

            // design doc section 8.3 example: s = 600 kg into Ferrosite -> 1,400 kg to next band.
            int amountToNextBand = PriceRule.AmountToNextBand(600, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(amountToNextBand, Is.EqualTo(1400));

            // "팔 것 없음" when holding is 0, and Enter must be a structural no-op (CanSell false).
            SellQuantityPresets empty = SellPanelCalculator.ComputePresets(0, 0, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(SellPanelCalculator.FormatLine(empty), Is.EqualTo("팔 것 없음"));
            Assert.That(empty.CanSell, Is.False, "AC-17/design doc: holding 0 -> Enter must be invalid");

            // at the floor band, the two presets collapse into one (design doc section 3.3) -
            // qa3 r1 gap: assert the FORMATTED line too, not just the flag (only "전량", no "/").
            SellQuantityPresets atFloor = SellPanelCalculator.ComputePresets(14000, 500, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(atFloor.HasDistinctUpToBandPreset, Is.False);
            Assert.That(atFloor.CanSell, Is.True, "holding > 0 at the floor - Enter must still be valid");
            string floorLine = SellPanelCalculator.FormatLine(atFloor);
            Assert.That(floorLine, Does.Not.Contain("/"), "a single preset must not show the '이 값까지 ... / 전량 ...' split");
            Assert.That(floorLine, Does.StartWith("전량"));
        }

        // ------------------------------------------------------------------ SC-87 (UI-09 ①)

        [Test]
        public void TradingUi09_SaleResultNotice_SecondLineOnlyWhenExpectedExceedsActual()
        {
            // expected 20,000, actual 18,000 (N = 2,000) - second line present.
            SaleResultNotice lessThanExpected = SaleResultNoticeBuilder.Build("Ferrosite", 2000, 9, 18000, 20000);
            Assert.That(lessThanExpected.FirstLine, Is.EqualTo("+₵ 18,000 · Ferrosite 2,000 kg 판매 (9 cr/kg)"));
            Assert.That(lessThanExpected.HasSecondLine, Is.True);
            Assert.That(lessThanExpected.SecondLine, Is.EqualTo("예상보다 ₵2,000 적다 — 직전에 다른 판매가 있었다"));

            // expected 18,000, actual 20,000 - no second line (not a loss).
            SaleResultNotice moreThanExpected = SaleResultNoticeBuilder.Build("Ferrosite", 2000, 10, 20000, 18000);
            Assert.That(moreThanExpected.HasSecondLine, Is.False);
        }

        // ------------------------------------------------------------------ SC-90 (UI-10 ①)

        [Test]
        public void TradingUi10_NewListingDetector_FirstMessageNeverNotifies()
        {
            var secondMessageListings = new[] { "ferrosite", "starfall-glass" };

            // first message: previous is null -> no notifications even though it "adds" 2.
            var firstMessageResult = NewListingDetector.Detect(null, new[] { "ferrosite" });
            Assert.That(firstMessageResult, Is.Empty);

            // second message adds exactly one mineral relative to the first.
            var secondResult = NewListingDetector.Detect(new[] { "ferrosite" }, secondMessageListings);
            Assert.That(secondResult, Is.EquivalentTo(new[] { "starfall-glass" }));
        }

        // ------------------------------------------------------------------ SC-93 (UI-11 ①)

        [Test]
        public void TradingUi11_RejectReasonText_FourKnownSentencesDistinctAndNonEmpty()
        {
            string[] codes = { "TARGET_OUT_OF_RANGE", "SHIP_TOO_FAST", "INSUFFICIENT_QUANTITY", "COOLDOWN_ACTIVE" };
            var sentences = new List<string>();
            foreach (string code in codes)
            {
                string sentence = TradingRejectReasonText.Resolve(code);
                Assert.That(sentence, Is.Not.Null.And.Not.Empty);
                sentences.Add(sentence);
            }
            Assert.That(new HashSet<string>(sentences).Count, Is.EqualTo(sentences.Count), "the four sentences must be distinct");

            // unknown code must not throw and must render something (ADR-0005 section 4).
            Assert.DoesNotThrow(() => TradingRejectReasonText.Resolve("SOME_FUTURE_CODE_2027"));
        }

        // ------------------------------------------------------------------ SC-96 (UI-12 ①)

        [Test]
        public void TradingUi12_LastSaleLine_MineSomeoneElseAndNeverSold()
        {
            Guid me = Guid.Parse("01a0b1c2-2c01-7a45-8b67-89abcdef0123");
            Guid someoneElse = Guid.Parse("01a0b1c2-9999-7a45-8b67-89abcdef9999");

            Assert.That(LastSaleLine.Format(null, 0, 0, me), Is.EqualTo("거래 없음"));
            Assert.That(LastSaleLine.Format(me, 14000, 45.0, me), Is.EqualTo("마지막 거래: 나 · 14,000 kg · 45초 전"));
            string otherLine = LastSaleLine.Format(someoneElse, 500, 125.0, me);
            Assert.That(otherLine, Does.StartWith("마지막 거래: Pilot-"));
            Assert.That(otherLine, Does.Contain("2분 전"));
        }

        // ------------------------------------------------------------------ SC-99 (UI-13 ①)

        [Test]
        public void TradingUi13_DiscovererTag_DrivenOnlyByHistoricalNotice_NotByMarketState()
        {
            var state = new DiscovererTagState();
            Guid me = Guid.Parse("01a0b1c2-2c01-7a45-8b67-89abcdef0123"); // same id as live.json's discoverer

            Assert.That(state.FormatTag("starfall-glass", isListed: true, myActorId: me), Is.EqualTo(DiscovererTagState.RecordingLine));
            Assert.That(state.FormatTag("ferrosite", isListed: false, myActorId: me), Is.Null, "not listed -> no tag at all");

            HistoricalEventNoticeMessage notice = ContractJson.DeserializeStrict<HistoricalEventNoticeMessage>(
                ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "live.json").ReadText());
            bool changed = state.Apply(notice);
            Assert.That(changed, Is.True);
            Assert.That(state.FormatTag("starfall-glass", isListed: true, myActorId: me), Is.EqualTo("발견: 나"));

            // last_sale.actor_id differs from the discoverer - the tag must still read from the
            // notice, not from a MARKET_STATE field (design doc: "서버 시장 메시지에서 읽지 않는다").
            Guid differentSeller = Guid.Parse("01a0b1c2-9999-7a45-8b67-89abcdef9999");
            string otherViewerTag = state.FormatTag("starfall-glass", isListed: true, myActorId: differentSeller);
            Assert.That(otherViewerTag, Is.EqualTo("발견: " + TradingPilotTag.From(me)), "tag must come from the notice's discoverer, not from any last_sale-shaped input");

            // qa3 r1 gap 1: someone ELSE discovers (not the viewer) - must render "발견: Pilot-xxxx".
            Guid otherDiscoverer = Guid.Parse("01a0b1c2-8888-7a45-8b67-89abcdef8888");
            var stateOther = new DiscovererTagState();
            string syntheticNoticeJson = ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "live.json").ReadText()
                .Replace("\"entity_id\": \"01a0b1c2-2c01-7a45-8b67-89abcdef0123\",\n          \"entity_kind\": \"PLAYER\"",
                         "\"entity_id\": \"" + otherDiscoverer + "\",\n          \"entity_kind\": \"PLAYER\"");
            Assert.That(syntheticNoticeJson, Does.Contain(otherDiscoverer.ToString()), "the discoverer-id substitution must actually have taken");
            HistoricalEventNoticeMessage otherNotice = ContractJson.DeserializeStrict<HistoricalEventNoticeMessage>(syntheticNoticeJson);
            stateOther.Apply(otherNotice);
            string viewerIsNotDiscoverer = stateOther.FormatTag("starfall-glass", isListed: true, myActorId: me);
            TestContext.WriteLine("other-discoverer tag: " + viewerIsNotDiscoverer);
            Assert.That(viewerIsNotDiscoverer, Does.StartWith("발견: Pilot-"));
            Assert.That(viewerIsNotDiscoverer, Is.Not.EqualTo("발견: 나"));

            // qa3 r1 gap 2: "한 번 붙으면 사라지지 않는다" - sticky across a LATER, unrelated notice.
            HistoricalEventNoticeMessage anotherMineralNotice = ContractJson.DeserializeStrict<HistoricalEventNoticeMessage>(
                ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "backfill.json").ReadText());
            state.Apply(anotherMineralNotice);
            Assert.That(state.FormatTag("starfall-glass", isListed: true, myActorId: me), Is.EqualTo("발견: 나"),
                "starfall-glass's tag must stay exactly as it was after an unrelated later notice (sticky)");
        }

        // ------------------------------------------------------------------ SC-102/103 (AC-16)

        [Test]
        public void Trading_ClientNeverComputesBalanceOrHoldings()
        {
            var wallet = new WalletMirrorState();
            long before = wallet.BalanceCr;

            // Computing an expected-proceeds preview must not touch the mirror at all.
            PriceRule.Sell(0, 2000, Ferrosite, PriceFloorPct, PriceDropPctPerBand);
            Assert.That(wallet.BalanceCr, Is.EqualTo(before), "a preview calculation must never change the mirrored balance");

            // Only WALLET_STATE changes it (positive control: it must actually change, to the
            // fixture's real value - not just "no assertion failed").
            WalletStateMessage message = ContractJson.DeserializeStrict<WalletStateMessage>(
                ContractFixtures.RequireValid("WALLET_STATE", "max-balance.json").ReadText());
            wallet.ApplyWalletState(message);
            Assert.That(wallet.BalanceCr, Is.EqualTo(9007199254740991L));
            TestContext.WriteLine("balance after WALLET_STATE: " + wallet.BalanceCr);
        }

        [Test]
        public void Trading_SourceScan_NoAddOrSubtractOnBalanceOrHoldings()
        {
            // Negative proof (SC-103) - must be paired with SC-102 (rule 9).
            string scriptsRoot = System.IO.Path.Combine(ContractFixtures.RequireRepoRoot(), "client", "Assets", "_Project", "Scripts");
            Assert.That(System.IO.Directory.Exists(scriptsRoot), Is.True);

            var pattern = new System.Text.RegularExpressions.Regex(
                @"(BalanceCr|balance_cr|QuantityKg|quantity_kg)\s*[+\-]=");

            string[] sources = System.IO.Directory.GetFiles(scriptsRoot, "*.cs", System.IO.SearchOption.AllDirectories);
            TestContext.WriteLine("Scripts/*.cs files scanned: " + sources.Length);
            Assert.That(sources.Length, Is.GreaterThan(0));

            var offenders = new List<string>();
            foreach (string file in sources)
            {
                string text = System.IO.File.ReadAllText(file);
                if (pattern.IsMatch(text)) offenders.Add(file);
            }
            foreach (string offender in offenders) TestContext.WriteLine("matched +=/-= pattern: " + offender);
            Assert.That(offenders.Count, Is.EqualTo(0),
                "no += or -= on BalanceCr/balance_cr/QuantityKg/quantity_kg anywhere under Scripts/.");

            // Positive control: the same pattern must match something (proves the regex itself
            // is live, not just "nothing in this codebase happens to match").
            Assert.That(pattern.IsMatch("int quantity_kg = 0; quantity_kg += 5;"), Is.True);
        }
    }
}
