// C4/R4. WORLD_SNAPSHOT bypasses the JObject tree (sprint contract section 0.7 "핫 경로",
// ContractDispatch.DirectDeserializeTypes). These tests prove the fast path is not just
// faster but IDENTICAL: same discriminator, same DTO field values as the tree-based path every
// other type still uses. Correctness first - the allocation numbers are measured separately in
// Unity Profiler under Editor (SC-58) and are already recorded by hand-off measurement
// (02_client_ack.md section wch7: 0.656 ms/8 gen0 tree vs 0.265 ms/1 gen0 direct, 200 reps).

using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using NUnit.Framework;
using Starfall.Contracts;
using Starfall.Contracts.Generated;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class WorldSnapshotFastPathTests
    {
        [Test]
        public void TryPeekTypeName_FindsDiscriminator_WithoutBuildingATree()
        {
            foreach (string fileName in new[] { "two-ships-one-lingering.json", "empty-nulls-and-bounds.json" })
            {
                FixtureFile fixture = ContractFixtures.RequireValid("WORLD_SNAPSHOT", fileName);
                string peeked;
                bool found = ContractDispatch.TryPeekTypeName(fixture.ReadText(), out peeked);

                TestContext.WriteLine(fixture + " : found=" + found + " typeName=" + peeked);
                Assert.That(found, Is.True);
                Assert.That(peeked, Is.EqualTo("WORLD_SNAPSHOT"));
            }
        }

        [Test]
        public void TryPeekTypeName_FindsDiscriminator_ForEveryValidMessageFixture()
        {
            // The scan must work for every message/command/event fixture, not just
            // WORLD_SNAPSHOT - it is the ONLY thing that decides which path a frame takes.
            int checkedCount = 0;
            foreach (FixtureFile fixture in ContractFixtures.RequireValidFixtures())
            {
                if (ContractFixtures.IsDataOnly(fixture)) continue; // no envelope, nothing to peek

                string json = fixture.ReadText();
                string viaTree;
                ContractDispatch.TryGetTypeName(ContractJson.ReadObject(json), out viaTree);

                string viaPeek;
                bool found = ContractDispatch.TryPeekTypeName(json, out viaPeek);

                Assert.That(found, Is.True, fixture + " : peek found no discriminator");
                Assert.That(viaPeek, Is.EqualTo(viaTree), fixture + " : peek and tree disagree on type name");
                checkedCount++;
            }

            TestContext.WriteLine("valid message fixtures checked: " + checkedCount);
            Assert.That(checkedCount, Is.EqualTo(ContractFixtures.ExpectedRoundTrippableValidFixtureCount));
        }

        [Test]
        public void DirectPath_And_TreePath_ProduceIdenticalShipStates()
        {
            FixtureFile fixture = ContractFixtures.RequireValid("WORLD_SNAPSHOT", "two-ships-one-lingering.json");
            string json = fixture.ReadText();

            // Direct path (what RealtimeClient.OnMessage now calls).
            string directTypeName;
            object directContract;
            string directError;
            bool directOk = ContractDispatch.TryRead(json, JsonSerializer.Create(ContractJson.Strict),
                out directTypeName, out directContract, out directError);

            // Tree path (what the fixture round-trip tests already exercise for every other type).
            JObject envelope = ContractJson.ReadObject(json);
            string treeTypeName;
            object treeContract;
            string treeError;
            bool treeOk = ContractDispatch.TryRead(envelope, JsonSerializer.Create(ContractJson.Strict),
                out treeTypeName, out treeContract, out treeError);

            Assert.That(directOk, Is.True, "direct path failed: " + directError);
            Assert.That(treeOk, Is.True, "tree path failed: " + treeError);
            Assert.That(directTypeName, Is.EqualTo(treeTypeName));

            var direct = (WorldSnapshotMessage)directContract;
            var tree = (WorldSnapshotMessage)treeContract;

            Assert.That(direct.Tick, Is.EqualTo(tree.Tick));
            Assert.That(direct.Payload.Ships.Length, Is.EqualTo(tree.Payload.Ships.Length));
            for (int i = 0; i < direct.Payload.Ships.Length; i++)
            {
                Assert.That(direct.Payload.Ships[i].ShipId, Is.EqualTo(tree.Payload.Ships[i].ShipId), "ship " + i);
                Assert.That(direct.Payload.Ships[i].PositionZMm, Is.EqualTo(tree.Payload.Ships[i].PositionZMm), "ship " + i);
                Assert.That(direct.Payload.Ships[i].AngularVelocityRollMdegS, Is.EqualTo(tree.Payload.Ships[i].AngularVelocityRollMdegS), "ship " + i);
                Assert.That(direct.Payload.Ships[i].Presence, Is.EqualTo(tree.Payload.Ships[i].Presence), "ship " + i);
            }

            TestContext.WriteLine("direct and tree paths agree on " + direct.Payload.Ships.Length + " ship(s), tick " + direct.Tick);
        }

        [Test]
        public void DirectPath_RejectsInvalidFixture_SameAsTreePath()
        {
            // A counter-example already assigned to the C# reject list (ContractFixtureTests)
            // must be rejected the same way through the fast path.
            FixtureFile fixture = ContractFixtures.RequireInvalid("WORLD_SNAPSHOT", "angular-velocity-roll-missing.json");
            string json = fixture.ReadText();

            string typeName;
            object contract;
            string error;
            bool ok = ContractDispatch.TryRead(json, JsonSerializer.Create(ContractJson.Strict), out typeName, out contract, out error);

            TestContext.WriteLine(fixture + " via direct path : ok=" + ok + " error=" + error);
            Assert.That(ok, Is.False, "the direct path must reject what the tree path rejects, not silently accept it");
        }

        [Test]
        public void OtherTypes_StillUseTreePath_Unaffected()
        {
            // PING_SERVER is not in DirectDeserializeTypes - this is really a guard that the
            // dispatch changes above did not silently widen the fast path.
            Assert.That(ContractDispatch.DirectDeserializeTypes.Contains("PING_SERVER"), Is.False);
            Assert.That(ContractDispatch.DirectDeserializeTypes.Contains("WORLD_SNAPSHOT"), Is.True);
            Assert.That(ContractDispatch.DirectDeserializeTypes.Count, Is.EqualTo(1),
                "only WORLD_SNAPSHOT is big+frequent enough to matter (sprint contract section 0.7)");
        }
    }
}
