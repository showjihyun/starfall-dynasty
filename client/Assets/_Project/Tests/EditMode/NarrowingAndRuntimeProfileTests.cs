// SC-39 (generator narrowing), SC-46 (Runtime profile). EditMode, no server needed.
//
// SC-39 is a regression test in the literal sense: the generator used to drop the actor_id
// narrowing on the floor, the DTO accepted null anyway, and the only thing that said
// otherwise was an XML comment (U-2). Asserting on the generated member's type and Required
// mode is what stops that from coming back the next time the merge logic is touched.

using System;
using System.Collections.Generic;
using System.Reflection;
using Newtonsoft.Json;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class NarrowingAndRuntimeProfileTests
    {
        // ------------------------------------------------------------------ SC-39

        /// <summary>The two types whose schema narrows actor_id from "UuidV7 or null" to
        /// UuidV7 (spec section 5.1).</summary>
        static readonly Type[] NarrowedActorTypes = { typeof(SessionOpenedEvent), typeof(SessionClosedEvent) };

        [Test]
        public void Narrowing_ActorId_IsNonNullableAndAlwaysRequired()
        {
            int checkedCount = 0;

            foreach (Type type in NarrowedActorTypes)
            {
                PropertyInfo property = type.GetProperty("ActorId");
                Assert.That(property, Is.Not.Null, type.Name + " has no ActorId property");

                var attribute = (JsonPropertyAttribute)Attribute.GetCustomAttribute(property, typeof(JsonPropertyAttribute));
                Assert.That(attribute, Is.Not.Null, type.Name + ".ActorId has no JsonProperty attribute");

                TestContext.WriteLine(type.Name + ".ActorId : " + property.PropertyType +
                                      " , Required = " + attribute.Required);

                Assert.That(property.PropertyType, Is.EqualTo(typeof(Guid)),
                    type.Name + ".ActorId must be Guid, not Guid?. The schema narrows actor_id " +
                    "to non-null for this type (ADR-0005 section 4-3).");
                Assert.That(attribute.Required, Is.EqualTo(Required.Always),
                    type.Name + ".ActorId must be Required.Always, not AllowNull.");
                Assert.That(attribute.PropertyName, Is.EqualTo("actor_id"));

                checkedCount++;
            }

            TestContext.WriteLine("narrowed properties checked: " + checkedCount);
            Assert.That(checkedCount, Is.EqualTo(2));
        }

        [Test]
        public void Narrowing_DoesNotLeakToFieldsTheEnvelopeLeavesNullable()
        {
            // The fix must narrow exactly what the schema narrows. causation_id stays nullable
            // in both types, and correlation_id is non-null in the event envelope to begin
            // with - if the merge started removing anyOf everywhere, causation_id would flip
            // and this test is what notices.
            int checkedCount = 0;

            foreach (Type type in NarrowedActorTypes)
            {
                AssertProperty(type, "CausationId", typeof(Guid?), Required.AllowNull);
                AssertProperty(type, "CorrelationId", typeof(Guid), Required.Always);
                checkedCount += 2;
            }

            AssertProperty(typeof(CommandResultMessage), "CorrelationId", typeof(Guid?), Required.AllowNull);
            AssertProperty(typeof(PingServerCommand), "ClientSentAt", typeof(string), Required.AllowNull);
            checkedCount += 2;

            TestContext.WriteLine("nullable-envelope properties checked: " + checkedCount);
            Assert.That(checkedCount, Is.EqualTo(6));
        }

        static void AssertProperty(Type type, string name, Type expectedType, Required expectedRequired)
        {
            PropertyInfo property = type.GetProperty(name);
            Assert.That(property, Is.Not.Null, type.Name + " has no " + name);

            var attribute = (JsonPropertyAttribute)Attribute.GetCustomAttribute(property, typeof(JsonPropertyAttribute));
            TestContext.WriteLine("  " + type.Name + "." + name + " : " + property.PropertyType +
                                  " , Required = " + attribute.Required);

            Assert.That(property.PropertyType, Is.EqualTo(expectedType), type.Name + "." + name);
            Assert.That(attribute.Required, Is.EqualTo(expectedRequired), type.Name + "." + name);
        }

        [Test]
        public void Narrowing_EnumValuesStayStrings()
        {
            // The other half of the generator contract: a closed value set is a C# string, so
            // an added value reaches the client as data instead of dropping the message
            // (ADR-0005 section 4-2). Generating an enum here would be a silent compatibility
            // break that only shows up after the server ships a new value.
            AssertIsString(typeof(CommandResultMessage.CommandResultPayload), "Status");
            AssertIsString(typeof(CommandResultMessage.CommandResultPayload), "ReasonCode");
            AssertIsString(typeof(SessionOpenedEvent.SessionOpenedPayload), "Transport");
            AssertIsString(typeof(SessionClosedEvent.SessionClosedPayload), "CloseReason");
            TestContext.WriteLine("closed value sets checked: 4");
        }

        static void AssertIsString(Type type, string name)
        {
            PropertyInfo property = type.GetProperty(name);
            Assert.That(property, Is.Not.Null, type.Name + " has no " + name);
            TestContext.WriteLine("  " + type.Name + "." + name + " : " + property.PropertyType);
            Assert.That(property.PropertyType, Is.EqualTo(typeof(string)),
                type.Name + "." + name + " must stay a string, never a C# enum.");
        }

        // ------------------------------------------------------------------ SC-46

        [Test]
        public void Runtime_IgnoresUnknownMember_AndWarns()
        {
            FixtureFile fixture = ContractFixtures.RequireInvalid("COMMAND_RESULT", "payload-unknown-field.json");
            var warnings = new List<string>();

            var message = JsonConvert.DeserializeObject<CommandResultMessage>(
                fixture.ReadText(), ContractJson.CreateRuntime(warnings.Add));

            foreach (string path in warnings) TestContext.WriteLine("ignored member at " + path);

            Assert.That(message, Is.Not.Null);
            Assert.That(warnings.Count, Is.EqualTo(1), "exactly one member should have been ignored");
            Assert.That(warnings[0], Is.EqualTo("payload.queued_ticks"));

            // The rest of the message must survive: tolerating the unknown member is pointless
            // if the known ones are lost with it.
            Assert.That(message.Payload.Status, Is.EqualTo("ACCEPTED"));
            Assert.That(message.Payload.CommandId, Is.Not.EqualTo(Guid.Empty));
        }

        [Test]
        public void Runtime_SameInput_IsAnExceptionUnderStrict()
        {
            FixtureFile fixture = ContractFixtures.RequireInvalid("COMMAND_RESULT", "payload-unknown-field.json");

            var thrown = Assert.Throws<JsonSerializationException>(
                () => ContractJson.DeserializeStrict<CommandResultMessage>(fixture.ReadText()));

            TestContext.WriteLine("Strict rejected the same input: " + thrown.Message);
            Assert.That(thrown.Message, Does.StartWith(ContractJson.IgnoredMemberMessagePrefix));
        }

        [Test]
        public void Runtime_MissingRequiredField_StillThrows()
        {
            // The line that keeps "tolerant" from meaning "unvalidated". Without it this
            // fixture deserializes without complaint and hands the caller a session whose id
            // is Guid.Empty (measured before the profile existed).
            FixtureFile fixture = ContractFixtures.RequireInvalid("SESSION_READY", "missing-session-id.json");
            var warnings = new List<string>();

            var thrown = Assert.Throws<JsonSerializationException>(
                () => JsonConvert.DeserializeObject<SessionReadyMessage>(
                    fixture.ReadText(), ContractJson.CreateRuntime(warnings.Add)));

            TestContext.WriteLine("Runtime rejected " + fixture + ": " + thrown.Message);
            Assert.That(thrown.Message, Does.Contain("session_id"));
            Assert.That(warnings, Is.Empty, "a missing required field is not an ignorable member");
        }

        [Test]
        public void Runtime_NullInNonNullableField_StillThrows()
        {
            FixtureFile fixture = ContractFixtures.RequireInvalid("SESSION_CLOSED", "correlation-id-null.json");
            var warnings = new List<string>();

            var thrown = Assert.Throws<JsonSerializationException>(
                () => JsonConvert.DeserializeObject<SessionClosedEvent>(
                    fixture.ReadText(), ContractJson.CreateRuntime(warnings.Add)));

            TestContext.WriteLine("Runtime rejected " + fixture + ": " + thrown.Message);
            Assert.That(warnings, Is.Empty, "a null in a non-nullable field is not an ignorable member");
        }

        [Test]
        public void Runtime_ValidFixture_ProducesNoWarnings()
        {
            // Data-table fixtures (SHIP_CLASS/STAR_SYSTEM/SYNC_TUNING) carry no envelope
            // discriminator at all (AC-10(d): no DTO is generated for kind "data"), so they are
            // not candidates for Runtime/Strict dispatch - same split as the round-trip suite
            // (ContractFixtureTests.RoundTrippableValidFixtureCases, 27 found / 21 dispatchable -
            // R3 decision 5 added SESSION_CLOSED/superseded.json, 2026-09-22).
            int checkedCount = 0;
            foreach (FixtureFile fixture in ContractFixtures.RequireValidFixtures())
            {
                if (ContractFixtures.IsDataOnly(fixture)) continue;

                var warnings = new List<string>();
                Type dtoType = DtoTypeOf(fixture);

                object dto = JsonConvert.DeserializeObject(
                    fixture.ReadText(), dtoType, ContractJson.CreateRuntime(warnings.Add));

                Assert.That(dto, Is.Not.Null, fixture + " failed under Runtime");
                Assert.That(warnings, Is.Empty, fixture + " produced warnings under Runtime: " + string.Join(", ", warnings));
                checkedCount++;
            }

            TestContext.WriteLine("valid fixtures read under Runtime with zero warnings: " + checkedCount);
            Assert.That(checkedCount, Is.EqualTo(ContractFixtures.ExpectedRoundTrippableValidFixtureCount));
        }

        [Test]
        public void Runtime_PrefixDependency_IsPinned()
        {
            // The Runtime profile can only tell "unknown member" from every other
            // deserialization error by this message prefix: ErrorContext.Member is non-null in
            // both cases and ErrorContext.Path equals the member name in the null-on-
            // non-nullable case. If a Newtonsoft upgrade changes the wording, this test fails
            // - which is the point. The alternative is the live receive path silently
            // becoming strict, or silently accepting broken messages, depending on which way
            // the wording moved.
            FixtureFile fixture = ContractFixtures.RequireInvalid("PING_REPLY", "payload-unknown-field.json");

            var thrown = Assert.Throws<JsonSerializationException>(
                () => ContractJson.DeserializeStrict<PingReplyMessage>(fixture.ReadText()));

            TestContext.WriteLine("Newtonsoft unknown-member message: " + thrown.Message);
            Assert.That(ContractJson.IsIgnorableMember(thrown.Message), Is.True,
                "ContractJson.IgnoredMemberMessagePrefix no longer matches Newtonsoft's wording. " +
                "Check the pinned com.unity.nuget.newtonsoft-json version before touching the profile.");

            Assert.That(ContractJson.IsIgnorableMember("Required property 'x' not found in JSON."), Is.False);
            Assert.That(ContractJson.IsIgnorableMember(null), Is.False);
        }

        static Type DtoTypeOf(FixtureFile fixture)
        {
            string typeName;
            Assert.That(ContractDispatch.TryGetTypeName(ContractJson.ReadObject(fixture.ReadText()), out typeName), Is.True);

            Type dtoType;
            Assert.That(ContractTypes.ByName.TryGetValue(typeName, out dtoType), Is.True,
                "registry type '" + typeName + "' has no generated DTO");
            return dtoType;
        }
    }
}
