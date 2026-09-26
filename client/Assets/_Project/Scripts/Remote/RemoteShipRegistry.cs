// Hand-written. Tracks one RemoteShipBuffer per ship_id seen in WORLD_SNAPSHOT, other than the
// controlled ship (which is predicted, not interpolated - ADR-0012 section 1). A ship absent
// from a snapshot's `ships` array is removed immediately: "스냅샷에 없는 ship_id는 사라진 것이다
// (디스폰). 별도 메시지가 없다" (design section 6.4) - there is no separate despawn message to
// wait for, and this must not be confused with the snapshot STREAM going quiet (that case is
// RemoteShipBuffer's own extrapolate-then-freeze, and does not remove anything).

using System;
using System.Collections.Generic;
using Starfall.Sim;

namespace Starfall.Remote
{
    public sealed class RemoteShipRegistry
    {
        readonly Dictionary<Guid, RemoteShipBuffer> _buffers = new Dictionary<Guid, RemoteShipBuffer>();

        public IReadOnlyDictionary<Guid, RemoteShipBuffer> Buffers => _buffers;

        /// <summary>One call per WORLD_SNAPSHOT, with every ship in it EXCEPT
        /// controlled_ship_id. Adds a sample to each ship's buffer (creating the buffer on
        /// first sight) and removes any previously-tracked ship not present this time.</summary>
        public void OnSnapshot(long tick, IEnumerable<(Guid ShipId, ShipSimState State, string Presence)> otherShips)
        {
            if (otherShips == null) throw new ArgumentNullException(nameof(otherShips));

            var seen = new HashSet<Guid>();
            foreach ((Guid shipId, ShipSimState state, string presence) in otherShips)
            {
                seen.Add(shipId);
                if (!_buffers.TryGetValue(shipId, out RemoteShipBuffer buffer))
                {
                    buffer = new RemoteShipBuffer();
                    _buffers[shipId] = buffer;
                }
                buffer.AddSample(tick, state, presence);
            }

            List<Guid> toRemove = null;
            foreach (Guid shipId in _buffers.Keys)
            {
                if (seen.Contains(shipId)) continue;
                (toRemove ??= new List<Guid>()).Add(shipId);
            }

            if (toRemove == null) return;
            foreach (Guid shipId in toRemove) _buffers.Remove(shipId);
        }

        public bool TryGetBuffer(Guid shipId, out RemoteShipBuffer buffer) => _buffers.TryGetValue(shipId, out buffer);
    }
}
