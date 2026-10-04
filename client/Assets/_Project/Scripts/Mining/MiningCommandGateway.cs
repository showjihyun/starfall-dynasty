// Hand-written. C3 (p1-02-mining). MINE_RESOURCE send/retry bookkeeping, split from the actual
// network call so the idempotency-key decision (unity-client skill section 2: "명령마다
// command_id를 만들고 대기 목록에 넣는다. 타임아웃 재시도는 같은 command_id로 한다") is testable
// without a live RealtimeClient. The payload is just a deposit_id (MINE_RESOURCE.schema.json),
// so "same intent" reduces to "same deposit_id while a send for it is still unresolved".

using System;
using Starfall.Contracts;

namespace Starfall.Mining
{
    public sealed class MiningCommandGateway
    {
        Guid? _pendingCommandId;
        string _pendingDepositId;

        public Guid? PendingCommandId => _pendingCommandId;
        public string PendingDepositId => _pendingDepositId;

        /// <summary>Pure decision: which command_id a new mine attempt at <paramref
        /// name="depositId"/> should carry. Reuses the pending id only when retrying the SAME
        /// target while it is still unresolved; a different target (or no pending send) gets a
        /// fresh UuidV7. The server treats command_id as the sole idempotency key, so reusing it
        /// for a genuinely different intent would be wrong in the other direction.</summary>
        public Guid ResolveCommandIdFor(string depositId) =>
            (_pendingCommandId.HasValue && string.Equals(_pendingDepositId, depositId, StringComparison.Ordinal))
                ? _pendingCommandId.Value
                : UuidV7.NewGuid();

        /// <summary>Records that <paramref name="commandId"/> was just sent for <paramref
        /// name="depositId"/> - call after a successful TrySendJson.</summary>
        public void MarkSent(Guid commandId, string depositId)
        {
            _pendingCommandId = commandId;
            _pendingDepositId = depositId;
        }

        /// <summary>Call from COMMAND_RESULT handling. Returns true (and clears pending state, so
        /// the next attempt gets a fresh id) only if this result answers the command this gateway
        /// is currently tracking - a result for an unrelated command_id (e.g. SET_SHIP_CONTROL)
        /// is not this gateway's concern and returns false.</summary>
        public bool TryResolve(Guid commandId)
        {
            if (!_pendingCommandId.HasValue || _pendingCommandId.Value != commandId) return false;
            _pendingCommandId = null;
            _pendingDepositId = null;
            return true;
        }
    }
}
