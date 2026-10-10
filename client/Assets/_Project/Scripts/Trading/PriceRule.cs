// Hand-written. C2 (p1-03-trading). Design doc section 3.3's pure price function - the
// EXPECTED PROCEEDS preview (UI-08) only. This never touches the authoritative current
// saturation or price - those reach the client only through MARKET_STATE (principle 1, spec
// p1-03 I-78). The server (server/crates/sim/src/trading/price.rs, S3b) computes the same
// function over the same MarketListingData constants; AC-15 UI-08 requires this preview and the
// server's SC-14 golden to agree on the same input, so every integer operation here matches the
// pseudocode in design doc section 3.3 literally (div truncates toward zero, all non-negative
// operands here so that matches C#'s integer division; multiply is long throughout).
//
// floor_band = ceil((100 - price_floor_pct) / price_drop_pct_per_band)
// saturation_cap = floor_band * band_kg
// band(s) = min(s div band_kg, floor_band)
// price(b) = base_price_cr_per_kg * max(price_floor_pct, 100 - b * price_drop_pct_per_band) div 100
//
// sell(s, q) walks the bands from s, taking the partial amount up to the next band boundary at
// each step, until q is exhausted or the floor band is reached (the floor takes the rest in one
// step, saturation pinned at the cap) - at most floor_band + 1 iterations, never a kg-by-kg loop
// (same bound the server's SC-20/ADR-0016 section 4 measures).

using System;
using Starfall.Sim;

namespace Starfall.Trading
{
    public readonly struct SaleProjection
    {
        public readonly long ProceedsCr;
        public readonly int SaturationAfterKg;

        public SaleProjection(long proceedsCr, int saturationAfterKg)
        {
            ProceedsCr = proceedsCr;
            SaturationAfterKg = saturationAfterKg;
        }
    }

    public static class PriceRule
    {
        /// <summary>ceil((100 - price_floor_pct) / price_drop_pct_per_band) - integer ceiling
        /// division, both operands non-negative.</summary>
        public static int FloorBand(int priceFloorPct, int priceDropPctPerBand)
        {
            int numerator = 100 - priceFloorPct;
            return (numerator + priceDropPctPerBand - 1) / priceDropPctPerBand;
        }

        public static int SaturationCap(int floorBand, int bandKg) => floorBand * bandKg;

        public static int Band(int saturationKg, int bandKg, int floorBand)
        {
            int b = saturationKg / bandKg;
            return b < floorBand ? b : floorBand;
        }

        /// <summary>price(b) = base * max(floor_pct, 100 - b * drop_pct) div 100. Never below
        /// base * floor_pct div 100 (the listing's floor price, >= 1 cr/kg by boot check -
        /// AC-15 UI-08 does not re-verify that bound, it is a server/data concern, S2 m10).</summary>
        public static long PriceAt(int band, long basePriceCrPerKg, int priceFloorPct, int priceDropPctPerBand)
        {
            int pct = 100 - band * priceDropPctPerBand;
            if (pct < priceFloorPct) pct = priceFloorPct;
            return basePriceCrPerKg * pct / 100;
        }

        /// <summary>Walks the bands from saturationBeforeKg over quantityKg, exactly as design
        /// doc section 3.3's sell(s, q) pseudocode. Iteration count is bounded by floorBand + 1,
        /// never by quantityKg (so a 2^31-1 kg preview is as cheap as a 1 kg one).</summary>
        public static SaleProjection Sell(int saturationBeforeKg, int quantityKg, MarketListingData listing, int priceFloorPct, int priceDropPctPerBand)
        {
            int floorBand = FloorBand(priceFloorPct, priceDropPctPerBand);
            int cap = SaturationCap(floorBand, listing.BandKg);

            long proceeds = 0;
            int s = saturationBeforeKg;
            long q = quantityKg;

            while (q > 0)
            {
                int b = Band(s, listing.BandKg, floorBand);
                if (b == floorBand)
                {
                    proceeds += q * PriceAt(floorBand, listing.BasePriceCrPerKg, priceFloorPct, priceDropPctPerBand);
                    s = cap;
                    q = 0;
                }
                else
                {
                    long bandCeilingKg = (long)(b + 1) * listing.BandKg;
                    long take = Math.Min(q, bandCeilingKg - s);
                    proceeds += take * PriceAt(b, listing.BasePriceCrPerKg, priceFloorPct, priceDropPctPerBand);
                    s += (int)take;
                    q -= take;
                }
            }

            return new SaleProjection(proceeds, s);
        }

        /// <summary>UI-08's "이 값으로 N kg 더" preset amount: how much more can be sold at the
        /// CURRENT band before crossing into the next one - (band(s) + 1) * band_kg - s. At the
        /// floor band this is unbounded (design doc section 3.3: "바닥이면 무제한"), signalled by
        /// returning -1 so callers never confuse it with a real, finite amount.</summary>
        public static int AmountToNextBand(int saturationKg, MarketListingData listing, int priceFloorPct, int priceDropPctPerBand)
        {
            int floorBand = FloorBand(priceFloorPct, priceDropPctPerBand);
            int b = Band(saturationKg, listing.BandKg, floorBand);
            if (b == floorBand) return -1;
            return (b + 1) * listing.BandKg - saturationKg;
        }
    }
}
