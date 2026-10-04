// Hand-written. Loads data/world/deposits/{star_system_id}.json (contracts/data/
// deposit-field.schema.json) - the client's copy of a SERVER-ONLY table (ADR-0012 section 7:
// "file copy, this time only"; the schema's own description says so in capitals).
//
// DepositTableEntry deliberately has NO field for mineral_id or initial_reserve_kg. This is not
// an oversight: those two values must never reach gameplay code before a deposit is revealed
// (design doc section 3.2, sprint contract SC-71). The client's build still embeds them (the
// JSON file itself is copied byte-for-byte, ADR-0012 section 7 recheck at p1-03), but the
// parsing model here cannot produce a value nothing declared a place for - the strongest defence
// against "a later change accidentally starts reading it" is that there is no field to read into.
// Whether the deposit is revealed, and what mineral it holds, comes from DEPOSIT_FIELD_STATE
// only (C3 wires that up); this loader answers a different question: where is the marker and how
// big is it, which is public information the client needs before any extraction ever happens.

using System;
using System.Collections.Generic;
using System.IO;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    /// <summary>One deposit's client-visible fields. Position and radius are needed to draw a
    /// marker and check "am I in range" for display; id joins to DEPOSIT_FIELD_STATE.deposits[*]
    /// .deposit_id. Nothing else from the schema is here on purpose (see file header).</summary>
    public sealed class DepositTableEntry
    {
        public string Id;
        public string DisplayName;
        public Vec3d PositionM;
        public double RadiusM;
    }

    public sealed class DepositFieldTable
    {
        public string StarSystemId;
        public IReadOnlyList<DepositTableEntry> Deposits;

        /// <summary>Parses a single data/world/deposits/{star_system_id}.json file. Reads only
        /// id/display_name/position_m/radius_m from each deposit entry - see file header.</summary>
        public static DepositFieldTable FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            string starSystemId = (string)root["star_system_id"]
                ?? throw new InvalidOperationException("deposit field JSON has no 'star_system_id'");

            var deposits = new List<DepositTableEntry>();
            if (root["deposits"] is JArray array)
            {
                foreach (JToken entry in array)
                {
                    JArray pos = (JArray)entry["position_m"]
                        ?? throw new InvalidOperationException("deposit entry has no 'position_m'");
                    deposits.Add(new DepositTableEntry
                    {
                        Id = (string)entry["id"] ?? throw new InvalidOperationException("deposit entry has no 'id'"),
                        DisplayName = (string)entry["display_name"],
                        PositionM = new Vec3d((double)pos[0], (double)pos[1], (double)pos[2]),
                        RadiusM = (double)entry["radius_m"],
                    });
                    // mineral_id and initial_reserve_kg are present in entry but deliberately
                    // never read here - see DepositTableEntry's doc comment and the file header.
                }
            }

            return new DepositFieldTable { StarSystemId = starSystemId, Deposits = deposits };
        }

        public static DepositFieldTable LoadFromFile(string path) =>
            File.Exists(path) ? FromJson(File.ReadAllText(path)) : null;
    }
}
