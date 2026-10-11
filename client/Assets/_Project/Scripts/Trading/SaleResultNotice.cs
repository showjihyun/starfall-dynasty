// Hand-written. C2 (p1-03-trading). Design doc section 8.3 UI-09 + section 2.1a: the sale
// result notice always shows the ACTUAL SALE_RECEIPT values, never the UI-08 preview - and when
// the actual proceeds are less than what was previewed (someone else sold first), a second line
// names the difference. When the actual is >= the preview (recovery tick landed first - not a
// loss), there is no second line.

using System.Globalization;

namespace Starfall.Trading
{
    public readonly struct SaleResultNotice
    {
        public readonly string FirstLine;
        public readonly string SecondLine;

        public SaleResultNotice(string firstLine, string secondLine)
        {
            FirstLine = firstLine;
            SecondLine = secondLine;
        }

        public bool HasSecondLine => SecondLine != null;
    }

    public static class SaleResultNoticeBuilder
    {
        /// <summary>mineralDisplayName/quantityKg/actualPriceCrPerKg/actualProceedsCr come from
        /// the accepted SALE_RECEIPT (the only source of truth here - design doc section 8.3
        /// UI-09: "예상과 다르면 실제를 쓴다"). expectedProceedsCr is the UI-08 preview shown
        /// before Enter was pressed, kept by the caller across the round trip.</summary>
        public static SaleResultNotice Build(
            string mineralDisplayName, int quantityKg, long actualPriceCrPerKg, long actualProceedsCr, long expectedProceedsCr)
        {
            string firstLine = "+₵ " + actualProceedsCr.ToString("N0", CultureInfo.InvariantCulture) +
                                " · " + mineralDisplayName + " " + quantityKg.ToString("N0", CultureInfo.InvariantCulture) +
                                " kg 판매 (" + actualPriceCrPerKg.ToString(CultureInfo.InvariantCulture) + " cr/kg)";

            if (expectedProceedsCr <= actualProceedsCr) return new SaleResultNotice(firstLine, null);

            long difference = expectedProceedsCr - actualProceedsCr;
            string secondLine = "예상보다 ₵" + difference.ToString("N0", CultureInfo.InvariantCulture) +
                                 " 적다 — 직전에 다른 판매가 있었다";
            return new SaleResultNotice(firstLine, secondLine);
        }
    }
}
