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
        /// The 11 counter-examples the C# Strict profile is responsible for rejecting (sprint
        /// contract section 0.5, rows 1,3,4,5,6,9,11,13,16 plus 12 and 14, which only became
        /// the C# layer's responsibility once the generator honoured the actor_id narrowing).
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
        };

        /// <summary>
        /// The 5 counter-examples C# cannot see. Not a bug: a recorded design asymmetry.
        /// Guid parses any UUID version, long holds values above 2^53-1, int holds 0 where the
        /// schema says minimum 1, and a closed value set is a plain string in C# on purpose so
        /// that an added value does not make an older client drop whole messages (ADR-0005
        /// section 4). The schema validator and Rust serde stop all five at the server
        /// boundary.
        /// </summary>
        static readonly string[][] NotDetectableByCSharp =
        {
            new[] { "PING_SERVER", "command-id-not-v7.json" },
            new[] { "PING_REPLY", "tick-above-safe-integer.json" },
            new[] { "COMMAND_RESULT", "unknown-reason-code.json" },
            new[] { "SESSION_READY", "tick-hz-zero.json" },
            new[] { "SESSION_CLOSED", "unknown-close-reason.json" },
        };

        // ------------------------------------------------------------------ SC-23

        static IEnumerable<FixtureFile> ValidFixtureCases() => ContractFixtures.RequireValidFixtures();

        [TestCaseSource(nameof(ValidFixtureCases))]
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
        public void Fixtures_RoundTrip_VisitedEveryValidFixture()
        {
            IReadOnlyList<FixtureFile> fixtures = ContractFixtures.RequireValidFixtures();
            foreach (FixtureFile fixture in fixtures) TestContext.WriteLine("visited " + fixture);
            TestContext.WriteLine("valid fixtures round-tripped: " + fixtures.Count);

            Assert.That(fixtures.Count, Is.EqualTo(ContractFixtures.ExpectedValidFixtureCount),
                "The round-trip suite must cover every valid fixture the contract defines.");
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
            Assert.That(visited.Count, Is.EqualTo(11),
                "Sprint contract section 0.5 assigns exactly 11 of the 16 counter-examples to the C# layer.");

            // Every listed file must be distinct. Two types now share a file name, and a
            // lookup that collapsed them would test one file twice and still report 11.
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count),
                "The C# counter-example list must name 11 distinct files.");

            // The other five must still exist on disk, otherwise the split is only in prose.
            Assert.That(ContractFixtures.InvalidFixtures().Count,
                Is.EqualTo(ContractFixtures.ExpectedInvalidFixtureCount),
                "The contract defines " + ContractFixtures.ExpectedInvalidFixtureCount + " counter-examples in total.");
        }

        // ------------------------------------------------------------------ SC-25 (recorded, not a pass/fail of the design)

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
        public void NotDetectable_ListCoversExactlyFive()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in NotDetectableCases()) visited.Add(fixture.ToString());
            foreach (string name in visited) TestContext.WriteLine("recorded as undetectable in C#: " + name);
            TestContext.WriteLine("counter-examples C# is recorded as unable to detect: " + visited.Count);

            Assert.That(visited.Count, Is.EqualTo(5),
                "Sprint contract section 0.5 records exactly 5 undetectable counter-examples.");
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count));

            // 11 rejected + 5 recorded must account for every counter-example on disk. If they
            // do not, a fixture landed that nobody assigned to a layer.
            Assert.That(RejectedByCSharp.Length + NotDetectableByCSharp.Length,
                Is.EqualTo(ContractFixtures.ExpectedInvalidFixtureCount),
                "Every counter-example must be assigned to exactly one C# outcome.");
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
