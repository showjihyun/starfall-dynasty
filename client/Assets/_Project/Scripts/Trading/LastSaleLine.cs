// Hand-written. C2 (p1-03-trading). Design doc section 8.3 UI-12: the market panel's
// "마지막 거래: Pilot-3f9a · 14,000 kg · 6분 전" trace line, from MARKET_STATE's
// last_sale (null if the mineral was never sold in this world - "거래 없음").
//
// Pilot tag duplicates Starfall.Mining.PilotTag's logic (last 4 hex chars of the actor_id guid,
// spec I-66) rather than referencing Starfall.Mining - Trading and Mining are sibling feature
// modules with no cross-reference in the asmdef graph (Flight/Mining do not reference each
// other either). WARNING carried over from that file: even the last 4 characters collide in
// this project's dev id scheme - this tag is DISPLAY ONLY, never a lookup key (sprint contract
// section 0.11).

using System;
using System.Globalization;

namespace Starfall.Trading
{
    public static class TradingPilotTag
    {
        public static string From(Guid actorId)
        {
            string text = actorId.ToString("D");
            string last4 = text.Length >= 4 ? text.Substring(text.Length - 4) : text;
            return "Pilot-" + last4;
        }
    }

    public static class LastSaleLine
    {
        public const string NeverSoldLine = "거래 없음";

        /// <summary>myActorId is the viewing client's own actor - "나" takes priority over the
        /// Pilot tag when the seller is the viewer (design doc section 8.3 UI-12: "내가 판
        /// 것이면 '나'"). elapsedSeconds is currentTick/tickHz-derived by the caller (this file
        /// does no tick arithmetic - the caller already has tick_hz and the current tick).</summary>
        public static string Format(Guid? sellerActorId, int quantityKg, double elapsedSeconds, Guid myActorId)
        {
            if (sellerActorId == null) return NeverSoldLine;

            string who = sellerActorId.Value == myActorId ? "나" : TradingPilotTag.From(sellerActorId.Value);
            return "마지막 거래: " + who + " · " + quantityKg.ToString("N0", CultureInfo.InvariantCulture) +
                   " kg · " + FormatElapsed(elapsedSeconds) + " 전";
        }

        static string FormatElapsed(double seconds)
        {
            if (seconds < 60.0) return ((int)seconds).ToString(CultureInfo.InvariantCulture) + "초";
            return ((int)(seconds / 60.0)).ToString(CultureInfo.InvariantCulture) + "분";
        }
    }
}
