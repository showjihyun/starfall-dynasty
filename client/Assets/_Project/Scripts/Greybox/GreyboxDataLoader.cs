// Hand-written. The "load ships/star-system/sync-tuning from the client's copy of data/" logic,
// factored out of GreyboxSession.LoadData so ObserverSession (SC-64/65's two-session-in-one-
// process harness, client ack issue 6) does not hand-copy it a second time. GreyboxSession keeps
// its own LoadData method unchanged (it is already reviewed and PlayMode-tested); this is purely
// for the new harness.

using System.IO;
using Starfall.Sim;
using UnityEngine;

namespace Starfall.Greybox
{
    /// <summary>The three data tables a session needs, or null where the file was missing -
    /// callers decide how to react (GreyboxSession logs and limps on; a strict caller could
    /// throw instead).</summary>
    public readonly struct GreyboxData
    {
        public readonly ShipClassCatalog ShipClasses;
        public readonly StarSystemData StarSystem;
        public readonly SyncTuningData Tuning;

        public GreyboxData(ShipClassCatalog shipClasses, StarSystemData starSystem, SyncTuningData tuning)
        {
            ShipClasses = shipClasses;
            StarSystem = starSystem;
            Tuning = tuning;
        }
    }

    /// <summary>The three p1-02-mining data tables a greybox session needs for display, or null
    /// where the file was missing. Deliberately does NOT include anything from the history rule
    /// file under data/history/rules/ - that file is copied for SC-72's byte-identity check only
    /// (Q-8, 02_client_ack.md) and nothing in gameplay code reads its values (see
    /// GreyboxMiningDataTests.MiningRuleFile_ValuesNotConsumedByGameplay, which fails loudly if a
    /// file under Scripts/ later names that file - this comment deliberately never spells the
    /// literal file name so it does not trip its own guard).</summary>
    public readonly struct GreyboxMiningData
    {
        public readonly DepositFieldTable Deposits;
        public readonly MineralCatalog Minerals;
        public readonly MiningRulesData Rules;

        public GreyboxMiningData(DepositFieldTable deposits, MineralCatalog minerals, MiningRulesData rules)
        {
            Deposits = deposits;
            Minerals = minerals;
            Rules = rules;
        }
    }

    public static class GreyboxDataLoader
    {
        /// <param name="logPrefix">Prefixes the incomplete-data warning, so two callers'
        /// warnings (e.g. "starfall.greybox: " vs "starfall.observer.b: ") are told apart in
        /// the log when both run in the same process.</param>
        public static GreyboxData Load(string logPrefix)
        {
            string dataRoot = Path.Combine(Application.dataPath, "_Project", "Data");
            ShipClassCatalog shipClasses = ShipClassCatalog.LoadFromDirectory(Path.Combine(dataRoot, "ships"));

            string cradlePath = Path.Combine(dataRoot, "world", "systems", "cradle.json");
            StarSystemData starSystem = File.Exists(cradlePath) ? StarSystemData.FromJson(File.ReadAllText(cradlePath)) : null;

            string tuningPath = Path.Combine(dataRoot, "movement", "sync-tuning.json");
            SyncTuningData tuning = File.Exists(tuningPath) ? SyncTuningData.FromJson(File.ReadAllText(tuningPath)) : null;

            if (starSystem == null || tuning == null || shipClasses.Count == 0)
            {
                Debug.LogWarning(logPrefix + "data/ copy incomplete under " + dataRoot +
                                 " - ship classes=" + shipClasses.Count +
                                 ", star system=" + (starSystem != null) + ", tuning=" + (tuning != null));
            }

            return new GreyboxData(shipClasses, starSystem, tuning);
        }

        /// <param name="starSystemId">Selects which data/world/deposits/{star_system_id}.json to
        /// load - "cradle" for the vertical slice.</param>
        public static GreyboxMiningData LoadMining(string logPrefix, string starSystemId)
        {
            string dataRoot = Path.Combine(Application.dataPath, "_Project", "Data");

            string depositsPath = Path.Combine(dataRoot, "world", "deposits", starSystemId + ".json");
            DepositFieldTable deposits = DepositFieldTable.LoadFromFile(depositsPath);

            MineralCatalog minerals = MineralCatalog.LoadFromDirectory(Path.Combine(dataRoot, "minerals"));

            string rulesPath = Path.Combine(dataRoot, "mining", "mining-rules.json");
            MiningRulesData rules = File.Exists(rulesPath) ? MiningRulesData.FromJson(File.ReadAllText(rulesPath)) : null;

            if (deposits == null || rules == null || minerals.Count == 0)
            {
                Debug.LogWarning(logPrefix + "mining data/ copy incomplete under " + dataRoot +
                                 " - deposits=" + (deposits != null) +
                                 ", minerals=" + minerals.Count + ", rules=" + (rules != null));
            }

            return new GreyboxMiningData(deposits, minerals, rules);
        }
    }
}
