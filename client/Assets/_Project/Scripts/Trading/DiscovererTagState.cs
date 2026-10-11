// Hand-written. C2 (p1-03-trading). Design doc section 8.3 UI-13: the market panel's
// "발견: Pilot-3f9a" tag next to a listed mineral's name - driven ONLY by
// HISTORICAL_EVENT_NOTICE(MINERAL_DISCOVERED), never by MARKET_STATE.last_sale (design doc:
// "서버 시장 메시지에서 읽지 않는다" - last_sale answers "who sold it last", a different
// question from "who found it first"). Mirrors Starfall.Mining.DiscoveryFeedState's LIVE/BACKFILL
// de-duplication (historical_event_id is deterministic UUIDv5, so the same discovery arriving
// twice must count once) but lives in Starfall.Trading - sibling module, no cross-reference.
//
// Sticky by design (design doc: "한 번 붙으면 사라지지 않는다") - once a mineral_id has a
// discoverer, it is never removed, even if a later message's shape would suggest otherwise.

using System;
using System.Collections.Generic;
using System.Linq;
using Starfall.Contracts.Generated;

namespace Starfall.Trading
{
    public sealed class DiscovererTagState
    {
        readonly HashSet<Guid> _seenHistoricalEventIds = new HashSet<Guid>();
        readonly Dictionary<string, Guid> _discovererByMineralId = new Dictionary<string, Guid>(StringComparer.Ordinal);

        public const string RecordingLine = "발견: 기록 중…";

        /// <summary>Returns true iff this notice added a NEW discoverer entry (false for a
        /// repeat of an already-seen historical_event_id, including the LIVE/BACKFILL duplicate
        /// case) - same contract as DiscoveryFeedState.Apply.</summary>
        public bool Apply(HistoricalEventNoticeMessage message)
        {
            HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent evt =
                message.Payload.HistoricalEvent;
            Guid historicalEventId = evt.HistoricalEventId;

            if (!_seenHistoricalEventIds.Add(historicalEventId)) return false;

            var discoverer = evt.Participants.FirstOrDefault(p => p.Role == "DISCOVERER");
            _discovererByMineralId[evt.Payload.MineralId] = discoverer.EntityId;
            return true;
        }

        public bool TryGetDiscoverer(string mineralId, out Guid discovererActorId) =>
            _discovererByMineralId.TryGetValue(mineralId, out discovererActorId);

        /// <summary>isListed is whether the mineral currently appears in MARKET_STATE's listings
        /// (UI-13 only shows a tag at all next to a listed row) - callers pass it so this never
        /// needs to read MARKET_STATE itself.</summary>
        public string FormatTag(string mineralId, bool isListed, Guid myActorId)
        {
            if (!isListed) return null;
            if (!TryGetDiscoverer(mineralId, out Guid discovererActorId)) return RecordingLine;
            return "발견: " + (discovererActorId == myActorId ? "나" : TradingPilotTag.From(discovererActorId));
        }
    }
}
