// Hand-written. C2 (p1-03-trading). Design doc section 8.3 UI-07 (market panel row) and UI-08
// (quantity preset + expected proceeds). Pure data/formatting - current saturation and price
// come from the caller's MARKET_STATE listing, holdings from INVENTORY_STATE; this file never
// reads either message itself (I-76/I-77/I-78, AC-16).

using System.Globalization;
using Starfall.Sim;

namespace Starfall.Trading
{
    /// <summary>UI-07: one market panel row. holdingKg is the actor's inventory of this
    /// mineral (0 if none - the row still shows, dimmed, design doc: "내 보유 0인 행도 보이되
    /// 흐리게").</summary>
    public readonly struct SellPanelRow
    {
        public readonly string MineralId;
        public readonly string DisplayName;
        public readonly long CurrentPriceCrPerKg;
        public readonly int SaturationKg;
        public readonly int SaturationCapKg;
        public readonly int HoldingKg;

        public SellPanelRow(string mineralId, string displayName, long currentPriceCrPerKg,
            int saturationKg, int saturationCapKg, int holdingKg)
        {
            MineralId = mineralId;
            DisplayName = displayName;
            CurrentPriceCrPerKg = currentPriceCrPerKg;
            SaturationKg = saturationKg;
            SaturationCapKg = saturationCapKg;
            HoldingKg = holdingKg;
        }

        public bool IsDimmed => HoldingKg == 0;
    }

    /// <summary>UI-07's per-row band phrase - "이 값으로 1,400 kg 더" or "바닥가" at the floor
    /// band (design doc section 8.3 UI-07: "포화 막대(0~상한), `이 값으로 N kg 더`(바닥이면
    /// `바닥가`)"). Distinct from UI-08's selected-row preset line (SellPanelCalculator.FormatLine)
    /// - this one has no "예상" amount and shows on EVERY row, not just the selected one.</summary>
    public static class SellPanelBandPhrase
    {
        public const string AtFloorLine = "바닥가";

        public static string Format(int amountToNextBandKg) =>
            amountToNextBandKg < 0
                ? AtFloorLine
                : "이 값으로 " + amountToNextBandKg.ToString("N0", CultureInfo.InvariantCulture) + " kg 더";
    }

    /// <summary>UI-07's ↑/↓ row selection - wraps around both ends (design doc does not specify
    /// clamping vs. wrapping explicitly, but a greybox list with no scrollbar reads better as a
    /// ring). Pure index arithmetic; GreyboxTradingSession.Update() uses this instead of its own
    /// inline modulo so the two never drift apart.</summary>
    public static class SellPanelSelection
    {
        public static int MoveUp(int currentIndex, int rowCount) =>
            rowCount <= 0 ? 0 : (currentIndex - 1 + rowCount) % rowCount;

        public static int MoveDown(int currentIndex, int rowCount) =>
            rowCount <= 0 ? 0 : (currentIndex + 1) % rowCount;
    }

    /// <summary>UI-08: the "이 값까지 N kg 더 / 전량 M kg · 예상 ₵P" line's two presets -
    /// "up to the current band" and "all of it". At the floor band (AmountToNextBand returns -1)
    /// the two presets collapse into one (design doc: "바닥이면... 프리셋이 '전량'과 같아져
    /// 하나만 보인다") - HasDistinctUpToBandPreset says whether to show both or just one.</summary>
    public readonly struct SellQuantityPresets
    {
        public readonly int UpToBandKg;
        public readonly bool HasDistinctUpToBandPreset;
        public readonly int AllKg;
        public readonly long ExpectedProceedsUpToBandCr;
        public readonly long ExpectedProceedsAllCr;

        public SellQuantityPresets(int upToBandKg, bool hasDistinctUpToBandPreset, int allKg,
            long expectedProceedsUpToBandCr, long expectedProceedsAllCr)
        {
            UpToBandKg = upToBandKg;
            HasDistinctUpToBandPreset = hasDistinctUpToBandPreset;
            AllKg = allKg;
            ExpectedProceedsUpToBandCr = expectedProceedsUpToBandCr;
            ExpectedProceedsAllCr = expectedProceedsAllCr;
        }

        /// <summary>No holding - design doc: "보유 0이면 '팔 것 없음', Enter 무효".</summary>
        public static readonly SellQuantityPresets Empty = new SellQuantityPresets(0, false, 0, 0, 0);
        public const string NothingToSellLine = "팔 것 없음";

        /// <summary>design doc section 8.3 UI-08: "보유 0이면 ... Enter 무효" - the caller
        /// (GreyboxTradingSession.TrySendSell) checks this before sending SELL_MINERAL, so Enter
        /// on an empty preset is structurally a no-op rather than a sentinel string the caller
        /// has to pattern-match.</summary>
        public bool CanSell => AllKg > 0;
    }

    public static class SellPanelCalculator
    {
        /// <summary>Row holding 0 kg has nothing to sell; row holding &gt; 0 always has at least
        /// the "all" preset, and a distinct "up to band" preset unless the current band is the
        /// floor band or the holding does not reach the next band boundary.</summary>
        public static SellQuantityPresets ComputePresets(
            int saturationKg, int holdingKg, MarketListingData listing, int priceFloorPct, int priceDropPctPerBand)
        {
            if (holdingKg <= 0) return SellQuantityPresets.Empty;

            int amountToNextBand = PriceRule.AmountToNextBand(saturationKg, listing, priceFloorPct, priceDropPctPerBand);
            bool hasDistinctUpToBand = amountToNextBand > 0 && amountToNextBand < holdingKg;
            int upToBandKg = hasDistinctUpToBand ? amountToNextBand : holdingKg;

            SaleProjection upToBand = PriceRule.Sell(saturationKg, upToBandKg, listing, priceFloorPct, priceDropPctPerBand);
            SaleProjection all = PriceRule.Sell(saturationKg, holdingKg, listing, priceFloorPct, priceDropPctPerBand);

            return new SellQuantityPresets(upToBandKg, hasDistinctUpToBand, holdingKg, upToBand.ProceedsCr, all.ProceedsCr);
        }

        /// <summary>"이 값까지 1,400 kg / 전량 3,400 kg · 예상 ₸ 12,600" - or just the
        /// "전량" half when the two presets collapsed, or NothingToSellLine when empty.</summary>
        public static string FormatLine(SellQuantityPresets presets)
        {
            if (presets.AllKg <= 0) return SellQuantityPresets.NothingToSellLine;

            string allPart = "전량 " + presets.AllKg.ToString("N0", CultureInfo.InvariantCulture) +
                              " kg · 예상 ₵ " + presets.ExpectedProceedsAllCr.ToString("N0", CultureInfo.InvariantCulture);

            if (!presets.HasDistinctUpToBandPreset) return allPart;

            string upToBandPart = "이 값까지 " + presets.UpToBandKg.ToString("N0", CultureInfo.InvariantCulture) +
                                   " kg · 예상 ₵ " + presets.ExpectedProceedsUpToBandCr.ToString("N0", CultureInfo.InvariantCulture);
            return upToBandPart + " / " + allPart;
        }
    }
}
