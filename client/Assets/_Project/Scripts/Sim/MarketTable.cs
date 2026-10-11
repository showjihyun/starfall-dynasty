// Hand-written. Loads data/market/{id}.json (contracts/data/market.schema.json) - the client's
// copy of a PUBLIC table (ADR-0015: the whole file is part of the client copy). Read for TWO
// purposes only: (1) display (exchange geometry UI-01, docking rules UI-03, listing constants
// for the ticker's base-price comparison UI-05) and (2) the expected-proceeds PREVIEW (UI-08,
// design doc section 3.3's pure price function, Starfall.Trading.PriceRule) - never for the
// authoritative current saturation or price, which reach the client only through MARKET_STATE
// (principle 1, spec p1-03 I-78: "THE CLIENT NEVER COMPUTES A CURRENT PRICE OR SATURATION").
//
// Mirrors DepositFieldTable/MiningRulesData's narrow-parsing-model discipline: this type carries
// exactly the fields design doc section 8.3/3.3 name a use for, not a 1:1 mirror of the schema.

using System;
using System.Collections.Generic;
using Newtonsoft.Json.Linq;
using Starfall.Contracts;

namespace Starfall.Sim
{
    /// <summary>One listed mineral's price-rule constants (design doc section 3.3) - the
    /// designer-owned numbers PriceRule's pure functions take as input.</summary>
    public sealed class MarketListingData
    {
        public string MineralId;
        public long BasePriceCrPerKg;
        public int BandKg;
        public int RecoveryKg;
    }

    public sealed class MarketTable
    {
        public string Id;
        public string StarSystemId;
        public string DisplayName;
        public string PricingVersion;
        public Vec3d PositionM;
        public double RadiusM;

        public double DockRangeFromSurfaceM;
        public double MaxShipSpeedMps;
        public int SellCooldownS;
        public int PriceDropPctPerBand;
        public int PriceFloorPct;
        public int RecoveryIntervalS;

        public IReadOnlyList<MarketListingData> Listings;

        /// <summary>UI-07/08: find a listing's constants by mineral_id, or null if this market
        /// does not list it (an unlisted mineral never reaches the sell panel - MARKET_STATE's
        /// listings array is the only source of "is this mineral listed", UI-06).</summary>
        public MarketListingData FindListing(string mineralId)
        {
            foreach (MarketListingData listing in Listings)
                if (string.Equals(listing.MineralId, mineralId, StringComparison.Ordinal))
                    return listing;
            return null;
        }

        public static MarketTable FromJson(string json)
        {
            JObject root = ContractJson.ReadObject(json);
            JObject rules = (JObject)root["rules"]
                ?? throw new InvalidOperationException("market JSON has no 'rules' block");
            JArray pos = (JArray)root["position_m"]
                ?? throw new InvalidOperationException("market JSON has no 'position_m'");

            var listings = new List<MarketListingData>();
            if (root["listings"] is JArray array)
            {
                foreach (JToken entry in array)
                {
                    listings.Add(new MarketListingData
                    {
                        MineralId = (string)entry["mineral_id"] ?? throw new InvalidOperationException("listing has no 'mineral_id'"),
                        BasePriceCrPerKg = (long)entry["base_price_cr_per_kg"],
                        BandKg = (int)entry["band_kg"],
                        RecoveryKg = (int)entry["recovery_kg"],
                    });
                }
            }

            return new MarketTable
            {
                Id = (string)root["id"] ?? throw new InvalidOperationException("market JSON has no 'id'"),
                StarSystemId = (string)root["star_system_id"] ?? throw new InvalidOperationException("market JSON has no 'star_system_id'"),
                DisplayName = (string)root["display_name"],
                PricingVersion = (string)root["pricing_version"],
                PositionM = new Vec3d((double)pos[0], (double)pos[1], (double)pos[2]),
                RadiusM = (double)root["radius_m"],
                DockRangeFromSurfaceM = (double)rules["dock_range_from_surface_m"],
                MaxShipSpeedMps = (double)rules["max_ship_speed_mps"],
                SellCooldownS = (int)rules["sell_cooldown_s"],
                PriceDropPctPerBand = (int)rules["price_drop_pct_per_band"],
                PriceFloorPct = (int)rules["price_floor_pct"],
                RecoveryIntervalS = (int)rules["recovery_interval_s"],
                Listings = listings,
            };
        }

        public static MarketTable LoadFromFile(string path) =>
            System.IO.File.Exists(path) ? FromJson(System.IO.File.ReadAllText(path)) : null;
    }
}
