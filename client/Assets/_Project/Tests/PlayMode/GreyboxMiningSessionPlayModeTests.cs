// PlayMode tests for C3 (p1-02-mining). team-lead ruling (2026-09-27): the prior client's signed
// sprint contract r2 assigns SC-70 (DiscoveryFeed_DedupesByHistoricalEventId) and SC-71(B)
// (DepositMarker_FollowsMessageNotDataCopy) to "unity test client --mode PlayMode" with a
// FakeTransport - both are reachable by fixture replay, no live server needed.
//
// Unlike MiningViewModelTests.cs (EditMode, Starfall.Mining classes constructed directly in C#),
// these tests drive the REAL wire path: FakeRealtimeTransport -> RealtimeClient.OnMessage ->
// ContractDispatch.TryRead -> the handler GreyboxMiningSession.Init() registered. That dispatch
// path is exactly the integration risk EditMode's direct-object-construction tests cannot see.

using System;
using System.Collections.Generic;
using System.IO;
using NUnit.Framework;
using Starfall.Greybox;
using Starfall.Net;
using UnityEngine;

namespace Starfall.Tests.PlayMode
{
    [TestFixture]
    public sealed class GreyboxMiningSessionPlayModeTests
    {
        FakeRealtimeTransport _transport;
        RealtimeClient _client;
        GameObject _go;
        GreyboxMiningSession _mining;

        [SetUp]
        public void SetUp()
        {
            _transport = new FakeRealtimeTransport();
            _client = new RealtimeClient(_transport, NullLogSink.Instance, jitterSeed: 1234);
            _go = new GameObject("GreyboxMiningSessionPlayModeTests");
            _mining = _go.AddComponent<GreyboxMiningSession>();
        }

        [TearDown]
        public void TearDown()
        {
            _client?.Dispose();
            if (_go != null) UnityEngine.Object.Destroy(_go);
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

        static string FixturePath(string typeName, string fileName) =>
            Path.Combine(RepoRoot(), "contracts", "fixtures", typeName, fileName);

        // ------------------------------------------------------------------ SC-70

        [Test]
        public void DiscoveryFeed_DedupesByHistoricalEventId()
        {
            // 02_client_ack.md C1 finding: contracts/fixtures/ has no fixture pair sharing a
            // historical_event_id (live.json and backfill.json are two DIFFERENT discoveries) -
            // qa-agreed synthetic input is backfill.json's text with its historical_event_id
            // replaced by live.json's, so the two notices are otherwise distinct messages that
            // must still dedupe on the id alone.
            string liveJson = File.ReadAllText(FixturePath("HISTORICAL_EVENT_NOTICE", "live.json"));
            string backfillJson = File.ReadAllText(FixturePath("HISTORICAL_EVENT_NOTICE", "backfill.json"));

            string liveId = ExtractHistoricalEventId(liveJson);
            string backfillId = ExtractHistoricalEventId(backfillJson);
            Assert.That(liveId, Is.Not.EqualTo(backfillId), "positive control: the two source fixtures must start with DIFFERENT historical_event_ids (otherwise this test proves nothing)");

            string backfillWithLiveId = backfillJson.Replace(backfillId, liveId);
            Assert.That(ExtractHistoricalEventId(backfillWithLiveId), Is.EqualTo(liveId),
                "the synthetic substitution must actually have taken (positive control on the input itself)");

            _mining.Init(_client, "cradle");
            _transport.SimulateOpen();
            _client.Pump();

            _transport.SimulateMessage(liveJson);
            _client.Pump();
            Assert.That(_mining.DiscoveriesForTests.DiscoveredCount, Is.EqualTo(1), "the LIVE notice must register as a new discovery");

            _transport.SimulateMessage(backfillWithLiveId);
            _client.Pump();

            TestContext.WriteLine("discovered after LIVE + BACKFILL(same id) = " + _mining.DiscoveriesForTests.DiscoveredCount);
            Assert.That(_mining.DiscoveriesForTests.DiscoveredCount, Is.EqualTo(1),
                "a BACKFILL notice carrying the SAME historical_event_id as an already-seen LIVE notice must not be counted twice - " +
                "this is the real RealtimeClient dispatch path, not a direct C# call into DiscoveryFeedState.");
        }

        static string ExtractHistoricalEventId(string json)
        {
            const string marker = "\"historical_event_id\": \"";
            int start = json.IndexOf(marker, StringComparison.Ordinal);
            Assert.That(start, Is.GreaterThanOrEqualTo(0), "fixture has no historical_event_id field - fixture shape changed?");
            start += marker.Length;
            int end = json.IndexOf('"', start);
            return json.Substring(start, end - start);
        }

        // ------------------------------------------------------------------ SC-71(B)

        [Test]
        public void DepositMarker_FollowsMessageNotDataCopy()
        {
            // qa r1's exact concern (02_client_ack.md): "사본 값 = 메시지 값이라 출처 구분 불가"
            // (if the local copy's value happens to equal the message's value, you cannot tell
            // whether the display read the message or the copy). The real client copy of
            // far-reach is "starfall-glass" - the SAME value the canonical revealed-and-depleted
            // .json fixture carries - so the two sources must be made to genuinely disagree.
            //
            // qa round 4 correction (2026-09-27) of this test's OWN round-3 version: that version
            // swapped the LOCAL COPY (via dataOverride, to avoid the round-2 file-write risk), but
            // DepositTableEntry has no field to carry a mineral into at all (Sim/DepositFieldTable
            // .cs) - so parsing the swapped copy text and the real copy text into a
            // DepositFieldTable produces the SAME DepositFieldTable either way. The swap silently
            // evaporated at parse time, and the two sources (copy on disk vs. message) were left
            // agreeing again (starfall-glass both ways) - a vacuous pass, exactly the ⊘ state the
            // sprint contract warns about. qa's fix, adopted here: swap the MESSAGE instead. The
            // session loads the REAL, unmodified data copy (dataOverride unused in this test -
            // the real GreyboxDataLoader.LoadMining path runs, reading files but never writing
            // any), and only the incoming DEPOSIT_FIELD_STATE JSON is altered in memory before
            // being handed to the fake transport. If any code path read the real copy's
            // "starfall-glass" instead of the (swapped) message's "cobaltine", it would leak
            // through as "Starfall Glass" and this test goes red.
            string copyPath = Path.Combine(RepoRoot(), "client", "Assets", "_Project", "Data", "world", "deposits", "cradle.json");
            string realCopyText = File.ReadAllText(copyPath); // read-only - never written
            Assert.That(realCopyText, Does.Contain("\"mineral_id\": \"starfall-glass\""),
                "input control: the real client copy's far-reach entry must actually say starfall-glass - " +
                "this is what makes the swap below meaningful (otherwise the two sources never agreed to begin with).");

            string revealedJsonOriginal = File.ReadAllText(FixturePath("DEPOSIT_FIELD_STATE", "revealed-and-depleted.json"));
            Assert.That(revealedJsonOriginal, Does.Contain("\"mineral_id\": \"starfall-glass\""),
                "input control: the canonical fixture's far-reach entry must ALSO say starfall-glass - " +
                "same value as the real copy, which is exactly why the swap has to happen on this side, not the copy's.");

            string revealedJsonSwapped = revealedJsonOriginal.Replace("\"mineral_id\": \"starfall-glass\"", "\"mineral_id\": \"cobaltine\"");
            Assert.That(revealedJsonSwapped, Is.Not.EqualTo(revealedJsonOriginal), "the in-memory message swap must actually change the text");

            _mining.Init(_client, "cradle"); // real loader, real (unmodified) data copy - no override, no disk writes
            _transport.SimulateOpen();
            _client.Pump();

            string unrevealedJson = File.ReadAllText(FixturePath("DEPOSIT_FIELD_STATE", "all-unrevealed.json"));
            _transport.SimulateMessage(unrevealedJson);
            _client.Pump();

            string unrevealedLine = _mining.MarkerLineForTests("far-reach");
            TestContext.WriteLine("far-reach after all-unrevealed: " + unrevealedLine);
            Assert.That(unrevealedLine, Does.Contain("미확인"));
            Assert.That(unrevealedLine, Does.Not.Contain("Starfall Glass").And.Not.Contain("Cobaltine"),
                "an unrevealed deposit must show no mineral at all, from either source");

            _transport.SimulateMessage(revealedJsonSwapped);
            _client.Pump();

            string revealedLine = _mining.MarkerLineForTests("far-reach");
            TestContext.WriteLine("far-reach after revealed-and-depleted (message swapped to cobaltine): " + revealedLine);
            // "Cobaltine" is the DISPLAY NAME (MineralCatalog, from the public
            // data/minerals/cobaltine.json) the marker resolves the SWAPPED MESSAGE's mineral_id
            // through - proof the display followed the message.
            Assert.That(revealedLine, Does.Contain("Cobaltine"),
                "the marker must show the SWAPPED MESSAGE's mineral (cobaltine -> display name 'Cobaltine')");
            Assert.That(revealedLine, Does.Not.Contain("Starfall Glass"),
                "the marker must NOT show the real data copy's mineral (starfall-glass -> 'Starfall Glass') - " +
                "if it did, some code path read the copy on disk instead of this message.");

            // This test only ever read copyPath; it must still be exactly what it was.
            Assert.That(File.ReadAllText(copyPath), Is.EqualTo(realCopyText),
                "sanity check: this test must not have touched the real client data copy at all");
        }
    }
}
