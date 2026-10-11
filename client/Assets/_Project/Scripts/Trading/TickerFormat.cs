// Hand-written. C2 (p1-03-trading). UI-05 ticker line + UI-06 unlisted-mineral privacy.
// Pure formatting/arithmetic only - price_cr_per_kg and listings come from MARKET_STATE; the
// base price (for the percent-down comparison) comes from MarketListingData (data/, public -
// ADR-0015). Never computes a CURRENT price (I-78) - only the percent-of-base DISPLAY figure
// from a price the server already sent.

using System.Globalization;

namespace Starfall.Trading
{
    public static class TickerFormat
    {
        /// <summary>"Ferrosite  3 cr/kg ▼70%" - percent-down from base, floor divide
        /// (design doc section 8.3 UI-05: compared against base_price_cr_per_kg from data/).
        /// basePriceCrPerKg must be > 0 (every listing's base price is schema-positive).</summary>
        public static string Line(string mineralDisplayName, long currentPriceCrPerKg, long basePriceCrPerKg)
        {
            long dropPct = (basePriceCrPerKg - currentPriceCrPerKg) * 100 / basePriceCrPerKg;
            return mineralDisplayName + "  " + currentPriceCrPerKg.ToString(CultureInfo.InvariantCulture) +
                   " cr/kg ▼" + dropPct.ToString(CultureInfo.InvariantCulture) + "%";
        }

        public const string NoListingsLine = "상장된 광물 없음 — 먼저 캐야 한다";

        /// <summary>UI-06: "미상장 N종" - totalMineralCount - listedCount. 0이면 줄 자체가
        /// 없어야 한다(design doc: "모두 상장되면 줄이 사라진다") - 호출자가 count == 0을 보고
        /// 숨긴다, 이 함수는 문구만 만든다.</summary>
        public static string UnlistedCountLine(int totalMineralCount, int listedCount) =>
            "미상장 " + (totalMineralCount - listedCount).ToString(CultureInfo.InvariantCulture) + "종";
    }
}
