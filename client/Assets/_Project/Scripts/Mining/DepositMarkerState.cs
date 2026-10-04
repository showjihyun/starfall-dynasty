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
            // Design doc §8 item 1 / SC-68 절차서 literal string: unrevealed deposits read
            // "미확인 광맥" (not just "미확인") so a human scanning the HUD recognizes the exact
            // phrase the spec names.
            if (!revealed)
                return label + " - 미확인 광맥";

            string mineral = mineralDisplayName ?? wire.MineralId;
            string remaining = wire.RemainingKg.HasValue ? wire.RemainingKg.Value + " kg" : "?";
            string line = label + " - " + mineral + " 잔량 " + remaining;

            if (discovery.HasValue)
                line += " (최초 발견: " + PilotTag.From(discovery.Value.DiscovererActorId) + ")";

            return line;
        }

        /// <summary>C4 (SC-68 재시도): <see cref="FormatLine"/> + 함선에서 이 광맥까지의 직선
        /// 거리(m/km, <see cref="DistanceLabel"/>). 사람이 화면 라벨만 보고 방향을 잡을 수 있게
        /// (team-lead 지시) - 거리는 중심 거리(채굴 사거리 판정의 표면 거리와는 다른 값, 그건
        /// <see cref="MiningRangeEvaluator"/>가 따로 계산한다).</summary>
        public static string FormatLineWithDistance(DepositTableEntry table,
            DepositFieldStateMessage.DepositFieldStatePayload.DepositState wire,
            DiscoveryEntry? discovery, string mineralDisplayName, double distanceToCenterM)
        {
            return FormatLine(table, wire, discovery, mineralDisplayName) + " · " + DistanceLabel.Format(distanceToCenterM);
        }
    }
}
