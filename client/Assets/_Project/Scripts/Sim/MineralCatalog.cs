// Hand-written. Loads every data/minerals/*.json (contracts/data/mineral.schema.json). Unlike
// DepositFieldTable, this reads every field the schema defines: mineral identity, display name
// and rarity are public information a player can see once a mineral is discovered (design doc
// section 2.4 - "only fields some code reads are allowed"; rarity is display-only, ADR-0014
// section 5 - it is never an input to any history rule). yield/regen numbers are read for the
// greybox "channel is available" display only; the server is the sole judge of an actual
// extraction (principle 1).

using System;
using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    public sealed class MineralStats
    {
        public string Id;
        public string DisplayName;
        public string Rarity;
        public int YieldPerExtractionKg;
        public int RegenKg;
        public int RegenIntervalS;

        public static MineralStats FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            JObject extraction = (JObject)root["extraction"]
                ?? throw new InvalidOperationException("mineral JSON has no 'extraction' block");
            JObject regeneration = (JObject)root["regeneration"]
                ?? throw new InvalidOperationException("mineral JSON has no 'regeneration' block");

            return new MineralStats
            {
                Id = (string)root["id"] ?? throw new InvalidOperationException("mineral JSON has no 'id'"),
                DisplayName = (string)root["display_name"],
                Rarity = (string)root["rarity"],
                YieldPerExtractionKg = (int)extraction["yield_per_extraction_kg"],
                RegenKg = (int)regeneration["regen_kg"],
                RegenIntervalS = (int)regeneration["regen_interval_s"],
            };
        }
    }

    /// <summary>Every mineral table row, indexed by the file's `id` field - not the file name,
    /// same rule as ShipClassCatalog (the server indexes data/ by id, never by file name).</summary>
    public sealed class MineralCatalog
    {
        readonly Dictionary<string, MineralStats> _byId;

        MineralCatalog(Dictionary<string, MineralStats> byId) => _byId = byId;

        public static MineralCatalog LoadFromDirectory(string mineralsDir)
        {
            var byId = new Dictionary<string, MineralStats>(StringComparer.Ordinal);
            if (Directory.Exists(mineralsDir))
            {
                foreach (string file in Directory.GetFiles(mineralsDir, "*.json", SearchOption.TopDirectoryOnly))
                {
                    MineralStats stats = MineralStats.FromJson(File.ReadAllText(file));
                    byId[stats.Id] = stats; // last one wins; duplicate-id rejection is the server's job
                }
            }
            return new MineralCatalog(byId);
        }

        public bool TryGet(string mineralId, out MineralStats stats) => _byId.TryGetValue(mineralId, out stats);

        public int Count => _byId.Count;
    }
}
