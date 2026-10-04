// EditMode tests for C3 (p1-02-mining) pure logic: Starfall.Mining's PilotTag, RejectReasonText,
// DiscoveryTiming, InventoryPanelState, DiscoveryFeedState, MiningCommandGateway. None of these
// need PlayMode - they take DTOs/plain values in and return plain values/state, same testing
// style as p1-01's ReconcileTickDrift/RebaseHold/etc (Starfall.Flight, pure + independently
// tested).

using System;
using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Contracts.Generated;
using Starfall.Mining;

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
    }
}
