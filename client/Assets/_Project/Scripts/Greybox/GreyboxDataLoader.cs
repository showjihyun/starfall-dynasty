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
    /// where the file was missing. p1-03-trading removed the history-rule-file note this comment
    /// used to carry - history/rules/ is no longer copied to the client at all (ADR-0015 section
    /// 2), so there is nothing left here to warn gameplay code away from reading.</summary>
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

    /// <summary>p1-03-trading (C1): the one market data table a greybox session needs for
    /// display and the expected-proceeds preview (MarketTable's file header), or null if the
    /// file was missing.</summary>
    public readonly struct GreyboxTradingData
    {
        public readonly MarketTable Market;

        public GreyboxTradingData(MarketTable market)
        {
            Market = market;
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

        /// <param name="marketId">Selects which data/market/{id}.json to load - "cradle-exchange"
        /// for the vertical slice.</param>
        public static GreyboxTradingData LoadTrading(string logPrefix, string marketId)
        {
            string dataRoot = Path.Combine(Application.dataPath, "_Project", "Data");

            string marketPath = Path.Combine(dataRoot, "market", marketId + ".json");
            MarketTable market = MarketTable.LoadFromFile(marketPath);

            if (market == null)
                Debug.LogWarning(logPrefix + "trading data/ copy incomplete under " + dataRoot + " - market missing");

            return new GreyboxTradingData(market);
        }
    }
}
