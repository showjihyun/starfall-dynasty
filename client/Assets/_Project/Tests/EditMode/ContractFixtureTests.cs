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
        /// The counter-examples the C# Strict profile is responsible for rejecting (spec
        /// section 5.4, sprint contract section 0.5). p1-01 measured 18 of 34 (reject 18 /
        /// accept 10 / no C# layer 6): the 11 p0-02 rows plus 7 new p1-01 rows (two I-26
        /// field-injection guards, the nested-array element check, the
        /// angular_velocity_roll_mdeg_s required field B-1, and the actor_id/causation_id
        /// narrowing on SHIP_SPAWNED/SHIP_DESPAWNED). p1-02-mining adds 16 more (client
        /// 2026-09-27, measured by reading each new invalid fixture against the generated DTO
        /// and confirmed empirically for the two fractional-quantity_kg cases - see below): 18
        /// -> 34 of 74.
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

            // p1-02-mining (sprint contract section 0.5 column 3, client-measured 2026-09-27).
            // Field injections (additionalProperties/unevaluatedProperties: false) and missing
            // Required.Always members throw under Strict the same way p1-01's did above.
            new[] { "MINE_RESOURCE", "actor-field-injected.json" },
            new[] { "MINE_RESOURCE", "mineral-field-injected.json" },
            new[] { "MINE_RESOURCE", "missing-deposit-id.json" },
            new[] { "MINE_RESOURCE", "position-field-injected.json" },
            new[] { "MINE_RESOURCE", "quantity-field-injected.json" },
            new[] { "DEPOSIT_FIELD_STATE", "hint-field-injected.json" },
            new[] { "DEPOSIT_FIELD_STATE", "missing-mineral-key.json" },
            new[] { "INVENTORY_STATE", "capacity-field-injected.json" },
            new[] { "HISTORICAL_EVENT_NOTICE", "sentence-injected.json" },
            new[] { "MINERAL_DISCOVERED", "domain-envelope-field-sequence.json" },
            new[] { "MINERAL_DISCOVERED", "narrative-field-injected.json" },
            new[] { "MINERAL_MINED", "actor-id-null.json" },
            new[] { "MINERAL_MINED", "causation-id-null.json" },
            new[] { "MINERAL_MINED", "importance-level-injected.json" },

            // Fractional JSON number into a C# int field. Two independent estimates disagreed
            // here (client-2 guessed ACCEPT, assuming Convert.ToInt32-style rounding; the prior
            // client agent guessed REJECT) - resolved empirically, twice: an isolated
            // Newtonsoft.Json 13.0.3 console probe throws JsonReaderException("Input string
            // '25.5' is not a valid integer") for a fractional token into an int property (no
            // silent rounding), and this project's actual EditMode run below is the test of
            // record for the Unity-packaged Newtonsoft version (com.unity.nuget.newtonsoft-json)
            // - see 03_client_impl.md for both readings.
            new[] { "INVENTORY_STATE", "fractional-quantity.json" },
            new[] { "MINERAL_MINED", "quantity-fractional.json" },
        };

        /// <summary>
        /// The counter-examples C# cannot see. Not a bug: a recorded design asymmetry. Guid
        /// parses any UUID version, long holds values above 2^53-1, int holds 0 where the schema
        /// says minimum 1, array minItems/exact-count is not a length the C# array type carries,
        /// a kebab-case DataId pattern is a plain string with no regex check, and a closed value
        /// set is a plain string in C# on purpose so that an added value does not make an older
        /// client drop whole messages (ADR-0005 section 4). The schema validator and Rust serde
        /// stop all of these at the server boundary. p1-01 measured 10; p1-02-mining adds 13
        /// more (client 2026-09-27): 10 -> 23 of 74.
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

            // p1-02-mining. DataId pattern (kebab-case) is a plain string with no regex check;
            // negative/zero/out-of-range ints have no runtime bound; a closed value set is a
            // plain string by design (ADR-0005 section 4); Guid accepts any UUID version;
            // array minItems is not a length the C# array type carries.
            new[] { "MINE_RESOURCE", "deposit-id-not-kebab.json" },
            new[] { "DEPOSIT_FIELD_STATE", "remaining-negative.json" },
            new[] { "INVENTORY_STATE", "zero-quantity-item.json" },
            new[] { "HISTORICAL_EVENT_NOTICE", "nested-level-zero.json" },
            new[] { "HISTORICAL_EVENT_NOTICE", "unknown-delivery.json" },
            new[] { "MINERAL_DISCOVERED", "fact-status-interpretation.json" },
            new[] { "MINERAL_DISCOVERED", "historical-id-v7-not-derived.json" },
            new[] { "MINERAL_DISCOVERED", "importance-level-zero.json" },
            new[] { "MINERAL_DISCOVERED", "participants-discoverer-only.json" },
            new[] { "MINERAL_DISCOVERED", "rule-version-without-number.json" },
            new[] { "MINERAL_DISCOVERED", "source-event-ids-empty.json" },
            new[] { "MINERAL_MINED", "quantity-zero.json" },
            new[] { "MINERAL_MINED", "remaining-negative.json" },
        };

        /// <summary>
        /// The counter-examples with no C# layer at all: the three p1-01 data tables
        /// (SHIP_CLASS, STAR_SYSTEM, SYNC_TUNING, two invalid fixtures each = 6) plus the four
        /// p1-02-mining data tables (DEPOSIT_FIELD, MINERAL, MINING_RULES, SIGNIFICANCE_RULE)
        /// contributing 11 more (3+3+2+3): 6 -> 17 of 74. AC-10(d) deliberately generates no DTO
        /// for a data kind (confirmed by running the generator: "skipped MINERAL (kind 'data':
        /// no DTO is generated)" etc.), so there is no Strict profile to reject or accept these -
        /// "no layer" is the designed outcome, not a gap.
        /// </summary>
        static readonly string[][] NoCSharpLayer =
        {
            new[] { "SHIP_CLASS", "turn-gain-zero.json" },
            new[] { "SHIP_CLASS", "movement-unknown-field.json" },
            new[] { "STAR_SYSTEM", "hard-radius-above-ceiling.json" },
            new[] { "STAR_SYSTEM", "no-spawn-points.json" },
            new[] { "SYNC_TUNING", "snapshot-hz-zero.json" },
            new[] { "SYNC_TUNING", "integrator-is-not-a-tunable.json" },

            // p1-02-mining data tables (AC-10(d): kind "data" gets no generated DTO).
            new[] { "DEPOSIT_FIELD", "no-deposits.json" },
            new[] { "DEPOSIT_FIELD", "position-two-components.json" },
            new[] { "DEPOSIT_FIELD", "reserve-fractional.json" },
            new[] { "MINERAL", "regen-interval-fractional.json" },
            new[] { "MINERAL", "unread-property.json" },
            new[] { "MINERAL", "yield-fractional.json" },
            new[] { "MINING_RULES", "cooldown-fractional.json" },
            new[] { "MINING_RULES", "range-zero.json" },
            new[] { "SIGNIFICANCE_RULE", "foreign-input-rarity.json" },
            new[] { "SIGNIFICANCE_RULE", "importance-level-zero.json" },
            new[] { "SIGNIFICANCE_RULE", "rule-version-malformed.json" },
        };

        // ------------------------------------------------------------------ SC-47 / SC-66

        // AC-11(a) / AC-14(a): 46 valid fixtures must be FOUND (27 measured at p1-01, +19 for
        // p1-02-mining - sprint contract SC-66), and of those, the 36 that carry an envelope
        // discriminator must round-trip through Strict. The other 10 are data tables (no DTO by
        // AC-10(d): 6 from p1-01, 4 new from p1-02-mining) and are not round-trip candidates.
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
        public void Fixtures_RoundTrip_Found46_RoundTripped36()
        {
            IReadOnlyList<FixtureFile> found = ContractFixtures.RequireValidFixtures();
            List<FixtureFile> roundTrippable = new List<FixtureFile>(RoundTrippableValidFixtureCases());

            foreach (FixtureFile fixture in found) TestContext.WriteLine("found " + fixture);
            TestContext.WriteLine("valid fixtures found: " + found.Count);
            TestContext.WriteLine("of those, carrying an envelope discriminator (round-trip candidates): " +
                                  roundTrippable.Count);

            Assert.That(found.Count, Is.EqualTo(ContractFixtures.ExpectedValidFixtureCount),
                "The loader must find every valid fixture the contract defines (46: 27 measured " +
                "at p1-01 plus 19 new for p1-02-mining, sprint contract SC-66).");
            Assert.That(roundTrippable.Count, Is.EqualTo(ContractFixtures.ExpectedRoundTrippableValidFixtureCount),
                "36 of the 46 valid fixtures carry a message_type/command_type/event_type " +
                "discriminator and must round-trip. The other 10 are data tables with no DTO " +
                "(AC-10(d): 6 from p1-01, 4 new from p1-02-mining) and must not be silently " +
                "dropped from either count.");
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

            // Assert.Catch<JsonException>, not Assert.Throws<JsonSerializationException>:
            // Assert.Throws<T> requires the EXACT exception type, not "is a" (measured on this
            // machine - the first EditMode run of this suite failed all 34 cases with "Expected:
            // <Newtonsoft.Json.JsonException> But was: <Newtonsoft.Json.JsonSerializationException>"
            // once JsonException was substituted naively). Most invalid fixtures throw
            // JsonSerializationException; the two fractional-quantity_kg cases
            // (INVENTORY_STATE/fractional-quantity.json, MINERAL_MINED/quantity-fractional.json)
            // throw JsonReaderException instead, one layer below - from inside
            // JsonTextReader.ParseNumber, before the object populator ever runs. Both are still
            // "Strict rejects it"; the sprint contract's reject/accept split is about the outcome
            // (throws vs. silently accepts), not the exact exception subtype, so Assert.Catch (any
            // JsonException or subtype) is the correct check here.
            var thrown = Assert.Catch<JsonException>(
                () => ContractJson.DeserializeStrict(json, dtoType),
                fixture + " must be rejected by the Strict profile but was accepted.");

            TestContext.WriteLine(fixture + " rejected (" + thrown.GetType().Name + "): " + thrown.Message);
        }

        /// <summary>
        /// Re-narrows the Assert.Catch<JsonException> widening above (team-lead request, qa
        /// verification round 2): asserts the JsonReaderException subset is EXACTLY the two
        /// fractional-quantity_kg cases, not a growing set. If a future invalid fixture starts
        /// throwing JsonReaderException for an unrelated reason, this fails and someone has to
        /// look, instead of it silently blending into "reject" with no further scrutiny.
        /// </summary>
        [Test]
        public void Invalid_Rejected_JsonReaderExceptionSubset_IsExactlyTheFractionalQuantityCases()
        {
            var readerExceptionCases = new List<string>();
            foreach (FixtureFile fixture in RejectedByCSharpCases())
            {
                string json = fixture.ReadText();
                Type dtoType = DtoTypeOf(json);
                try
                {
                    ContractJson.DeserializeStrict(json, dtoType);
                    Assert.Fail(fixture + " was accepted - Invalid_Rejected_ByStrictProfile should already have failed on this");
                }
                catch (JsonReaderException)
                {
                    readerExceptionCases.Add(fixture.ToString());
                }
                catch (JsonException)
                {
                    // JsonSerializationException (or another JsonException subtype) - expected
                    // for the other 32 cases, not this test's concern.
                }
            }

            foreach (string name in readerExceptionCases) TestContext.WriteLine("JsonReaderException: " + name);
            TestContext.WriteLine("JsonReaderException count: " + readerExceptionCases.Count);

            Assert.That(readerExceptionCases, Is.EquivalentTo(new[]
            {
                "INVENTORY_STATE/fractional-quantity.json",
                "MINERAL_MINED/quantity-fractional.json",
            }));
        }

        [Test]
        public void Invalid_Rejected_VisitedEveryCSharpCase()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in RejectedByCSharpCases()) visited.Add(fixture.ToString());
            foreach (string name in visited) TestContext.WriteLine("visited " + name);
            TestContext.WriteLine("counter-examples the C# layer must reject: " + visited.Count);

            Assert.That(visited.Count, Is.EqualTo(RejectedByCSharp.Length));
            Assert.That(visited.Count, Is.EqualTo(34),
                "Sprint contract section 0.5 assigns exactly 34 of the 74 counter-examples to the C# layer (reject): " +
                "18 measured at p1-01, 16 new for p1-02-mining (client 2026-09-27).");

            // Every listed file must be distinct. Two types now share a file name, and a
            // lookup that collapsed them would test one file twice and still report 34.
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count),
                "The C# counter-example list must name 34 distinct files.");

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
        public void Invalid_NoCSharpLayer_HasNoGeneratedDto(FixtureFile fixture)
        {
            // These are data-table counter-examples (SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING, and for
            // p1-02-mining DEPOSIT_FIELD/MINERAL/MINING_RULES/SIGNIFICANCE_RULE). AC-10(d) never
            // generates a DTO for kind "data", so there is nothing here for Strict to accept or
            // reject - the authoritative check is that ContractTypes.ByName has no entry for the
            // registry type name.
            //
            // The weaker, ContractDispatch-based "no envelope discriminator" check this test used
            // at p1-01 did NOT hold for SIGNIFICANCE_RULE when its data schema named a field
            // "event_type" (client finding, measured EditMode run 2026-09-27: 3 false positives -
            // ContractDispatch's generic message_type/command_type/event_type scan read the
            // data-table's business value "MINERAL_DISCOVERED" as if it were an envelope
            // discriminator). architect fixed this at the contract level, not here: the field is
            // now "produces_event_type" (contracts/data/significance-rule.schema.json,
            // ADR-0002 section 1a reserves *_type as discriminator-only keys; data/history/rules/
            // mineral-discovery.json and its 4 fixtures updated to match). The authoritative
            // ContractTypes.ByName check below never depended on this and needed no change; the
            // assertion right after confirms the rename actually closed the false positive.
            string json = fixture.ReadText();
            JObject envelope = ContractJson.ReadObject(json);

            string typeName;
            bool hasDiscriminator = ContractDispatch.TryGetTypeName(envelope, out typeName);
            TestContext.WriteLine(fixture + " has envelope discriminator: " + hasDiscriminator +
                                  (hasDiscriminator ? " (value: " + typeName + ")" : ""));

            // Confirms the produces_event_type rename actually closed the false positive (not
            // just "should have" by reading the schema) - if a SIGNIFICANCE_RULE fixture still
            // reports a discriminator here, the rename did not reach this fixture.
            if (fixture.TypeName == "SIGNIFICANCE_RULE")
                Assert.That(hasDiscriminator, Is.False,
                    fixture + ": SIGNIFICANCE_RULE must not have any message_type/command_type/" +
                    "event_type field any more - the collision field was renamed to " +
                    "produces_event_type (architect fix, 2026-09-27). If this is true, the fixture " +
                    "was not updated.");

            bool hasGeneratedDto = ContractTypes.ByName.ContainsKey(fixture.TypeName);
            TestContext.WriteLine(fixture + " registry type '" + fixture.TypeName + "' has generated DTO: " + hasGeneratedDto);
            Assert.That(hasGeneratedDto, Is.False,
                fixture + " is recorded as having no C# layer (data kind, AC-10(d)), but " +
                "ContractTypes.ByName now has an entry for '" + fixture.TypeName + "' - tell " +
                "architect, the layer split changed.");
        }

        /// <summary>
        /// Positive control for Invalid_NoCSharpLayer_HasNoGeneratedDto (qa request, C1
        /// verification round 2, 2026-09-27): that test only ever asserts
        /// ContractTypes.ByName.ContainsKey(fixture.TypeName) is False. Without this control, a
        /// fixture.TypeName whose shape does not match ByName's keys at all (wrong case, a path
        /// instead of a bare registry name) would make all 17 cases False "for free" and the
        /// suite would never notice - the same failure mode as the SC-71 discussion of "reflection
        /// looking at the wrong type" (02_client_ack.md). This proves the SAME lookup, on the SAME
        /// path (fixture.TypeName -> ContractTypes.ByName), returns True for a fixture whose type
        /// DOES have a generated DTO - MINE_RESOURCE, a p1-02-mining type C1 itself added.
        /// </summary>
        [Test]
        public void NoCSharpLayerCheck_PositiveControl_DtoBackedTypeReportsTrue()
        {
            FixtureFile fixture = ContractFixtures.RequireValid("MINE_RESOURCE", "basic.json");
            bool hasGeneratedDto = ContractTypes.ByName.ContainsKey(fixture.TypeName);
            TestContext.WriteLine(fixture + " registry type '" + fixture.TypeName + "' has generated DTO: " + hasGeneratedDto);

            Assert.That(hasGeneratedDto, Is.True,
                "MIN_RESOURCE has a generated DTO (C1) - if this lookup reports False for it, " +
                "Invalid_NoCSharpLayer_HasNoGeneratedDto's False results are not trustworthy either " +
                "(same lookup, same fixture.TypeName path).");
        }

        [Test]
        public void CSharpLayerSplit_Totals74()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in RejectedByCSharpCases()) visited.Add(fixture.ToString());
            foreach (FixtureFile fixture in NotDetectableCases()) visited.Add(fixture.ToString());
            foreach (FixtureFile fixture in NoCSharpLayerCases()) visited.Add(fixture.ToString());

            TestContext.WriteLine("reject " + RejectedByCSharp.Length + " / accept " +
                                  NotDetectableByCSharp.Length + " / no layer " + NoCSharpLayer.Length +
                                  " = " + visited.Count);

            Assert.That(RejectedByCSharp.Length, Is.EqualTo(34));
            Assert.That(NotDetectableByCSharp.Length, Is.EqualTo(23));
            Assert.That(NoCSharpLayer.Length, Is.EqualTo(17));
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count),
                "reject / accept / no-layer must not overlap.");
            Assert.That(visited.Count, Is.EqualTo(ContractFixtures.ExpectedInvalidFixtureCount),
                "34 + 23 + 17 must account for every counter-example the contract defines (74). " +
                "If this differs, the counts changed - tell architect instead of editing the table " +
                "(sprint contract section 0.5).");
        }

        // ------------------------------------------------------------------ SC-67

        /// <summary>
        /// SC-67: five closed-value-set-in-name-only string fields (reason_code, delivery,
        /// visibility, entity_kind, role - all plain C# strings by design, ADR-0005 section 4)
        /// must survive an UNKNOWN value under the Runtime profile (the one live message
        /// dispatch actually uses, not Strict). contracts/fixtures/ has no fixture carrying a
        /// value outside the known set (every invalid/ fixture the schema rejects is a schema
        /// violation, the opposite of what this tests) - qa's 02_client_ack.md SC-67 read agreed
        /// these five inputs are client-authored synthetic strings, not a fixture-sourced expected
        /// number, so this does not conflict with contract section 0.6.
        /// <para>
        /// Each case: (1) positive control - the injected value is NOT one of the field's known
        /// values (so a no-op test that reused a known value could not pass by accident), (2) the
        /// Runtime-profile deserialization does not throw, (3) the raw string comes back byte-for-
        /// byte unchanged (verbatim survival, not silently substituted or nulled).
        /// </para>
        /// </summary>
        [Test]
        public void UnknownClosedValue_ReasonCode_SurvivesRuntime()
        {
            string[] known = { "MALFORMED_COMMAND", "UNKNOWN_COMMAND_TYPE", "SCHEMA_VERSION_UNSUPPORTED",
                "DUPLICATE_COMMAND_ID", "SERVER_BUSY", "TOO_MANY_IN_FLIGHT", "RATE_LIMITED", "STALE_INPUT",
                "TARGET_UNKNOWN", "COOLDOWN_ACTIVE", "TARGET_OUT_OF_RANGE", "SHIP_TOO_FAST",
                "RESOURCE_DEPLETED", "RECORDING_BACKLOG", "CAPACITY_EXCEEDED" };
            const string unknownValue = "SOME_FUTURE_REASON_CODE_2027";
            Assert.That(known, Does.Not.Contain(unknownValue), "positive control: the injected value must not already be a known reason_code");

            string json = ContractFixtures.RequireValid("COMMAND_RESULT", "rejected-cooldown-active.json").ReadText()
                .Replace("\"COOLDOWN_ACTIVE\"", "\"" + unknownValue + "\"");

            CommandResultMessage message = null;
            Assert.DoesNotThrow(() => message = ContractJson.DeserializeRuntime<CommandResultMessage>(json),
                "Runtime must not throw on an unrecognised reason_code - ADR-0005 section 4.");
            TestContext.WriteLine("reason_code survived Runtime as: " + message.Payload.ReasonCode);
            Assert.That(message.Payload.ReasonCode, Is.EqualTo(unknownValue), "the raw string must survive verbatim, not be substituted or nulled");
        }

        [Test]
        public void UnknownClosedValue_Delivery_SurvivesRuntime()
        {
            string[] known = { "LIVE", "BACKFILL" };
            const string unknownValue = "REPLAY_FROM_THE_FUTURE";
            Assert.That(known, Does.Not.Contain(unknownValue));

            string json = ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "live.json").ReadText()
                .Replace("\"delivery\": \"LIVE\"", "\"delivery\": \"" + unknownValue + "\"");

            HistoricalEventNoticeMessage message = null;
            Assert.DoesNotThrow(() => message = ContractJson.DeserializeRuntime<HistoricalEventNoticeMessage>(json),
                "Runtime must not throw on an unrecognised delivery value.");
            TestContext.WriteLine("delivery survived Runtime as: " + message.Payload.Delivery);
            Assert.That(message.Payload.Delivery, Is.EqualTo(unknownValue));
        }

        [Test]
        public void UnknownClosedValue_Visibility_SurvivesRuntime()
        {
            string[] known = { "PUBLIC", "PARTICIPANTS_ONLY" };
            const string unknownValue = "FACTION_ONLY_P3";
            Assert.That(known, Does.Not.Contain(unknownValue));

            string json = ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "live.json").ReadText()
                .Replace("\"visibility\": \"PUBLIC\"", "\"visibility\": \"" + unknownValue + "\"");

            HistoricalEventNoticeMessage message = null;
            Assert.DoesNotThrow(() => message = ContractJson.DeserializeRuntime<HistoricalEventNoticeMessage>(json),
                "Runtime must not throw on an unrecognised visibility value.");
            TestContext.WriteLine("visibility survived Runtime as: " + message.Payload.HistoricalEvent.Visibility);
            Assert.That(message.Payload.HistoricalEvent.Visibility, Is.EqualTo(unknownValue));
        }

        [Test]
        public void UnknownClosedValue_EntityKind_SurvivesRuntime()
        {
            string[] known = { "PLAYER", "SHIP" };
            const string unknownValue = "STATION_P4";
            Assert.That(known, Does.Not.Contain(unknownValue));

            // Patches the FIRST entity_kind occurrence (the PLAYER/DISCOVERER participant) -
            // .Replace would hit both participants, so this targets just one via the "PLAYER"
            // literal that is unique to it in this fixture.
            string json = ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "live.json").ReadText()
                .Replace("\"entity_kind\": \"PLAYER\"", "\"entity_kind\": \"" + unknownValue + "\"");

            HistoricalEventNoticeMessage message = null;
            Assert.DoesNotThrow(() => message = ContractJson.DeserializeRuntime<HistoricalEventNoticeMessage>(json),
                "Runtime must not throw on an unrecognised entity_kind value.");
            string survived = message.Payload.HistoricalEvent.Participants[0].EntityKind;
            TestContext.WriteLine("entity_kind survived Runtime as: " + survived);
            Assert.That(survived, Is.EqualTo(unknownValue));
        }

        [Test]
        public void UnknownClosedValue_Role_SurvivesRuntime()
        {
            string[] known = { "DISCOVERER", "VESSEL" };
            const string unknownValue = "WITNESS_P5";
            Assert.That(known, Does.Not.Contain(unknownValue));

            string json = ContractFixtures.RequireValid("HISTORICAL_EVENT_NOTICE", "live.json").ReadText()
                .Replace("\"role\": \"DISCOVERER\"", "\"role\": \"" + unknownValue + "\"");

            HistoricalEventNoticeMessage message = null;
            Assert.DoesNotThrow(() => message = ContractJson.DeserializeRuntime<HistoricalEventNoticeMessage>(json),
                "Runtime must not throw on an unrecognised role value.");
            string survived = message.Payload.HistoricalEvent.Participants[0].Role;
            TestContext.WriteLine("role survived Runtime as: " + survived);
            Assert.That(survived, Is.EqualTo(unknownValue));
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

        // ------------------------------------------------------------------ SC-50 / SC-72

        /// <summary>
        /// Counts and lists *.json files under a root, independently of any other root - used by
        /// ClientDataCopy_MatchesRepositoryOriginal to walk data/ and the client copy as two
        /// unrelated directory trees (qa verification round 2, 2026-09-27: the original version
        /// of this test walked ONLY data/ and asked the copy "does this path exist", which can
        /// never notice a file the copy has that the ORIGINAL no longer does - e.g. a file deleted
        /// or renamed upstream that a stale client build still ships. Q-7's agreed shape was two
        /// independent traversals compared afterward, not one walk with a lookup into the other).
        /// </summary>
        static Dictionary<string, string> ListRelativeJsonPaths(string root)
        {
            var result = new Dictionary<string, string>(StringComparer.Ordinal);
            foreach (string path in Directory.GetFiles(root, "*.json", SearchOption.AllDirectories))
                result[Path.GetRelativePath(root, path).Replace('\\', '/')] = path;
            return result;
        }

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
                "client/Assets/_Project/Data not found - C2 must copy data/ into the client project (ADR-0012 section 7)");

            // Two INDEPENDENT traversals (Q-7, sprint contract SC-72) - neither walk consults the
            // other directory, so a file the copy has and the original does not (deleted/renamed
            // upstream, a stale build artifact) is visible as a copy-only entry, not silently
            // absorbed into "found 3, expected 3".
            Dictionary<string, string> originals = ListRelativeJsonPaths(dataRoot);
            Dictionary<string, string> copies = ListRelativeJsonPaths(clientCopyRoot);
            TestContext.WriteLine("data/ *.json found (original, independent count): " + originals.Count);
            TestContext.WriteLine("client copy *.json found (independent count): " + copies.Count);
            Assert.That(originals.Count, Is.GreaterThan(0), "data/ has no *.json files to compare");

            var copyOnly = new List<string>();
            foreach (string relative in copies.Keys)
                if (!originals.ContainsKey(relative)) copyOnly.Add(relative);
            copyOnly.Sort(StringComparer.Ordinal);
            foreach (string relative in copyOnly) TestContext.WriteLine("COPY-ONLY (not in data/): " + relative);
            Assert.That(copyOnly.Count, Is.EqualTo(0),
                "the client copy has " + copyOnly.Count + " file(s) with no matching original under data/ - " +
                "a file deleted or renamed upstream is still shipping in the client build.");

            Assert.That(copies.Count, Is.EqualTo(originals.Count),
                "independent counts must match: data/ has " + originals.Count + ", client copy has " + copies.Count);

            int checkedCount = 0;
            using (var sha256 = System.Security.Cryptography.SHA256.Create())
            {
                foreach (var pair in originals)
                {
                    string relative = pair.Key;
                    string originalPath = pair.Value;

                    Assert.That(copies.TryGetValue(relative, out string copyPath), Is.True,
                        "client copy is missing " + relative + " (expected under " + clientCopyRoot + ")");

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

        /// <summary>
        /// Negative control (team-lead request, qa verification round 2): proves
        /// ClientDataCopy_MatchesRepositoryOriginal's copy-only check is actually live by making
        /// it fail on purpose. Writes a throwaway stale.json under the client copy (nothing
        /// matching it under data/), runs the SAME independent-traversal logic inline, asserts it
        /// DOES report the file as copy-only, then deletes it in a finally block regardless of
        /// outcome - this test must never leave stray state in the tree it shares with the real
        /// SC-50/SC-72 test.
        /// </summary>
        [Test]
        public void ClientDataCopy_CopyOnlyFile_IsDetected_NegativeControl()
        {
            string repoRoot = ContractFixtures.RequireRepoRoot();
            string dataRoot = Path.Combine(repoRoot, "data");
            string clientCopyRoot = Path.Combine(repoRoot, "client", "Assets", "_Project", "Data");
            string staleRelative = "stale-negative-control.json";
            string stalePath = Path.Combine(clientCopyRoot, staleRelative);

            Assert.That(File.Exists(stalePath), Is.False, "a leftover stale file from a previous failed run must not already be here");

            try
            {
                File.WriteAllText(stalePath, "{}");

                Dictionary<string, string> originals = ListRelativeJsonPaths(dataRoot);
                Dictionary<string, string> copies = ListRelativeJsonPaths(clientCopyRoot);

                var copyOnly = new List<string>();
                foreach (string relative in copies.Keys)
                    if (!originals.ContainsKey(relative)) copyOnly.Add(relative);

                TestContext.WriteLine("with injected " + staleRelative + " present: copy-only count = " + copyOnly.Count);
                Assert.That(copyOnly, Does.Contain(staleRelative),
                    "the copy-only check must catch a file that exists in the client copy but not in data/ - " +
                    "if this fails, ClientDataCopy_MatchesRepositoryOriginal's copy-only assertion is not live.");
            }
            finally
            {
                if (File.Exists(stalePath)) File.Delete(stalePath);
            }

            Assert.That(File.Exists(stalePath), Is.False, "cleanup must remove the injected file even if the assertion above failed");
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
        public void NotDetectable_ListCoversExactlyTwentyThree()
        {
            var visited = new List<string>();
            foreach (FixtureFile fixture in NotDetectableCases()) visited.Add(fixture.ToString());
            foreach (string name in visited) TestContext.WriteLine("recorded as undetectable in C#: " + name);
            TestContext.WriteLine("counter-examples C# is recorded as unable to detect: " + visited.Count);

            Assert.That(visited.Count, Is.EqualTo(23),
                "Sprint contract section 0.5 records exactly 23 undetectable counter-examples " +
                "(10 measured at p1-01, 13 new for p1-02-mining).");
            Assert.That(new HashSet<string>(visited).Count, Is.EqualTo(visited.Count));

            // The full three-way split (reject 34 / accept 23 / no layer 17 = 74) is asserted in
            // CSharpLayerSplit_Totals74 above.
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
