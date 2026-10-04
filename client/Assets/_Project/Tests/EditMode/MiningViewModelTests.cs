// EditMode tests for C3 (p1-02-mining) pure logic: Starfall.Mining's PilotTag, RejectReasonText,
// DiscoveryTiming, InventoryPanelState, DiscoveryFeedState, MiningCommandGateway. None of these
// need PlayMode - they take DTOs/plain values in and return plain values/state, same testing
// style as p1-01's ReconcileTickDrift/RebaseHold/etc (Starfall.Flight, pure + independently
// tested).
//
// C4 (SC-68 human session, 1st attempt FAIL, 2026-10-04): added MiningRangeStatus/
// MiningRangeEvaluator, DistanceLabel, CooldownDisplay, MiningNoticeKind/MiningNoticeClassifier,
// and DepositMarkerState.FormatLine/FormatLineWithDistance - the pure logic behind design doc §8's
// six greybox elements (markers+distance, range/speed traffic light, cooldown, two notice kinds).

using System;
using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Contracts.Generated;
using Starfall.Mining;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class MiningViewModelTests
    {
        // ------------------------------------------------------------------ PilotTag (SC-110)

        // Contract name (sprint contract §row 361, SC-110): PilotLabel_LastFourChars. Renamed
        // from PilotTag_UsesLastFourCharacters_NotFirst (Phase 5 r1 name reconciliation, 2026-09-30)
        // - assertions unchanged, only the method name.
        [Test]
        public void PilotLabel_LastFourChars()
        {
            // Sprint contract section 0.11 / SC-110: the two dev ids the leader found actually
            // colliding on their LAST 4 characters - used here as the positive input (both must
            // map to the SAME tag), never as a lookup key (see PilotTag.cs header).
            Guid unityDefaultSubject = Guid.Parse("01a0b1c2-7e57-7c11-8e57-000000000001");
            Guid bot001 = Guid.Parse("01a0b1c2-b010-7000-8000-000000000001");

            string tagA = PilotTag.From(unityDefaultSubject);
            string tagB = PilotTag.From(bot001);
            TestContext.WriteLine(unityDefaultSubject + " -> " + tagA);
            TestContext.WriteLine(bot001 + " -> " + tagB);

            Assert.That(tagA, Is.EqualTo("Pilot-0001"));
            Assert.That(tagB, Is.EqualTo("Pilot-0001"));
            Assert.That(tagA, Is.EqualTo(tagB), "both known-colliding dev ids must produce the same display tag (last 4 chars, not first 4)");
        }

        // ------------------------------------------------------------------ RejectReasonText

        static readonly string[] AllKnownReasonCodes =
        {
            "MALFORMED_COMMAND", "UNKNOWN_COMMAND_TYPE", "SCHEMA_VERSION_UNSUPPORTED", "DUPLICATE_COMMAND_ID",
            "SERVER_BUSY", "TOO_MANY_IN_FLIGHT", "RATE_LIMITED", "STALE_INPUT",
            "TARGET_UNKNOWN", "COOLDOWN_ACTIVE", "TARGET_OUT_OF_RANGE", "SHIP_TOO_FAST",
            "RESOURCE_DEPLETED", "RECORDING_BACKLOG", "CAPACITY_EXCEEDED",
        };

        [Test]
        public void RejectReasonText_ResolvesEveryKnownCode_ToADistinctNonEmptySentence()
        {
            var seen = new HashSet<string>();
            foreach (string code in AllKnownReasonCodes)
            {
                string text = RejectReasonText.Resolve(code);
                TestContext.WriteLine(code + " -> " + text);
                Assert.That(string.IsNullOrEmpty(text), Is.False, code + " must resolve to a non-empty sentence");
                Assert.That(seen.Add(text), Is.True, code + " produced a sentence already used by another code: " + text);
            }
            TestContext.WriteLine("known reason codes resolved: " + seen.Count);
            Assert.That(seen.Count, Is.EqualTo(15), "contracts/messages/COMMAND_RESULT.schema.json RejectReasonCode enum has 15 values (8 p1-01 + 7 p1-02-mining)");
        }

        [Test]
        public void RejectReasonText_UnknownCode_FallsBackWithoutThrowing()
        {
            // C1's finding: ReasonCode is a plain C# string precisely so an unknown FUTURE value
            // does not crash the client. This is the UI-layer counterpart of that guarantee.
            string text = RejectReasonText.Resolve("SOME_FUTURE_REASON_NOT_YET_INVENTED");
            TestContext.WriteLine("unknown code -> " + text);
            Assert.That(text, Does.Contain("SOME_FUTURE_REASON_NOT_YET_INVENTED"));
            Assert.That(() => RejectReasonText.Resolve(null), Throws.Nothing);
        }

        // ------------------------------------------------------------------ DiscoveryTiming (design doc S-2/S-2b)

        [Test]
        public void DiscoveryTiming_SameTick_SaysOneStepAhead_NeverZeroSeconds()
        {
            string text = DiscoveryTiming.Describe("Pilot-0001", discovererTick: 24000, myTick: 24000);
            TestContext.WriteLine(text);
            Assert.That(text, Is.EqualTo("Pilot-0001가 한발 먼저 발견했다."));
            Assert.That(text, Does.Not.Contain("0.00"), "a tie must never render as '0.00초 먼저' - design doc S-2");
        }

        [Test]
        public void DiscoveryTiming_TickDeltaSeven_Is035Seconds()
        {
            // Design doc S-2b: tick delta 7 at tick_hz=20 (ADR-0006, 1 tick = 50ms) = 0.35s.
            string text = DiscoveryTiming.Describe("Pilot-0001", discovererTick: 24000, myTick: 24007);
            TestContext.WriteLine(text);
            Assert.That(text, Is.EqualTo("Pilot-0001가 0.35초 먼저 발견했다."));
        }

        // ------------------------------------------------------------------ InventoryPanelState -
        // sprint contract's chosen RED for C3: the panel must not change before INVENTORY_STATE.

        /// <summary>
        /// SC-69 (contract name, sprint contract §row 359): "패널이 아무 입력에도 안 바뀜" (negative,
        /// asserted first) "→ 같은 테스트에서 INVENTORY_STATE 수신 후 값이 바뀜을 단언" (positive,
        /// in the SAME test). Renamed from the split InventoryPanelState_StartsEmpty_
        /// AndHasNoOtherMutator / InventoryPanelState_ApplyInventoryState_ReplacesItemsFromMessage
        /// (Phase 5 r1 name reconciliation, team-lead/qa 2026-09-30) - merged into one test under
        /// the contract's own name; no assertion or production code changed, only which method
        /// they live in.
        /// </summary>
        [Test]
        public void InventoryPanel_ChangesOnlyOnInventoryState()
        {
            var state = new InventoryPanelState();
            Assert.That(state.Items.Count, Is.EqualTo(0));

            // Negative half: structural guard, not just behavioural - enumerate every public
            // method on the type and assert ApplyInventoryState is the only one that could
            // mutate anything (everything else is either the constructor or a property getter).
            // No other input (no matter what it is) has anywhere to attach a mutation.
            var methods = typeof(InventoryPanelState).GetMethods(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.DeclaredOnly);
            var mutatorNames = new List<string>();
            foreach (var m in methods)
                if (!m.IsSpecialName) mutatorNames.Add(m.Name); // excludes property get_/set_ accessors
            foreach (string name in mutatorNames) TestContext.WriteLine("public method: " + name);
            Assert.That(mutatorNames, Is.EquivalentTo(new[] { "ApplyInventoryState" }),
                "InventoryPanelState must expose exactly one way to change Items - a second mutator " +
                "would be a place an optimistic (pre-server) update could sneak back in.");

            // Positive half: INVENTORY_STATE itself DOES change it.
            var message = new InventoryStateMessage
            {
                Tick = 100,
                Payload = new InventoryStateMessage.InventoryStatePayload
                {
                    ActorId = Guid.NewGuid(),
                    Items = new[]
                    {
                        new InventoryStateMessage.InventoryStatePayload.InventoryItem { MineralId = "ferrosite", QuantityKg = 200 },
                    },
                },
            };

            state.ApplyInventoryState(message);

            Assert.That(state.Items.Count, Is.EqualTo(1));
            Assert.That(state.Items[0].MineralId, Is.EqualTo("ferrosite"));
            Assert.That(state.Items[0].QuantityKg, Is.EqualTo(200));
        }

        // ------------------------------------------------------------------ DiscoveryFeedState (LIVE/BACKFILL dedupe)

        static HistoricalEventNoticeMessage MakeNotice(Guid historicalEventId, string mineralId, string depositId, Guid discovererActorId, long tick, string delivery)
        {
            return new HistoricalEventNoticeMessage
            {
                Tick = tick,
                Payload = new HistoricalEventNoticeMessage.HistoricalEventNoticePayload
                {
                    Delivery = delivery,
                    HistoricalEvent = new HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent
                    {
                        HistoricalEventId = historicalEventId,
                        Tick = tick,
                        Participants = new[]
                        {
                            new HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent.HistoricalParticipant
                                { EntityId = discovererActorId, EntityKind = "PLAYER", Role = "DISCOVERER" },
                            new HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent.HistoricalParticipant
                                { EntityId = Guid.NewGuid(), EntityKind = "SHIP", Role = "VESSEL" },
                        },
                        Payload = new HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent.MineralDiscoveredPayload
                            { MineralId = mineralId, DepositId = depositId, QuantityKg = 25 },
                    },
                },
            };
        }

        [Test]
        public void DiscoveryFeedState_DedupesByHistoricalEventId_LiveThenBackfillCountsOnce()
        {
            // 02_client_ack.md C1's flagged scenario: the same discovery arriving once as LIVE
            // and again as BACKFILL (a reconnect, or a race with the BACKFILL list) must count
            // once, not twice. Synthetic input (fixtures don't have a same-id pair) - documented
            // as such per SC-70's agreed input-source note.
            var feed = new DiscoveryFeedState(totalMinerals: 4);
            Guid id = Guid.NewGuid();
            Guid discoverer = Guid.NewGuid();

            bool firstIsNew = feed.Apply(MakeNotice(id, "starfall-glass", "far-reach", discoverer, 24000, "LIVE"));
            bool secondIsNew = feed.Apply(MakeNotice(id, "starfall-glass", "far-reach", discoverer, 24000, "BACKFILL"));

            TestContext.WriteLine("first (LIVE) isNew=" + firstIsNew + ", second (BACKFILL, same id) isNew=" + secondIsNew);
            Assert.That(firstIsNew, Is.True);
            Assert.That(secondIsNew, Is.False, "a second notice with the SAME historical_event_id must not count as a new discovery");
            Assert.That(feed.DiscoveredCount, Is.EqualTo(1));
        }

        [Test]
        public void DiscoveryFeedState_DifferentIds_BothCount()
        {
            var feed = new DiscoveryFeedState(totalMinerals: 4);
            feed.Apply(MakeNotice(Guid.NewGuid(), "ferrosite", "inner-belt-1", Guid.NewGuid(), 1000, "LIVE"));
            feed.Apply(MakeNotice(Guid.NewGuid(), "glacine", "vela-orbit-1", Guid.NewGuid(), 2000, "LIVE"));

            TestContext.WriteLine("discovered=" + feed.DiscoveredCount + "/" + feed.TotalMinerals);
            Assert.That(feed.DiscoveredCount, Is.EqualTo(2));
            Assert.That(feed.TotalMinerals, Is.EqualTo(4));
        }

        /// <summary>
        /// Contract name (sprint contract §row 361, SC-110): DiscoveryList_KeysByActorIdNotLabel.
        /// Renamed from DiscoveryFeedState_CollidingPilotTags_DiscoverersStayDistinct_ByActorId
        /// (Phase 5 r1 name reconciliation, 2026-09-30) - assertions unchanged, only the method
        /// name.
        /// <para>
        /// SC-110 second half (qa round 3, team-lead 2026-09-27): PilotLabel_LastFourChars proves
        /// the two known-colliding dev ids produce the SAME display tag. This one proves that
        /// collision never lets two DIFFERENT discoverers get merged into one entry - the
        /// discovery list's real key is DiscovererActorId (a Guid), the tag is display-only
        /// (PilotTag.cs header, sprint contract section 0.11). DiscoveryFeedState.ByMineralId is
        /// keyed by mineral_id (not by tag or actor_id), so nothing here currently COULD merge on
        /// tag - this is a forward-looking regression guard, proven live below rather than just
        /// asserted by reading the source.
        /// </para>
        /// </summary>
        [Test]
        public void DiscoveryList_KeysByActorIdNotLabel()
        {
            // Same colliding pair as PilotLabel_LastFourChars (leader-confirmed real dev-id
            // collision, sprint contract section 0.11 / SC-110).
            Guid unityDefaultSubject = Guid.Parse("01a0b1c2-7e57-7c11-8e57-000000000001");
            Guid bot001 = Guid.Parse("01a0b1c2-b010-7000-8000-000000000001");

            string tagA = PilotTag.From(unityDefaultSubject);
            string tagB = PilotTag.From(bot001);
            Assert.That(tagA, Is.EqualTo(tagB), "positive control: these two actor_ids must collide on their display tag - otherwise this test proves nothing");

            var feed = new DiscoveryFeedState(totalMinerals: 4);
            feed.Apply(MakeNotice(Guid.NewGuid(), "ferrosite", "inner-belt-1", unityDefaultSubject, 1000, "LIVE"));
            feed.Apply(MakeNotice(Guid.NewGuid(), "glacine", "vela-orbit-1", bot001, 2000, "LIVE"));

            TestContext.WriteLine("discovered=" + feed.DiscoveredCount + " (two different minerals, two colliding-tag discoverers)");
            Assert.That(feed.DiscoveredCount, Is.EqualTo(2), "both discoveries must be recorded - neither the mineral-id key nor the collision should drop one");

            feed.TryGet("ferrosite", out DiscoveryEntry ferrositeEntry);
            feed.TryGet("glacine", out DiscoveryEntry glacineEntry);
            TestContext.WriteLine("ferrosite discoverer=" + ferrositeEntry.DiscovererActorId + " tag=" + PilotTag.From(ferrositeEntry.DiscovererActorId));
            TestContext.WriteLine("glacine discoverer=" + glacineEntry.DiscovererActorId + " tag=" + PilotTag.From(glacineEntry.DiscovererActorId));
            Assert.That(ferrositeEntry.DiscovererActorId, Is.Not.EqualTo(glacineEntry.DiscovererActorId),
                "the two discoverers must stay distinct by actor_id even though their display tags are identical");

            string summary = feed.FormatSummary(new Dictionary<string, string> { { "ferrosite", "Ferrosite" }, { "glacine", "Glacine" } });
            TestContext.WriteLine(summary);
            // FormatSummary's per-discovery line shape is "  {mineral name} - {tag} ({deposit_id})"
            // (DiscoveryFeedState.cs) - the tag sits in the middle, not at the start of the line.
            int discoveryLineCount = 0;
            foreach (string line in summary.Split('\n'))
                if (line.Contains(" - " + tagA + " (")) discoveryLineCount++;
            Assert.That(discoveryLineCount, Is.EqualTo(2), "the discovery list must show two separate lines for the colliding tag, one per deposit - not merged into one");

            // Negative control (team-lead request): show that grouping by the DISPLAY TAG instead
            // of actor_id (the hypothetical bug this whole test guards against) WOULD incorrectly
            // collapse these two into one group - without touching DiscoveryFeedState's real
            // production code (which is keyed by mineral_id, never by tag). If this assertion
            // failed, the positive control above ("tags collide") would not actually be true and
            // the rest of this test would not be exercising the collision case at all.
            var groupedByTagIfKeyedWrong = new Dictionary<string, List<Guid>>(StringComparer.Ordinal);
            foreach (DiscoveryEntry entry in new[] { ferrositeEntry, glacineEntry })
            {
                string tag = PilotTag.From(entry.DiscovererActorId);
                if (!groupedByTagIfKeyedWrong.TryGetValue(tag, out List<Guid> list))
                    groupedByTagIfKeyedWrong[tag] = list = new List<Guid>();
                list.Add(entry.DiscovererActorId);
            }
            TestContext.WriteLine("if a lookup were (incorrectly) keyed by display tag instead of actor_id: " +
                                  groupedByTagIfKeyedWrong.Count + " group(s) for 2 discoverers");
            Assert.That(groupedByTagIfKeyedWrong.Count, Is.EqualTo(1),
                "negative control: grouping by display tag SHOULD collapse the two colliding-tag discoverers into one group - " +
                "if this is not 1, the positive control's claimed collision is not real and the test above is not exercising SC-110.");
        }

        // ------------------------------------------------------------------ MiningCommandGateway (idempotent retry)

        [Test]
        public void MiningCommandGateway_RetryingSameDeposit_ReusesCommandId()
        {
            var gateway = new MiningCommandGateway();
            Guid first = gateway.ResolveCommandIdFor("far-reach");
            gateway.MarkSent(first, "far-reach");

            // "timeout, retry before any COMMAND_RESULT arrived" - same target, must reuse the id
            // (unity-client skill section 2: server dedupes by command_id).
            Guid retry = gateway.ResolveCommandIdFor("far-reach");
            TestContext.WriteLine("first=" + first + " retry=" + retry);
            Assert.That(retry, Is.EqualTo(first));
        }

        [Test]
        public void MiningCommandGateway_DifferentDeposit_GetsFreshCommandId()
        {
            var gateway = new MiningCommandGateway();
            Guid first = gateway.ResolveCommandIdFor("far-reach");
            gateway.MarkSent(first, "far-reach");

            Guid other = gateway.ResolveCommandIdFor("inner-belt-1");
            TestContext.WriteLine("first=" + first + " other-target=" + other);
            Assert.That(other, Is.Not.EqualTo(first));
        }

        [Test]
        public void MiningCommandGateway_TryResolve_OnlyClearsMatchingCommandId()
        {
            var gateway = new MiningCommandGateway();
            Guid mine = gateway.ResolveCommandIdFor("far-reach");
            gateway.MarkSent(mine, "far-reach");

            bool resolvedUnrelated = gateway.TryResolve(Guid.NewGuid());
            Assert.That(resolvedUnrelated, Is.False, "a COMMAND_RESULT for an unrelated command_id (e.g. SET_SHIP_CONTROL) must not be claimed");
            Assert.That(gateway.PendingCommandId, Is.EqualTo(mine), "the unrelated result must not clear this gateway's own pending state");

            bool resolvedOwn = gateway.TryResolve(mine);
            Assert.That(resolvedOwn, Is.True);
            Assert.That(gateway.PendingCommandId, Is.Null);

            // After resolution, the SAME deposit gets a FRESH id (previous attempt is done).
            Guid next = gateway.ResolveCommandIdFor("far-reach");
            Assert.That(next, Is.Not.EqualTo(mine));
        }

        // ------------------------------------------------------------------ MiningRangeEvaluator (C4, design doc §8 item 2)

        [Test]
        public void MiningRangeEvaluator_SurfaceDistance_SubtractsRadius()
        {
            // 중심 거리 300, 반지름 50 -> 표면 거리 250 (디자인 §4.2 #4 정의 그대로).
            MiningRangeStatus status = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 300.0, depositRadiusM: 50.0, shipSpeedMps: 0.0,
                rangeFromSurfaceM: 150.0, maxSpeedMps: 10.0);
            Assert.That(status.SurfaceDistanceM, Is.EqualTo(250.0));
            Assert.That(status.InRange, Is.False, "표면 거리 250 > 사거리 150");
        }

        [Test]
        public void MiningRangeEvaluator_RangeBoundary_ExactlyAtLimit_IsInRange()
        {
            // 서버 판정은 <=(mining.rs within_mining_range) - 경계값이 "사거리 안"이어야 서버와
            // 클라이언트 표시가 어긋나지 않는다.
            MiningRangeStatus atLimit = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 200.0, depositRadiusM: 50.0, shipSpeedMps: 0.0,
                rangeFromSurfaceM: 150.0, maxSpeedMps: 10.0);
            Assert.That(atLimit.SurfaceDistanceM, Is.EqualTo(150.0));
            Assert.That(atLimit.InRange, Is.True, "표면 거리 == 사거리 한계는 안쪽(<=)이어야 한다");

            MiningRangeStatus justOutside = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 200.0001, depositRadiusM: 50.0, shipSpeedMps: 0.0,
                rangeFromSurfaceM: 150.0, maxSpeedMps: 10.0);
            Assert.That(justOutside.InRange, Is.False, "경계 바로 바깥은 밖이어야 한다 - 음성 대조");
        }

        [Test]
        public void MiningRangeEvaluator_SpeedBoundary_ExactlyAtLimit_IsOk()
        {
            MiningRangeStatus atLimit = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 0.0, depositRadiusM: 0.0, shipSpeedMps: 10.0,
                rangeFromSurfaceM: 150.0, maxSpeedMps: 10.0);
            Assert.That(atLimit.SpeedOk, Is.True, "속도 == 상한은 허용(<=)이어야 한다");

            MiningRangeStatus justOver = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 0.0, depositRadiusM: 0.0, shipSpeedMps: 10.0001,
                rangeFromSurfaceM: 150.0, maxSpeedMps: 10.0);
            Assert.That(justOver.SpeedOk, Is.False, "경계 바로 위는 거부여야 한다 - 음성 대조");
        }

        [Test]
        public void MiningRangeEvaluator_ReadyToMine_RequiresBothInRangeAndSpeedOk()
        {
            var rangeOnly = new MiningRangeStatus(10.0, inRange: true, speedOk: false);
            var speedOnly = new MiningRangeStatus(10.0, inRange: false, speedOk: true);
            var both = new MiningRangeStatus(10.0, inRange: true, speedOk: true);

            Assert.That(rangeOnly.ReadyToMine, Is.False);
            Assert.That(speedOnly.ReadyToMine, Is.False);
            Assert.That(both.ReadyToMine, Is.True);
        }

        [Test]
        public void MiningRangeEvaluator_FormatStatusLines_MarksEachLineOkOrNg()
        {
            MiningRangeStatus status = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 100.0, depositRadiusM: 50.0, shipSpeedMps: 5.0,
                rangeFromSurfaceM: 150.0, maxSpeedMps: 10.0);
            string[] lines = MiningRangeEvaluator.FormatStatusLines(status, 5.0);
            TestContext.WriteLine(string.Join(" | ", lines));

            Assert.That(lines.Length, Is.EqualTo(3));
            Assert.That(lines[0], Does.Contain("표면 거리"));
            Assert.That(lines[1], Does.StartWith("[OK] "), "사거리 안이면 [OK]");
            Assert.That(lines[2], Does.StartWith("[OK] "), "속도 5 <= 상한 10이면 [OK]");

            MiningRangeStatus badStatus = MiningRangeEvaluator.Evaluate(
                distanceToCenterM: 100.0, depositRadiusM: 0.0, shipSpeedMps: 50.0,
                rangeFromSurfaceM: 10.0, maxSpeedMps: 10.0);
            string[] badLines = MiningRangeEvaluator.FormatStatusLines(badStatus, 50.0);
            Assert.That(badLines[1], Does.StartWith("[NG] "));
            Assert.That(badLines[2], Does.StartWith("[NG] "));
        }

        // ------------------------------------------------------------------ DistanceLabel (C4)

        [Test]
        public void DistanceLabel_Boundary_999IsMeters_1000IsKilometers()
        {
            Assert.That(DistanceLabel.Format(999.0), Is.EqualTo("999 m"));
            Assert.That(DistanceLabel.Format(1000.0), Is.EqualTo("1.00 km"));
            Assert.That(DistanceLabel.Format(6263.0), Is.EqualTo("6.26 km"), "Far Reach 스폰 거리 예시(design doc §1.3)");
        }

        // ------------------------------------------------------------------ CooldownDisplay (C4, design doc §8 item 3)

        [Test]
        public void CooldownDisplay_Boundary_ElapsedEqualsCooldown_IsExactlyZero()
        {
            Assert.That(CooldownDisplay.RemainingSeconds(3.0, 0.0), Is.EqualTo(3.0));
            Assert.That(CooldownDisplay.RemainingSeconds(3.0, 3.0), Is.EqualTo(0.0), "경과 == 쿨다운이면 정확히 0 (음수 아님)");
            Assert.That(CooldownDisplay.RemainingSeconds(3.0, 10.0), Is.EqualTo(0.0), "경과가 쿨다운을 넘어도 0에서 멈춘다");
        }

        [Test]
        public void CooldownDisplay_FormatOrNull_ZeroRemaining_IsNull_NotZeroText()
        {
            // 0초 쿨다운을 계속 그리면 "채굴 가능"과 구별이 안 된다(team-lead 지시) - 줄 자체가 없어야 한다.
            Assert.That(CooldownDisplay.FormatOrNull(0.0), Is.Null);
            Assert.That(CooldownDisplay.FormatOrNull(1.4), Is.EqualTo("쿨다운 1.4s"));
        }

        // ------------------------------------------------------------------ MiningNoticeClassifier (C4, design doc §8 item 5 / §3.4)

        [Test]
        public void MiningNoticeClassifier_SameActorId_IsOwnDiscovery()
        {
            Guid me = Guid.NewGuid();
            Assert.That(MiningNoticeClassifier.ClassifyDiscovery(me, me), Is.EqualTo(MiningNoticeKind.OwnDiscovery));
        }

        [Test]
        public void MiningNoticeClassifier_DifferentActorId_IsSystemWideDiscovery()
        {
            Assert.That(MiningNoticeClassifier.ClassifyDiscovery(Guid.NewGuid(), Guid.NewGuid()),
                Is.EqualTo(MiningNoticeKind.SystemWideDiscovery));
        }

        [Test]
        public void MiningNoticeClassifier_FormatBanner_OwnAndSystemWide_AreDistinctSentences()
        {
            string own = MiningNoticeClassifier.FormatBanner(MiningNoticeKind.OwnDiscovery, "Starfall Glass", "Pilot-0001", "far-reach");
            string systemWide = MiningNoticeClassifier.FormatBanner(MiningNoticeKind.SystemWideDiscovery, "Starfall Glass", "Pilot-0001", "far-reach");
            TestContext.WriteLine("own: " + own);
            TestContext.WriteLine("system-wide: " + systemWide);

            Assert.That(own, Is.Not.EqualTo(systemWide), "내 발견과 남의 발견 배너는 같은 사건이라도 다른 문장이어야 한다");
            Assert.That(own, Does.Contain("역사적 발견"));
            Assert.That(systemWide, Does.Contain("Pilot-0001"));

            string yield = MiningNoticeClassifier.FormatYieldNotice("Starfall Glass", 25);
            Assert.That(yield, Does.Not.Contain("발견"), "산출 알림은 '발견'이라는 단어를 쓰지 않는다 - 배너와 어휘로도 구별");
            Assert.That(own, Is.Not.EqualTo(yield));
            Assert.That(systemWide, Is.Not.EqualTo(yield));
        }

        // ------------------------------------------------------------------ DepositMarkerState (C4: "미확인 광맥" literal text, distance suffix)

        [Test]
        public void DepositMarkerState_FormatLine_Unrevealed_SaysExactLiteralPhrase()
        {
            var table = new DepositTableEntry { Id = "far-reach", DisplayName = "Far Reach", RadiusM = 40.0 };
            string line = DepositMarkerState.FormatLine(table, wire: null, discovery: null, mineralDisplayName: null);
            TestContext.WriteLine(line);
            Assert.That(line, Does.Contain("미확인 광맥"), "design doc §8 item 1 / SC-68 절차서 문구 그대로");
        }

        [Test]
        public void DepositMarkerState_FormatLineWithDistance_AppendsDistanceSuffix()
        {
            var table = new DepositTableEntry { Id = "far-reach", DisplayName = "Far Reach", RadiusM = 40.0 };
            string line = DepositMarkerState.FormatLineWithDistance(table, wire: null, discovery: null, mineralDisplayName: null, distanceToCenterM: 6263.0);
            TestContext.WriteLine(line);
            Assert.That(line, Does.Contain("미확인 광맥"));
            Assert.That(line, Does.EndWith("6.26 km"), "DistanceLabel.Format과 같은 포맷 - 같은 코드 경로");
        }
    }
}
