// Hand-written. Loads the parts of data/world/systems/*.json (contracts/data/
// star-system.schema.json) the prediction core and the greybox HUD need: play area boundary
// (ADR-0010 section 2 steps 7 and 12) and the four client-only reference markers (C6,
// AC-14(b)). Both server and client read the SAME file, so the boundary the integrator uses
// here is the boundary the server actually enforces - no separate copy of the numbers.

using System;
using System.Collections.Generic;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    /// <summary>A client-side-only static visual (ADR design section, `reference_markers`).
    /// Never sent on the wire, never collided with - it exists so a stationary human has
    /// something to measure motion against (AC-14(b)).</summary>
    public sealed class ReferenceMarker
    {
        public string Id;
        public string DisplayName;
        public string Kind;
        public Vec3d PositionM;
        public double VisualRadiusM;
    }

    public sealed class StarSystemData
    {
        public string Id;

        public double SoftBoundaryRadiusM;
        public double HardBoundaryRadiusM;
        public double BoundaryPullMps2;

        public IReadOnlyList<ReferenceMarker> ReferenceMarkers;

        public static StarSystemData FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            JObject playArea = (JObject)root["play_area"]
                ?? throw new InvalidOperationException("star system JSON has no 'play_area' block");

            var markers = new List<ReferenceMarker>();
            if (root["reference_markers"] is JArray array)
            {
                foreach (JToken entry in array)
                {
                    JArray pos = (JArray)entry["position_m"]
                        ?? throw new InvalidOperationException("reference marker has no 'position_m'");
                    markers.Add(new ReferenceMarker
                    {
                        Id = (string)entry["id"],
                        DisplayName = (string)entry["display_name"],
                        Kind = (string)entry["kind"],
                        PositionM = new Vec3d((double)pos[0], (double)pos[1], (double)pos[2]),
                        VisualRadiusM = (double)entry["visual_radius_m"],
                    });
                }
            }

            return new StarSystemData
            {
                Id = (string)root["id"] ?? throw new InvalidOperationException("star system JSON has no 'id'"),
                SoftBoundaryRadiusM = Required(playArea, "soft_boundary_radius_m"),
                HardBoundaryRadiusM = Required(playArea, "hard_boundary_radius_m"),
                BoundaryPullMps2 = Required(playArea, "boundary_pull_mps2"),
                ReferenceMarkers = markers,
            };
        }

        static double Required(JObject obj, string field)
        {
            JToken token = obj[field];
            if (token == null)
                throw new InvalidOperationException("star system 'play_area' block has no '" + field + "'");
            return (double)token;
        }
    }
}
