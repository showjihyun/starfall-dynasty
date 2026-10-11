// Hand-written. Loads data/world/deposits/{star_system_id}.json (contracts/data/
// deposit-field.schema.json) - the client's copy of a PUBLIC table (ADR-0015 section 2).
//
// p1-03-trading: DEPOSIT_FIELD moved to schema_version 2. mineral_id and initial_reserve_kg -
// which this file never read anyway (see below) - no longer even exist in this file; they moved
// to the new server-only DEPOSIT_RESERVE table (data/world/deposit-reserves/{id}.json,
// "consumers": ["server"] only), which never reaches the client copy (ContractFixtureTests.
// ClientDataCopy_MatchesRegistryDerivedSet, SC-56). Before this split, DEPOSIT_FIELD carried
// those two fields itself and relied on this loader's parsing model never giving gameplay code a
// place to read them into - now the file itself carries nothing to leak, the stronger of the two
// defences, but the parsing model stays exactly this narrow on purpose.
//
// DepositTableEntry deliberately has NO field for mineral_id or initial_reserve_kg. This is not
// an oversight: those two values must never reach gameplay code before a deposit is revealed
// (design doc section 3.2, sprint contract SC-71). Whether the deposit is revealed, and what
// mineral it holds, comes from DEPOSIT_FIELD_STATE only (C3 wires that up); this loader answers a
// different question: where is the marker and how big is it, which is public information the
// client needs before any extraction ever happens.

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
                    // schema_version 2 (p1-03-trading, ADR-0015 section 2): mineral_id and
                    // initial_reserve_kg no longer exist in this entry at all - they moved to
                    // DEPOSIT_RESERVE. See DepositTableEntry's doc comment and the file header.
                }
            }

            return new DepositFieldTable { StarSystemId = starSystemId, Deposits = deposits };
        }

        public static DepositFieldTable LoadFromFile(string path) =>
            File.Exists(path) ? FromJson(File.ReadAllText(path)) : null;
    }
}
