// Hand-written. C2 (p1-03-trading). Mirrors Starfall.Mining.InventoryPanelState's single-mutator
// discipline (principle 1, AC-16(a)/I-77): the wallet balance changes ONLY through
// ApplyWalletState - there is no other method that touches Balance, so computing an expected
// proceeds preview (PriceRule/SellPanelCalculator) and even reading a SALE_RECEIPT cannot change
// this type's state; only WALLET_STATE itself can.

using Starfall.Contracts.Generated;

namespace Starfall.Trading
{
    public sealed class WalletMirrorState
    {
        public long BalanceCr { get; private set; }
        public bool HasReceivedAny { get; private set; }

        /// <summary>The only mutator.</summary>
        public void ApplyWalletState(WalletStateMessage message)
        {
            BalanceCr = message.Payload.BalanceCr;
            HasReceivedAny = true;
        }
    }
}
