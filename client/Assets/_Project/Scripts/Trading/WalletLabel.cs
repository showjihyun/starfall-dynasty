// Hand-written. C2 (p1-03-trading). UI-04: "₵ 20,000" with thousand separators, "₵ 0" for a
// fresh actor. Pure formatting only - the VALUE comes from WALLET_STATE.payload.balance_cr alone
// (I-77, AC-16(a): the client never computes a balance), this file only turns that long into
// the display string.

using System.Globalization;

namespace Starfall.Trading
{
    public static class WalletLabel
    {
        public static string Format(long balanceCr) =>
            "₵ " + balanceCr.ToString("N0", CultureInfo.InvariantCulture);
    }
}
