// EditMode contract tests. Sprint contract items SC-23 .. SC-28.
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

        /// <summary>Counter-examples the C# Strict profile is responsible for (spec section 5).</summary>
        static readonly string[] RejectedByCSharp =
        {
            "actor-field-injected.json",
            "probe-seq-negative.json",
            "probe-seq-above-u32.json",
            "missing-tick.json",
            "payload-unknown-field.json",
        };

        /// <summary>Counter-examples C# cannot see. Not a bug: a recorded design asymmetry.
        /// Guid parses any UUID version, and long holds values above 2^53-1. The schema
        /// validator and Rust serde stop these at the server boundary.</summary>
        static readonly string[] NotDetectableByCSharp =
        {
            "command-id-not-v7.json",
            "tick-above-safe-integer.json",
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
        public void Fixtures_RoundTrip_VisitedAllFourValidFixtures()
        {
            IReadOnlyList<FixtureFile> fixtures = ContractFixtures.RequireValidFixtures();
            foreach (FixtureFile fixture in fixtures) TestContext.WriteLine("visited " + fixture);

            Assert.That(fixtures.Count, Is.EqualTo(ContractFixtures.ExpectedValidFixtureCount),
                "The round-trip suite must cover every valid fixture the contract defines.");
        }

        // ------------------------------------------------------------------ SC-24

        static IEnumerable<FixtureFile> RejectedByCSharpCases()
        {
            foreach (string name in RejectedByCSharp) yield return ContractFixtures.RequireInvalid(name);
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
        public void Invalid_Rejected_VisitedAllFiveCSharpCases()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in RejectedByCSharpCases()) visited.Add(fixture.ToString());
            foreach (string name in visited) TestContext.WriteLine("visited " + name);

            Assert.That(visited.Count, Is.EqualTo(5),
                "Spec section 5 assigns exactly 5 of the 7 counter-examples to the C# layer.");

            // The other two must still exist on disk, otherwise the split is only in prose.
            Assert.That(ContractFixtures.InvalidFixtures().Count, Is.EqualTo(7),
                "The contract defines 7 counter-examples in total.");
        }

        // ------------------------------------------------------------------ SC-25 (recorded, not a pass/fail of the design)

        static IEnumerable<FixtureFile> NotDetectableCases()
        {
            foreach (string name in NotDetectableByCSharp) yield return ContractFixtures.RequireInvalid(name);
        }

        [TestCaseSource(nameof(NotDetectableCases))]
        public void Invalid_NotDetectableByCSharp_DocumentedAsymmetry(FixtureFile fixture)
        {
            string json = fixture.ReadText();
            Type dtoType = DtoTypeOf(json);

            object dto = ContractJson.DeserializeStrict(json, dtoType);

            TestContext.WriteLine(fixture + " was ACCEPTED by C# Strict, as documented in spec section 5.");
            TestContext.WriteLine("  Guid accepts any UUID version; long holds values above 2^53-1.");
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
        public void FixtureLoader_FailsWhenFewerThanFourValidFixtures()
        {
            Assert.Throws<InvalidOperationException>(
                () => ContractFixtures.EnsureEnough(new List<FixtureFile>()),
                "An empty fixture set must fail the suite rather than pass silently.");

            Assert.Throws<InvalidOperationException>(
                () => ContractFixtures.EnsureEnough(new List<FixtureFile> { ContractFixtures.RequireValid("PING_SERVER", "basic.json") }),
                "A short fixture set must fail too, not only an empty one.");

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
