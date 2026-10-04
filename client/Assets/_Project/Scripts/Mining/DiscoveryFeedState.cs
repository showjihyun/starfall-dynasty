// Hand-written. C3 (p1-02-mining). "발견된 광물 N / 4" list (design doc section 3.3 item 6) and
// LIVE/BACKFILL de-duplication (02_client_ack.md C1, item "LIVE/BACKFILL 중복 제거"):
// HISTORICAL_EVENT_NOTICE.historical_event.historical_event_id is deterministic (UUIDv5), so the
// same discovery arriving once as LIVE and once as BACKFILL (a session that reconnects, or a
// notice that raced the BACKFILL list) must count once. The denominator (4) is public information
// the client already has from its own data/minerals/ copy (MineralCatalog.Count) - not a
// server-only value, so this does not repeat C2's leak concern.

using System;
using System.Collections.Generic;
using System.Linq;
using Starfall.Contracts.Generated;

namespace Starfall.Mining
{
    public readonly struct DiscoveryEntry
    {
        public readonly string MineralId;
        public readonly string DepositId;
        public readonly Guid DiscovererActorId;
        public readonly long Tick;

        public DiscoveryEntry(string mineralId, string depositId, Guid discovererActorId, long tick)
        {
            MineralId = mineralId;
            DepositId = depositId;
            DiscovererActorId = discovererActorId;
            Tick = tick;
        }
    }

    public sealed class DiscoveryFeedState
    {
        readonly HashSet<Guid> _seenHistoricalEventIds = new HashSet<Guid>();
        readonly Dictionary<string, DiscoveryEntry> _byMineralId = new Dictionary<string, DiscoveryEntry>(StringComparer.Ordinal);

        public int TotalMinerals { get; }

        public DiscoveryFeedState(int totalMinerals) => TotalMinerals = totalMinerals;

        public int DiscoveredCount => _byMineralId.Count;
        public IReadOnlyDictionary<string, DiscoveryEntry> ByMineralId => _byMineralId;

        /// <summary>Returns true iff this notice's historical_event_id had not been seen before
        /// (i.e. this call actually added a new discovery - a LIVE notice for a mineral already
        /// known from BACKFILL, or a repeat of the same LIVE notice, returns false and changes
        /// nothing). Callers use this to decide whether to show the discovery banner (design doc
        /// "산출 알림 ≠ 발견 배너": the banner fires only for a NEW discovery notice).</summary>
        public bool Apply(HistoricalEventNoticeMessage message)
        {
            HistoricalEventNoticeMessage.HistoricalEventNoticePayload.MineralDiscoveredEvent evt =
                message.Payload.HistoricalEvent;
            Guid historicalEventId = evt.HistoricalEventId;

            if (!_seenHistoricalEventIds.Add(historicalEventId)) return false;

            var discoverer = evt.Participants.FirstOrDefault(p => p.Role == "DISCOVERER");
            _byMineralId[evt.Payload.MineralId] = new DiscoveryEntry(
                evt.Payload.MineralId, evt.Payload.DepositId, discoverer.EntityId, evt.Tick);
            return true;
        }

        public bool TryGet(string mineralId, out DiscoveryEntry entry) => _byMineralId.TryGetValue(mineralId, out entry);

        /// <summary>Design doc section 3.3: "발견된 것은 이름·발견자·광맥, 나머지는 '미발견 M종'".
        /// mineralDisplayNames maps mineral_id -> display name (from MineralCatalog, public data).</summary>
        public string FormatSummary(IReadOnlyDictionary<string, string> mineralDisplayNames)
        {
            int undiscovered = TotalMinerals - DiscoveredCount;
            var lines = new List<string> { "발견된 광물 " + DiscoveredCount + " / " + TotalMinerals };
            foreach (KeyValuePair<string, DiscoveryEntry> pair in _byMineralId)
            {
                string name = mineralDisplayNames.TryGetValue(pair.Key, out string n) ? n : pair.Key;
                lines.Add("  " + name + " - " + PilotTag.From(pair.Value.DiscovererActorId) +
                          " (" + pair.Value.DepositId + ")");
            }
            if (undiscovered > 0) lines.Add("  미발견 " + undiscovered + "종");
            return string.Join("\n", lines);
        }
    }
}
