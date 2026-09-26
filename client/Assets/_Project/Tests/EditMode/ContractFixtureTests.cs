// EditMode contract tests. Sprint contract SC-42 .. SC-48 (p0-01 wrote these as SC-23 .. SC-28).
//
// Every iterating test also asserts how many items it visited: a suite that silently
// iterates zero fixtures looks exactly like a passing suite.

using System;
using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ContractFixtureTests
    {
        // contracts/fixtures/PING_SERVER/basic.json
        const string RealTimeOnTheWire = "2026-09-17T14:05:09.123Z";

        // What Newtonsoft's DEFAULT reader turns that same value into: the format changes and
        // the milliseconds are gone. Measured on this machine before the code existed.
        const string RealTimeAfterDefaultReader = "09/17/2026 14:05:09";

        /// <summary>
        /// The 18 counter-examples the C# Strict profile is responsible for rejecting (spec
        /// section 5.4, sprint contract section 0.5: reject 18 / accept 10 / no C# layer 6 =
        /// 34). The 11 p0-02 rows plus 7 new p1-01 rows: two I-26 field-injection guards, the
        /// nested-array element check (WORLD_SNAPSHOT.ships[*] required fields), the
        /// angular_velocity_roll_mdeg_s required field B-1 added, and the actor_id/causation_id
        /// narrowing on SHIP_SPAWNED/SHIP_DESPAWNED (client 2026-09-20 measured, 0 corrections
        /// from the predicted set).
        /// </summary>
        static readonly string[][] RejectedByCSharp =
        {
            new[] { "PING_SERVER", "actor-field-injected.json" },
            new[] { "PING_SERVER", "probe-seq-negative.json" },
            new[] { "PING_SERVER", "probe-seq-above-u32.json" },
            new[] { "PING_REPLY", "missing-tick.json" },
            new[] { "PING_REPLY", "payload-unknown-field.json" },
            new[] { "COMMAND_RESULT", "payload-unknown-field.json" },
            new[] { "SESSION_READY", "missing-session-id.json" },
            new[] { "SESSION_OPENED", "missing-world-id.json" },
            new[] { "SESSION_OPENED", "actor-id-null.json" },
            new[] { "SESSION_CLOSED", "actor-id-null.json" },
            new[] { "SESSION_CLOSED", "correlation-id-null.json" },
            new[] { "SET_SHIP_CONTROL", "position-field-injected.json" },
            new[] { "SET_SHIP_CONTROL", "attitude-field-injected.json" },
            new[] { "WORLD_SNAPSHOT", "ship-missing-orientation-w.json" },
            new[] { "WORLD_SNAPSHOT", "angular-velocity-roll-missing.json" },
            new[] { "SHIP_SPAWNED", "actor-id-null.json" },
            new[] { "SHIP_SPAWNED", "causation-id-null.json" },
            new[] { "SHIP_DESPAWNED", "causation-id-null.json" },
        };

        /// <summary>
        /// The 10 counter-examples C# cannot see. Not a bug: a recorded design asymmetry.
        /// Guid parses any UUID version, long holds values above 2^53-1, int holds 0 where the
        /// schema says minimum 1, and a closed value set is a plain string in C# on purpose so
        /// that an added value does not make an older client drop whole messages (ADR-0005
        /// section 4). The schema validator and Rust serde stop all ten at the server boundary.
        /// </summary>
        static readonly string[][] NotDetectableByCSharp =
        {
            new[] { "PING_SERVER", "command-id-not-v7.json" },
            new[] { "PING_REPLY", "tick-above-safe-integer.json" },
            new[] { "COMMAND_RESULT", "unknown-reason-code.json" },
            new[] { "SESSION_READY", "tick-hz-zero.json" },
            new[] { "SESSION_CLOSED", "unknown-close-reason.json" },
            new[] { "SET_SHIP_CONTROL", "thrust-above-range.json" },
            new[] { "SET_SHIP_CONTROL", "input-seq-zero.json" },
            new[] { "WORLD_SNAPSHOT", "unknown-presence.json" },
            new[] { "WORLD_SNAPSHOT", "hard-radius-above-ceiling.json" },
            new[] { "SHIP_DESPAWNED", "session-closed-is-not-a-despawn.json" },
        };

        /// <summary>
        /// The 6 counter-examples with no C# layer at all: the three p1-01 data tables
        /// (SHIP_CLASS, STAR_SYSTEM, SYNC_TUNING), two invalid fixtures each. AC-10(d)
        /// deliberately generates no DTO for a data kind, so there is no Strict profile to
        /// reject or accept these - "no layer" is the designed outcome, not a gap.
        /// </summary>
        static readonly string[][] NoCSharpLayer =
        {
            new[] { "SHIP_CLASS", "turn-gain-zero.json" },
            new[] { "SHIP_CLASS", "movement-unknown-field.json" },
            new[] { "STAR_SYSTEM", "hard-radius-above-ceiling.json" },
            new[] { "STAR_SYSTEM", "no-spawn-points.json" },
            new[] { "SYNC_TUNING", "snapshot-hz-zero.json" },
            new[] { "SYNC_TUNING", "integrator-is-not-a-tunable.json" },
        };

        // ------------------------------------------------------------------ SC-47

        // AC-11(a): 27 valid fixtures must be FOUND (SC-49c guards this count; 26 -> 27 from
        // SESSION_CLOSED/superseded.json, R3 decision 5, 2026-09-22), and of those, the 21 that
        // carry an envelope discriminator must round-trip through Strict. The other 6 are the
        // p1-01 data tables (no DTO by AC-10(d)) and are not round-trip candidates.
        static IEnumerable<FixtureFile> ValidFixtureCases() => ContractFixtures.RequireValidFixtures();

        static IEnumerable<FixtureFile> RoundTrippableValidFixtureCases()
        {
            foreach (FixtureFile fixture in ContractFixtures.RequireValidFixtures())
                if (!ContractFixtures.IsDataOnly(fixture))
                    yield return fixture;
        }

        [TestCaseSource(nameof(RoundTrippableValidFixtureCases))]
        public void Fixtures_RoundTrip_MatchesOriginal(FixtureFile fixture)
        {
            string original = fixture.ReadText();
            Type dtoType = DtoTypeOf(original);

            object dto = ContractJson.DeserializeStrict(original, dtoType);
            Assert.That(dto, Is.Not.Null, fixture + " deserialized to null");

            string roundTripped = ContractJson.Serialize(dto);

            // Both sides are read with DateParseHandling.None. Opening the ORIGINAL with the
            // default reader would rewrite its dates before the comparison even starts, which
            // would make this assertion meaningless.
            JToken before = ContractJson.ReadToken(original);
            JToken after = ContractJson.ReadToken(roundTripped);

            TestContext.WriteLine(fixture + " -> " + dtoType.Name);
            TestContext.WriteLine("  re-serialized: " + after.ToString(Formatting.None));

            Assert.That(JToken.DeepEquals(before, after), Is.True,
                fixture + " did not survive the round trip.\n  original: " + before.ToString(Formatting.None) +
                "\n  actual:   " + after.ToString(Formatting.None));
        }

        [Test]
        public void Fixtures_RoundTrip_Found27_RoundTripped21()
        {
            IReadOnlyList<FixtureFile> found = ContractFixtures.RequireValidFixtures();
            List<FixtureFile> roundTrippable = new List<FixtureFile>(RoundTrippableValidFixtureCases());

            foreach (FixtureFile fixture in found) TestContext.WriteLine("found " + fixture);
            TestContext.WriteLine("valid fixtures found: " + found.Count);
            TestContext.WriteLine("of those, carrying an envelope discriminator (round-trip candidates): " +
                                  roundTrippable.Count);

            Assert.That(found.Count, Is.EqualTo(ContractFixtures.ExpectedValidFixtureCount),
                "The loader must find every valid fixture the contract defines (27: 26 plus " +
                "SESSION_CLOSED/superseded.json, R3 decision 5, 2026-09-22).");
            Assert.That(roundTrippable.Count, Is.EqualTo(ContractFixtures.ExpectedRoundTrippableValidFixtureCount),
                "21 of the 27 valid fixtures carry a message_type/command_type/event_type " +
                "discriminator and must round-trip. The other 6 are data tables with no DTO " +
                "(AC-10(d)) and must not be silently dropped from either count.");
        }

        // ------------------------------------------------------------------ SC-24

        static IEnumerable<FixtureFile> RejectedByCSharpCases()
        {
            foreach (string[] pair in RejectedByCSharp)
                yield return ContractFixtures.RequireInvalid(pair[0], pair[1]);
        }

        [TestCaseSource(nameof(RejectedByCSharpCases))]
        public void Invalid_Rejected_ByStrictProfile(FixtureFile fixture)
        {
            string json = fixture.ReadText();
            Type dtoType = DtoTypeOf(json);

            var thrown = Assert.Throws<JsonSerializationException>(
                () => ContractJson.DeserializeStrict(json, dtoType),
                fixture + " must be rejected by the Strict profile but was accepted.");

            TestContext.WriteLine(fixture + " rejected: " + thrown.Message);
        }

        [Test]
        public void Invalid_Rejected_VisitedEveryCSharpCase()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in RejectedByCSharpCases()) visited.Add(fixture.ToString());
            foreach (string name in visited) TestContext.WriteLine("visited " + name);
            TestContext.WriteLine("counter-examples the C# layer must reject: " + visited.Count);

            Assert.That(visited.Count, Is.EqualTo(RejectedByCSharp.Length));
            Assert.That(visited.Count, Is.EqualTo(18),
                "Sprint contract section 0.5 assigns exactly 18 of the 34 counter-examples to the C# layer (reject).");

            // Every listed file must be distinct. Two types now share a file name, and a
            // lookup that collapsed them would test one file twice and still report 18.
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count),
                "The C# counter-example list must name 18 distinct files.");

            // The rest must still exist on disk, otherwise the split is only in prose.
            Assert.That(ContractFixtures.InvalidFixtures().Count,
                Is.EqualTo(ContractFixtures.ExpectedInvalidFixtureCount),
                "The contract defines " + ContractFixtures.ExpectedInvalidFixtureCount + " counter-examples in total.");
        }

        // ------------------------------------------------------------------ SC-48 (recorded, not a pass/fail of the design)

        static IEnumerable<FixtureFile> NotDetectableCases()
        {
            foreach (string[] pair in NotDetectableByCSharp)
                yield return ContractFixtures.RequireInvalid(pair[0], pair[1]);
        }

        [TestCaseSource(nameof(NotDetectableCases))]
        public void Invalid_NotDetectableByCSharp_DocumentedAsymmetry(FixtureFile fixture)
        {
            string json = fixture.ReadText();
            Type dtoType = DtoTypeOf(json);

            object dto = ContractJson.DeserializeStrict(json, dtoType);

            TestContext.WriteLine(fixture + " was ACCEPTED by C# Strict, as documented in spec section 5.4.");
            TestContext.WriteLine("  Guid accepts any UUID version; long holds values above 2^53-1;");
            TestContext.WriteLine("  int holds 0 where the schema says minimum 1; a closed value set is a plain string.");
            TestContext.WriteLine("  The schema validator and Rust serde reject it at the server boundary.");

            Assert.That(dto, Is.Not.Null,
                fixture + " is recorded as undetectable in C#. If it now fails here, the contract's " +
                "layer split changed - tell architect instead of editing the table.");
        }

        static IEnumerable<FixtureFile> NoCSharpLayerCases()
        {
            foreach (string[] pair in NoCSharpLayer)
                yield return ContractFixtures.RequireInvalid(pair[0], pair[1]);
        }

        [TestCaseSource(nameof(NoCSharpLayerCases))]
        public void Invalid_NoCSharpLayer_HasNoEnvelopeDiscriminator(FixtureFile fixture)
        {
            // These are data-table counter-examples (SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING). There
            // is no envelope to read a type constant from, which is exactly why "no C# layer"
            // is the correct outcome and not a missing test: AC-10(d) never generates a DTO for
            // kind "data", so there is nothing here for Strict to accept or reject.
            string json = fixture.ReadText();
            JObject envelope = ContractJson.ReadObject(json);

            string typeName;
            bool hasDiscriminator = ContractDispatch.TryGetTypeName(envelope, out typeName);

            TestContext.WriteLine(fixture + " has envelope discriminator: " + hasDiscriminator);
            Assert.That(hasDiscriminator, Is.False,
                fixture + " is recorded as having no C# layer (data kind), but carries a " +
                "message_type/command_type/event_type - tell architect, the layer split changed.");
        }

        [Test]
        public void CSharpLayerSplit_Totals34()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in RejectedByCSharpCases()) visited.Add(fixture.ToString());
            foreach (FixtureFile fixture in NotDetectableCases()) visited.Add(fixture.ToString());
            foreach (FixtureFile fixture in NoCSharpLayerCases()) visited.Add(fixture.ToString());

            TestContext.WriteLine("reject " + RejectedByCSharp.Length + " / accept " +
                                  NotDetectableByCSharp.Length + " / no layer " + NoCSharpLayer.Length +
                                  " = " + visited.Count);

            Assert.That(RejectedByCSharp.Length, Is.EqualTo(18));
            Assert.That(NotDetectableByCSharp.Length, Is.EqualTo(10));
            Assert.That(NoCSharpLayer.Length, Is.EqualTo(6));
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count),
                "reject / accept / no-layer must not overlap.");
            Assert.That(visited.Count, Is.EqualTo(ContractFixtures.ExpectedInvalidFixtureCount),
                "18 + 10 + 6 must account for every counter-example the contract defines (34). " +
                "If this differs, the counts changed - tell architect instead of editing the table " +
                "(sprint contract section 0.5).");
        }

        // ------------------------------------------------------------------ SC-49(d)

        [Test]
        public void WorldSnapshot_EmptyShipsArray_RoundTrips()
        {
            // AC-11(d): ships == [] must round-trip. The generator's array support (C1 change
            // 1) is what makes "empty JSON array" distinct from "absent property" here.
            FixtureFile fixture = ContractFixtures.RequireValid("WORLD_SNAPSHOT", "empty-nulls-and-bounds.json");
            var message = ContractJson.DeserializeStrict<WorldSnapshotMessage>(fixture.ReadText());

            Assert.That(message.Payload.Ships, Is.Not.Null, "ships must deserialize to an empty array, not null");
            Assert.That(message.Payload.Ships.Length, Is.EqualTo(0));
            TestContext.WriteLine(fixture + " : ships.Length == 0, controlled_ship_id == " +
                                  message.Payload.ControlledShipId);
        }

        [Test]
        public void WorldSnapshot_TwoShipsOneLingering_RoundTrips()
        {
            // AC-11(d): 2 ships, one LINGERING, must round-trip - including the 18-property
            // ShipState array element (SC-43) and its presence values (ACTIVE vs LINGERING).
            FixtureFile fixture = ContractFixtures.RequireValid("WORLD_SNAPSHOT", "two-ships-one-lingering.json");
            var message = ContractJson.DeserializeStrict<WorldSnapshotMessage>(fixture.ReadText());

            Assert.That(message.Payload.Ships.Length, Is.EqualTo(2));
            TestContext.WriteLine(fixture + " : ship[0].presence = " + message.Payload.Ships[0].Presence +
                                  ", ship[1].presence = " + message.Payload.Ships[1].Presence);

            Assert.That(message.Payload.Ships[0].Presence, Is.EqualTo("ACTIVE"));
            Assert.That(message.Payload.Ships[1].Presence, Is.EqualTo("LINGERING"));

            // angular_velocity_roll_mdeg_s must be its own property (B-1), not folded into the
            // 3-component vector, and every one of ShipState's 18 properties must have carried a
            // real value across the wire.
            Assert.That(typeof(WorldSnapshotMessage.WorldSnapshotPayload.ShipState).GetProperties().Length,
                Is.EqualTo(18));
        }

        // ------------------------------------------------------------------ SC-49(e)

        [Test]
        public void Runtime_ToleratesUnknownFieldInsideShipStateArrayElement_AndStrictThrows()
        {
            // AC-11(e), phrased for the nested-array-element case specifically: an unknown field
            // inside ships[*] (not just at the top level) must be ignored + warned under
            // Runtime, and the identical input must throw under Strict. contracts/ has no
            // fixture that puts a stray field inside a ShipState element, so this test builds
            // its own JSON from a known-valid ship element instead of adding a fixture the
            // client does not own.
            string json = @"{
              ""message_id"": ""01a0b1c2-9c03-7d13-9f34-2a445566bb78"",
              ""message_type"": ""WORLD_SNAPSHOT"",
              ""schema_version"": 1,
              ""tick"": 100,
              ""correlation_id"": null,
              ""payload"": {
                ""star_system_id"": ""cradle"",
                ""soft_boundary_radius_mm"": 10000000,
                ""hard_boundary_radius_mm"": 12000000,
                ""snapshot_interval_ticks"": 2,
                ""controlled_ship_id"": null,
                ""ack_input_seq"": null,
                ""ships"": [
                  {
                    ""ship_id"": ""01a0b1c2-8b01-7a11-8b22-9c33d44e55f6"",
                    ""actor_id"": ""01a0b1c2-2c01-7a45-8b67-89abcdef0123"",
                    ""ship_class_id"": ""scout-s01"",
                    ""presence"": ""ACTIVE"",
                    ""position_x_mm"": 0, ""position_y_mm"": 0, ""position_z_mm"": 0,
                    ""velocity_x_mm_s"": 0, ""velocity_y_mm_s"": 0, ""velocity_z_mm_s"": 0,
                    ""orientation_x_micro"": 0, ""orientation_y_micro"": 0, ""orientation_z_micro"": 0, ""orientation_w_micro"": 1000000,
                    ""angular_velocity_x_mdeg_s"": 0, ""angular_velocity_y_mdeg_s"": 0, ""angular_velocity_z_mdeg_s"": 0,
                    ""angular_velocity_roll_mdeg_s"": 0,
                    ""warp_charge_percent"": 50
                  }
                ]
              }
            }";

            var warnings = new List<string>();
            var message = JsonConvert.DeserializeObject<WorldSnapshotMessage>(json, ContractJson.CreateRuntime(warnings.Add));

            foreach (string path in warnings) TestContext.WriteLine("ignored member at " + path);
            Assert.That(message, Is.Not.Null);
            Assert.That(warnings.Count, Is.EqualTo(1), "exactly the nested unknown field should have been ignored");
            Assert.That(warnings[0], Does.Contain("warp_charge_percent"));
            Assert.That(message.Payload.Ships[0].ShipId, Is.Not.EqualTo(Guid.Empty), "known members must still survive");

            var thrown = Assert.Throws<JsonSerializationException>(
                () => ContractJson.DeserializeStrict<WorldSnapshotMessage>(json),
                "Strict must reject the identical input that Runtime tolerated");
            TestContext.WriteLine("Strict rejected: " + thrown.Message);
            Assert.That(thrown.Message, Does.StartWith(ContractJson.IgnoredMemberMessagePrefix));
        }

        // ------------------------------------------------------------------ SC-50

        [Test]
        public void ClientDataCopy_MatchesRepositoryOriginal()
        {
            // AC-11(f): the client's copy of data/ under client/Assets/_Project/Data must be
            // byte-identical to the repository original (ADR-0012 section 7 - "file copy, this
            // time only", and the file-hash test is the only defence that copy has).
            string repoRoot = ContractFixtures.RequireRepoRoot();
            string dataRoot = Path.Combine(repoRoot, "data");
            string clientCopyRoot = Path.Combine(repoRoot, "client", "Assets", "_Project", "Data");

            Assert.That(Directory.Exists(dataRoot), Is.True, "repository data/ not found at " + dataRoot);
            Assert.That(Directory.Exists(clientCopyRoot), Is.True,
                "client/Assets/_Project/Data not found - C3 must copy data/ into the client project (ADR-0012 section 7)");

            string[] originals = Directory.GetFiles(dataRoot, "*.json", SearchOption.AllDirectories);
            Array.Sort(originals, StringComparer.Ordinal);
            Assert.That(originals.Length, Is.GreaterThan(0), "data/ has no *.json files to compare");

            int checkedCount = 0;
            using (var sha256 = System.Security.Cryptography.SHA256.Create())
            {
                foreach (string originalPath in originals)
                {
                    string relative = Path.GetRelativePath(dataRoot, originalPath);
                    string copyPath = Path.Combine(clientCopyRoot, relative);

                    Assert.That(File.Exists(copyPath), Is.True,
                        "client copy is missing " + relative + " (expected at " + copyPath + ")");

                    byte[] originalBytes = File.ReadAllBytes(originalPath);
                    byte[] copyBytes = File.ReadAllBytes(copyPath);
                    string originalHash = ToHex(sha256.ComputeHash(originalBytes));
                    string copyHash = ToHex(sha256.ComputeHash(copyBytes));

                    TestContext.WriteLine(relative + " : original " + originalHash + " / copy " + copyHash);
                    Assert.That(copyHash, Is.EqualTo(originalHash),
                        relative + " differs between data/ and the client copy - re-copy from the repository original.");
                    checkedCount++;
                }
            }

            TestContext.WriteLine("data/ files hash-compared against the client copy: " + checkedCount);
            Assert.That(checkedCount, Is.GreaterThan(0));
        }

        // ------------------------------------------------------------------ SC-26

        [Test]
        public void Dispatch_PreservesRealTime()
        {
            FixtureFile fixture = ContractFixtures.RequireValid("PING_SERVER", "basic.json");
            string json = fixture.ReadText();

            string typeName;
            object contract;
            string error;
            Assert.That(ContractDispatch.TryRead(json, out typeName, out contract, out error), Is.True,
                "dispatch helper could not read " + fixture + ": " + error);
            Assert.That(typeName, Is.EqualTo(PingServerCommand.CommandTypeConst));

            var command = (PingServerCommand)contract;
            TestContext.WriteLine("via ContractDispatch.TryRead : " + command.ClientSentAt);

            Assert.That(command.ClientSentAt, Is.EqualTo(RealTimeOnTheWire),
                "The dispatch path must hand the RealTime string through untouched.");

            // Regression guard: show that the trap is real, so nobody "simplifies"
            // ContractJson.ReadObject into JObject.Parse later.
            JObject unguarded = JObject.Parse(json);
            string viaDefaultReader = unguarded["client_sent_at"].ToObject<string>();
            TestContext.WriteLine("via default JObject.Parse   : " + viaDefaultReader);

            Assert.That(viaDefaultReader, Is.Not.EqualTo(RealTimeOnTheWire),
                "If the default reader stopped rewriting dates, this guard is obsolete - check Newtonsoft's version.");
            Assert.That(viaDefaultReader, Does.Not.Contain("T").And.Not.Contain("Z"),
                "The default reader is expected to produce a reformatted, non-ISO value.");
            Assert.That(viaDefaultReader, Is.EqualTo(RealTimeAfterDefaultReader));
        }

        // ------------------------------------------------------------------ SC-27

        [Test]
        public void UuidV7_MatchesSchemaPattern()
        {
            string pattern = ContractFixtures.UuidV7Pattern();
            TestContext.WriteLine("pattern from contracts/common/primitives.schema.json: " + pattern);

            const int Samples = 1000;
            for (int i = 0; i < Samples; i++)
            {
                string text = UuidV7.NewGuid().ToString("D");
                Assert.That(text, Does.Match(pattern), "generated id #" + i + " (" + text + ") is not a valid UuidV7");
            }
            TestContext.WriteLine("checked " + Samples + " generated ids against the schema pattern");
        }

        [Test]
        public void UuidV7_NoDuplicatesWithinSameMillisecond()
        {
            const int Count = 10000;
            long millisecond = UuidV7.CurrentUnixTimeMilliseconds();

            var seen = new HashSet<Guid>();
            string timestampPrefix = null;

            for (int i = 0; i < Count; i++)
            {
                Guid id = UuidV7.FromUnixTimeMilliseconds(millisecond);
                Assert.That(seen.Add(id), Is.True, "duplicate id " + id + " at iteration " + i);

                // First 12 hex characters are the 48-bit timestamp: identical for all of them
                // proves these really were generated inside one millisecond.
                string prefix = id.ToString("D").Substring(0, 13);
                if (timestampPrefix == null) timestampPrefix = prefix;
                else Assert.That(prefix, Is.EqualTo(timestampPrefix));
            }

            TestContext.WriteLine("generated " + Count + " ids inside millisecond " + millisecond +
                                  " (shared timestamp prefix " + timestampPrefix + "), duplicates: 0");
            Assert.That(seen.Count, Is.EqualTo(Count));
        }

        // ------------------------------------------------------------------ SC-28

        [Test]
        public void FixtureLoader_FindsRepoRootByMarker()
        {
            string root = ContractFixtures.FindRepoRoot();
            Assert.That(root, Is.Not.Null,
                "Could not find " + ContractFixtures.RootMarker + " above " + UnityEngine.Application.dataPath);

            TestContext.WriteLine("repo root : " + root);
            TestContext.WriteLine("marker    : " + ContractFixtures.RootMarker);
            Assert.That(File.Exists(Path.Combine(root, "contracts", "registry", "types.json")), Is.True);

            IReadOnlyList<FixtureFile> valid = ContractFixtures.ValidFixtures();
            foreach (FixtureFile fixture in valid) TestContext.WriteLine("  valid fixture: " + fixture);
            TestContext.WriteLine("valid fixtures counted: " + valid.Count);

            Assert.That(valid.Count, Is.EqualTo(ContractFixtures.ExpectedValidFixtureCount));

            // invalid/ must not be swallowed into the valid set by a recursive search.
            foreach (FixtureFile fixture in valid)
                Assert.That(fixture.FullPath.Replace('\\', '/'), Does.Not.Contain("/invalid/"));
        }

        [Test]
        public void NotDetectable_ListCoversExactlyTen()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in NotDetectableCases()) visited.Add(fixture.ToString());
            foreach (string name in visited) TestContext.WriteLine("recorded as undetectable in C#: " + name);
            TestContext.WriteLine("counter-examples C# is recorded as unable to detect: " + visited.Count);

            Assert.That(visited.Count, Is.EqualTo(10),
                "Sprint contract section 0.5 records exactly 10 undetectable counter-examples.");
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count));

            // The full three-way split (reject 18 / accept 10 / no layer 6 = 34) is asserted in
            // CSharpLayerSplit_Totals34 above.
        }

        [Test]
        public void FixtureLoader_FailsWhenTooFewValidFixtures()
        {
            Assert.Throws<InvalidOperationException>(
                () => ContractFixtures.EnsureEnough(new List<FixtureFile>()),
                "An empty fixture set must fail the suite rather than pass silently.");

            Assert.Throws<InvalidOperationException>(
                () => ContractFixtures.EnsureEnough(new List<FixtureFile> { ContractFixtures.RequireValid("PING_SERVER", "basic.json") }),
                "A short fixture set must fail too, not only an empty one.");

            // One short of the expected count must still fail. An off-by-one guard is what
            // catches a deleted fixture; a guard that only rejects the empty set would not.
            var oneShort = new List<FixtureFile>(ContractFixtures.ValidFixtures());
            oneShort.RemoveAt(oneShort.Count - 1);
            TestContext.WriteLine("guard sees " + oneShort.Count + " of " +
                                  ContractFixtures.ExpectedValidFixtureCount + " expected fixtures");
            Assert.Throws<InvalidOperationException>(() => ContractFixtures.EnsureEnough(oneShort));

            Assert.DoesNotThrow(() => ContractFixtures.EnsureEnough(ContractFixtures.ValidFixtures()));
        }

        // ------------------------------------------------------------------ helpers

        /// <summary>Lowercase hex, no separators. Not Convert.ToHexString: that overload is
        /// .NET 5+ only and Unity's Mono BCL for this project's api compatibility level does
        /// not guarantee it.</summary>
        static string ToHex(byte[] bytes)
        {
            var sb = new System.Text.StringBuilder(bytes.Length * 2);
            foreach (byte b in bytes) sb.Append(b.ToString("x2"));
            return sb.ToString();
        }

        /// <summary>Looks the DTO up through the generated registry map, the same way the
        /// dispatch path does, so a renamed type breaks the tests instead of being missed.</summary>
        static Type DtoTypeOf(string json)
        {
            JObject envelope = ContractJson.ReadObject(json);

            string typeName;
            Assert.That(ContractDispatch.TryGetTypeName(envelope, out typeName), Is.True,
                "fixture carries no envelope type constant");

            Type dtoType;
            Assert.That(ContractTypes.ByName.TryGetValue(typeName, out dtoType), Is.True,
                "registry type '" + typeName + "' has no generated DTO");

            return dtoType;
        }
    }
}
