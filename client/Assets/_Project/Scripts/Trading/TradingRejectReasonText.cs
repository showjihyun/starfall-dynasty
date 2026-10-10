// Hand-written. C2 (p1-03-trading). SELL_MINERAL rejection sentences for COMMAND_RESULT.reason_code
// (design doc section 8.3 UI-11). Mirrors Starfall.Mining.RejectReasonText's discipline (ADR-0005
// section 4: reason_code's value set is not closed, so an unrecognised code must render, not
// throw) but lives in Starfall.Trading - sibling feature module, no cross-reference to Mining.

namespace Starfall.Trading
{
    public static class TradingRejectReasonText
    {
        /// <summary>Never throws, never returns null. Design doc section 8.3 UI-11 names four
        /// specific sentences (range/speed/holding/cooldown); every other known or unknown code
        /// falls back to the raw code so a future reason the client predates still shows
        /// something rather than crashing or looking like silent acceptance.</summary>
        public static string Resolve(string reasonCode)
        {
            switch (reasonCode)
            {
                case "TARGET_OUT_OF_RANGE": return "도킹 구역 밖이다";
                case "SHIP_TOO_FAST": return "너무 빠르다 — 10 m/s 이하로";
                case "INSUFFICIENT_QUANTITY": return "가진 양보다 많다";
                case "COOLDOWN_ACTIVE": return "잠시 뒤 다시";
                default: return reasonCode ?? "null";
            }
        }
    }
}
