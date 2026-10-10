// Hand-written. C3 (p1-03-trading). SELL_MINERAL send/retry bookkeeping - mirrors
// Starfall.Mining.MiningCommandGateway's discipline (unity-client skill section 2: a fresh
// command_id per NEW intent, the SAME command_id on a retry of the same unresolved intent).
// SELL_MINERAL's full intent is (market_id, mineral_id, quantity_kg) - all three must match for
// a retry to reuse the pending id; any difference (even just quantity) is a different intent and
// gets a fresh UuidV7 (the server's idempotency key is command_id alone, so reusing it for a
// genuinely different sale would be wrong in the other direction - same reasoning as Mining's
// deposit_id-only key, just a wider tuple since SELL_MINERAL carries three fields where
// MINE_RESOURCE carries one).

using System;
using Starfall.Contracts;

namespace Starfall.Trading
{
    public sealed class TradingCommandGateway
    {
        Guid? _pendingCommandId;
        string _pendingMarketId;
        string _pendingMineralId;
        int _pendingQuantityKg;

        public Guid? PendingCommandId => _pendingCommandId;

        public Guid ResolveCommandIdFor(string marketId, string mineralId, int quantityKg) =>
            (_pendingCommandId.HasValue &&
             string.Equals(_pendingMarketId, marketId, StringComparison.Ordinal) &&
             string.Equals(_pendingMineralId, mineralId, StringComparison.Ordinal) &&
             _pendingQuantityKg == quantityKg)
                ? _pendingCommandId.Value
                : UuidV7.NewGuid();

        public void MarkSent(Guid commandId, string marketId, string mineralId, int quantityKg)
        {
            _pendingCommandId = commandId;
            _pendingMarketId = marketId;
            _pendingMineralId = mineralId;
            _pendingQuantityKg = quantityKg;
        }

        public bool TryResolve(Guid commandId)
        {
            if (!_pendingCommandId.HasValue || _pendingCommandId.Value != commandId) return false;
            _pendingCommandId = null;
            _pendingMarketId = null;
            _pendingMineralId = null;
            _pendingQuantityKg = 0;
            return true;
        }
    }
}
