// Hand-written. C3 (p1-02-mining). Deposit marker display: unrevealed/revealed, remaining, and
// "최초 발견: Pilot-xxxx" - joining three sources per 02_client_ack.md C3:
//   - DepositFieldTable (C2): position/radius/display_name only, from the client's copy - never
//     mineral_id or initial_reserve_kg (see DepositFieldTable.cs).
//   - DEPOSIT_FIELD_STATE (server, this class's only mutator): "all four null or all four set"
//     per deposit (contract description, I-68) - mineral_id == null means unrevealed.
//   - DiscoveryFeedState (HISTORICAL_EVENT_NOTICE, joined by deposit_id): the first-discoverer
//     tag DEPOSIT_FIELD_STATE itself does not carry.

using System.Collections.Generic;
using Starfall.Contracts.Generated;
using Starfall.Sim;

namespace Starfall.Mining
{
    /// <summary>Mutable, but only through <see cref="ApplyDepositFieldState"/> - same single-
    /// mutator discipline as InventoryPanelState, for the same reason: the revealed/mineral/
    /// remaining fields must come from the server message, never be inferred locally.</summary>
    public sealed class DepositMarkerState
    {
        readonly Dictionary<string, DepositFieldStateMessage.DepositFieldStatePayload.DepositState> _byDepositId =
            new Dictionary<string, DepositFieldStateMessage.DepositFieldStatePayload.DepositState>(System.StringComparer.Ordinal);

        public void ApplyDepositFieldState(DepositFieldStateMessage message)
        {
            foreach (var deposit in message.Payload.Deposits)
                _byDepositId[deposit.DepositId] = deposit;
        }

        public bool TryGetWireState(string depositId, out DepositFieldStateMessage.DepositFieldStatePayload.DepositState state) =>
            _byDepositId.TryGetValue(depositId, out state);

        /// <summary>One HUD line for a marker. Pure function of its inputs so it is testable
        /// without a live session - table/wire/discovery/mineral name are each optional
        /// independently (a marker can be drawn before any DEPOSIT_FIELD_STATE has arrived, or
        /// before a discoverer is known even though the deposit is revealed - e.g. this is the
        /// very extraction that revealed it).</summary>
        public static string FormatLine(DepositTableEntry table,
            DepositFieldStateMessage.DepositFieldStatePayload.DepositState wire,
            DiscoveryEntry? discovery, string mineralDisplayName)
        {
            string label = table.DisplayName + " (" + table.Id + ")";

            bool revealed = wire != null && wire.MineralId != null;
            if (!revealed)
                return label + " - 미확인";

            string mineral = mineralDisplayName ?? wire.MineralId;
            string remaining = wire.RemainingKg.HasValue ? wire.RemainingKg.Value + " kg" : "?";
            string line = label + " - " + mineral + " 잔량 " + remaining;

            if (discovery.HasValue)
                line += " (최초 발견: " + PilotTag.From(discovery.Value.DiscovererActorId) + ")";

            return line;
        }
    }
}
