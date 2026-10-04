// Hand-written. Loads data/mining/mining-rules.json (contracts/data/mining-rules.schema.json).
// The three numbers spec p1-02 section 4.2 judges every extraction by. Read here for DISPLAY
// ONLY (greybox "can I mine from here" / "am I slow enough" hints) - the server re-checks the
// same values against its own copy of the same file and is the sole judge of MINE_RESOURCE
// (principle 1). Actual ship position/velocity for the display comes from
// Flight/PredictedShipController.CurrentState, not from anything in this file.

using System;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    public sealed class MiningRulesData
    {
        public string Id;
        public double MiningRangeFromSurfaceM;
        public double MaxShipSpeedMps;
        public int CooldownS;

        public static MiningRulesData FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            JObject extraction = (JObject)root["extraction"]
                ?? throw new InvalidOperationException("mining rules JSON has no 'extraction' block");

            return new MiningRulesData
            {
                Id = (string)root["id"] ?? throw new InvalidOperationException("mining rules JSON has no 'id'"),
                MiningRangeFromSurfaceM = (double)extraction["mining_range_from_surface_m"],
                MaxShipSpeedMps = (double)extraction["max_ship_speed_mps"],
                CooldownS = (int)extraction["cooldown_s"],
            };
        }
    }
}
