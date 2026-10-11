// EditMode tests for C2 (p1-02-mining): the client's copy of data/{minerals,mining,world/deposits}/**
// and the loader that reads it. Sprint contract SC-71 (part A only - part B,
// DepositMarker_FollowsMessageNotDataCopy, needs the marker component C3 builds), SC-72 was
// covered by the (now p1-03-trading SC-56-renamed) ContractFixtureTests.
// ClientDataCopy_MatchesRegistryDerivedSet - it derives its expected set from the registry's
// "consumers" field, not a glob over all of data/. history/rules/ no longer belongs to this
// comment's file list, and MiningRuleFile_ValuesNotConsumedByGameplay (p1-02's Q-8 guard against
// gameplay code reading mineral-discovery.json) was removed in p1-03-trading: ADR-0015 section 2
// confirmed SIGNIFICANCE_RULE's consumers stayed history-only, so the file the guard protected
// against reading is no longer copied to the client at all - the guard's premise (the file sits
// in the client tree only for a byte-identity test) no longer holds, and
// ClientDataCopy_ServerOnlyKindFile_IsDetected_NegativeControl in ContractFixtureTests.cs now
// covers "a server-only kind's file must never be in the client copy" more strongly (it fails
// the build, not just a source-text grep).

using System;
using System.IO;
using System.Linq;
using System.Reflection;
using NUnit.Framework;
using Starfall.Greybox;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class GreyboxMiningDataTests
    {
        static string DataRoot() =>
            Path.Combine(ContractFixtures.RequireRepoRoot(), "client", "Assets", "_Project", "Data");

        // ------------------------------------------------------------------ SC-71 part A

        [Test]
        public void DepositTableEntry_HasNoServerOnlyFields()
        {
            // AC-14(f), I-68, sprint contract SC-71: the local parsing model must not be able to
            // hold mineral_id or initial_reserve_kg at all - the strongest defence against a
            // later change accidentally reading them (02_client_ack.md C2 "테스트 A").
            Type type = typeof(DepositTableEntry);
            MemberInfo[] members = type.GetMembers(BindingFlags.Public | BindingFlags.Instance)
                .Where(m => m.MemberType == MemberTypes.Field || m.MemberType == MemberTypes.Property)
                .ToArray();

            foreach (MemberInfo member in members) TestContext.WriteLine("member: " + member.Name);
            TestContext.WriteLine("members checked: " + members.Length);

            // Positive control (sprint contract SC-71 (A), qa r1 agreement): assert the type DOES
            // carry a position_m-equivalent field first. Without this, a reflection bug that
            // pointed at the wrong type would also report "no server-only fields" and pass for
            // the wrong reason (the false-negative the qa/client discussion in 02_client_ack.md
            // was worried about).
            Assert.That(members.Any(m => m.Name.IndexOf("Position", StringComparison.OrdinalIgnoreCase) >= 0),
                Is.True, "positive control failed: DepositTableEntry has no Position-like member - " +
                "reflection is probably looking at the wrong type.");

            string[] forbidden = { "mineral_id", "MineralId", "initial_reserve_kg", "InitialReserveKg" };
            foreach (string name in forbidden)
            {
                Assert.That(members.Any(m => string.Equals(m.Name, name, StringComparison.Ordinal)), Is.False,
                    "DepositTableEntry must not carry a '" + name + "' member - a deposit's mineral " +
                    "and reserve must reach gameplay code only through DEPOSIT_FIELD_STATE, never " +
                    "the client's copy of the server-only deposit table (design doc section 3.2).");
            }
        }

        [Test]
        public void DepositFieldTable_ParsesCradle_PositionAndRadiusOnly()
        {
            string path = Path.Combine(DataRoot(), "world", "deposits", "cradle.json");
            Assert.That(File.Exists(path), Is.True, "client copy missing at " + path + " - C2 must copy data/world/deposits/cradle.json");

            DepositFieldTable table = DepositFieldTable.LoadFromFile(path);
            Assert.That(table, Is.Not.Null);
            Assert.That(table.StarSystemId, Is.EqualTo("cradle"));

            TestContext.WriteLine("deposits parsed: " + table.Deposits.Count);
            Assert.That(table.Deposits.Count, Is.EqualTo(8), "data/world/deposits/cradle.json defines 8 deposits");

            DepositTableEntry farReach = table.Deposits.Single(d => d.Id == "far-reach");
            TestContext.WriteLine("far-reach: position=" + farReach.PositionM + " radius_m=" + farReach.RadiusM);
            Assert.That(farReach.DisplayName, Is.EqualTo("Far Reach"));
            Assert.That(farReach.RadiusM, Is.EqualTo(40.0));
        }

        // ------------------------------------------------------------------ mineral / mining-rules loaders

        [Test]
        public void MineralCatalog_LoadsAllFour()
        {
            MineralCatalog catalog = MineralCatalog.LoadFromDirectory(Path.Combine(DataRoot(), "minerals"));
            TestContext.WriteLine("minerals loaded: " + catalog.Count);
            Assert.That(catalog.Count, Is.EqualTo(4), "data/minerals/ has 4 files (ferrosite, glacine, cobaltine, starfall-glass)");

            MineralStats ferrosite;
            Assert.That(catalog.TryGet("ferrosite", out ferrosite), Is.True);
            Assert.That(ferrosite.DisplayName, Is.EqualTo("Ferrosite"));
            Assert.That(ferrosite.Rarity, Is.EqualTo("common"));
            Assert.That(ferrosite.YieldPerExtractionKg, Is.EqualTo(200));
        }

        [Test]
        public void MiningRulesData_ParsesThreeNumbers()
        {
            string path = Path.Combine(DataRoot(), "mining", "mining-rules.json");
            Assert.That(File.Exists(path), Is.True, "client copy missing at " + path);

            MiningRulesData rules = MiningRulesData.FromJson(File.ReadAllText(path));
            TestContext.WriteLine("mining_range_from_surface_m=" + rules.MiningRangeFromSurfaceM +
                                  " max_ship_speed_mps=" + rules.MaxShipSpeedMps + " cooldown_s=" + rules.CooldownS);

            Assert.That(rules.MiningRangeFromSurfaceM, Is.EqualTo(150.0));
            Assert.That(rules.MaxShipSpeedMps, Is.EqualTo(10.0));
            Assert.That(rules.CooldownS, Is.EqualTo(3));
        }

        [Test]
        public void GreyboxDataLoader_LoadMining_PopulatesAllThreeTables()
        {
            // Runs the same code path GreyboxSession will call (C3), against the real client
            // copy under Assets/_Project/Data - not a fixture, this is genuinely "does the
            // loader work end to end".
            GreyboxMiningData data = GreyboxDataLoader.LoadMining("starfall.tests: ", "cradle");

            Assert.That(data.Deposits, Is.Not.Null);
            Assert.That(data.Deposits.Deposits.Count, Is.EqualTo(8));
            Assert.That(data.Minerals.Count, Is.EqualTo(4));
            Assert.That(data.Rules, Is.Not.Null);
        }
    }
}
