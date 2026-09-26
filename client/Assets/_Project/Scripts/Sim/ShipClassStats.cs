// Hand-written. Loads the `movement` block of a data/ships/*.json row (contracts/data/
// ship-class.schema.json). SI f64 values, read as-is - no unit conversion, the data table is
// ADR-0009 section 2's documented exception to wire quantisation.
//
// This is a data table, not a wire message: AC-10(d) deliberately generates no C# DTO for
// kind "data" (contracts/registry/types.json). The client reads its data/ copy directly
// (ADR-0012 section 7) and this is that direct read.

using System;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    public sealed class ShipClassStats
    {
        public string Id;

        public double MaxSpeedMps;
        public double MainThrustMps2;
        public double ReverseThrustMps2;
        public double LateralThrustMps2;
        public double BrakeDecelMps2;

        public double AssistLinearDecelMps2;
        public double AssistLateralDecelMps2;

        public double TurnRateMaxDegS;
        public double TurnAccelDegS2;
        public double TurnGainDegSPerSinHalf;
        public double TurnDeadzoneSinHalf;

        public double RollRateMaxDegS;
        public double RollAccelDegS2;
        public double AutoLevelRateDegS;
        public double AutoLevelDeadzoneSin;

        public static ShipClassStats FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            JObject movement = (JObject)root["movement"]
                ?? throw new InvalidOperationException("ship class JSON has no 'movement' block");

            return new ShipClassStats
            {
                Id = (string)root["id"] ?? throw new InvalidOperationException("ship class JSON has no 'id'"),
                MaxSpeedMps = Required(movement, "max_speed_mps"),
                MainThrustMps2 = Required(movement, "main_thrust_mps2"),
                ReverseThrustMps2 = Required(movement, "reverse_thrust_mps2"),
                LateralThrustMps2 = Required(movement, "lateral_thrust_mps2"),
                BrakeDecelMps2 = Required(movement, "brake_decel_mps2"),
                AssistLinearDecelMps2 = Required(movement, "assist_linear_decel_mps2"),
                AssistLateralDecelMps2 = Required(movement, "assist_lateral_decel_mps2"),
                TurnRateMaxDegS = Required(movement, "turn_rate_max_deg_s"),
                TurnAccelDegS2 = Required(movement, "turn_accel_deg_s2"),
                TurnGainDegSPerSinHalf = Required(movement, "turn_gain_deg_s_per_sin_half"),
                TurnDeadzoneSinHalf = Required(movement, "turn_deadzone_sin_half"),
                RollRateMaxDegS = Required(movement, "roll_rate_max_deg_s"),
                RollAccelDegS2 = Required(movement, "roll_accel_deg_s2"),
                AutoLevelRateDegS = Required(movement, "auto_level_rate_deg_s"),
                AutoLevelDeadzoneSin = Required(movement, "auto_level_deadzone_sin"),
            };
        }

        static double Required(JObject obj, string field)
        {
            JToken token = obj[field];
            if (token == null)
                throw new InvalidOperationException("ship class 'movement' block has no '" + field + "'");
            return (double)token;
        }
    }
}
