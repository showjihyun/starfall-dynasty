// EditMode tests for C2 (p1-02-mining): the client's copy of data/{minerals,mining,world/deposits,
// history/rules}/** and the loader that reads it. Sprint contract SC-71 (part A only - part B,
// DepositMarker_FollowsMessageNotDataCopy, needs the marker component C3 builds), SC-72 is
// covered by the existing ContractFixtureTests.ClientDataCopy_MatchesRepositoryOriginal (it
// globs contracts/../data/**/*.json, so it picks up these 7 new files without changes here).

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

        // ------------------------------------------------------------------ Q-8: history rule
        // file is copied (for SC-72's hash test) but never consumed by gameplay code.

        [Test]
        public void MiningRuleFile_ValuesNotConsumedByGameplay()
        {
            // Q-8 (02_client_ack.md, qa agreement): data/history/rules/mineral-discovery.json is
            // not an AC item, kept as a client-only regression guard. The rule's own designer_note
            // says the values are Rust-code inputs (ADR-0014 section 5) - nothing in the client
            // should ever open this specific file. This is a source-text guard, not a schema
            // check: it fails loudly if a future change starts reading it, rather than relying on
            // "nobody happened to write that code yet".
            string scriptsRoot = Path.Combine(ContractFixtures.RequireRepoRoot(), "client", "Assets", "_Project", "Scripts");
            Assert.That(Directory.Exists(scriptsRoot), Is.True);

            string[] sources = Directory.GetFiles(scriptsRoot, "*.cs", SearchOption.AllDirectories);
            TestContext.WriteLine("Scripts/*.cs files scanned: " + sources.Length);
            Assert.That(sources.Length, Is.GreaterThan(0));

            var offenders = new System.Collections.Generic.List<string>();
            foreach (string file in sources)
            {
                string text = File.ReadAllText(file);
                if (text.Contains("mineral-discovery.json") || text.Contains("\"rules\", \"mineral-discovery"))
                    offenders.Add(file);
            }

            foreach (string offender in offenders) TestContext.WriteLine("references mineral-discovery.json: " + offender);
            Assert.That(offenders.Count, Is.EqualTo(0),
                "no file under Scripts/ should name mineral-discovery.json - it is copied for the " +
                "byte-identity test only (SC-72) and gameplay code must never read its values.");
        }
    }
}
