// qa r6 §A/F-2: H-9's queue-and-defer fix was applied to ObserverSession in R13, but qa proved
// the fix was UNVERIFIED - deleting ObserverSession's ApplyPendingRebase() call left the suite at
// 237/235 green, because not one test executes ObserverSession at all (it is a MonoBehaviour
// driving a live transport and a CSV file). The leader had reported "compile + suite green" as
// confirmation; that confirmation was worth nothing, which is exactly the §7a failure this slice
// keeps re-learning.
//
// This type is the part of that fix which CAN be executed by a test: the whole queue-one-batch /
// apply-once-per-frame state machine, with no MonoBehaviour, transport, controller or CSV in it.
// ObserverSession now holds one of these instead of five loose fields, so the behaviour under
// test and the behaviour that ships are the same object rather than two copies of one idea.
//
// GreyboxSession still carries its own inline copy of this state (_pendingRebase* +
// ApplyPendingRebase). That is deliberate scope, not an oversight: GreyboxSession's copy IS
// covered - SnapshotRebaseBatchTests exercises the selection rule and GreyboxSendBurstTests
// drives the session end to end through a fake transport - and migrating it is a refactor of a
// file four rounds of evidence already point at. Recorded here so the duplication is a known
// debt with a reason, not a thing someone finds later.

using System;
using Starfall.Sim;

namespace Starfall.Greybox
{
    /// <summary>At most one deferred rebase, the newest snapshot of the current drain batch.
    /// <see cref="TryQueue"/> per arriving snapshot, <see cref="TryTake"/> once per frame.</summary>
    public sealed class PendingRebaseSlot
    {
        long? _tick;
        ShipSimState _confirmed;
        uint? _ackInputSeq;
        Guid _shipId;
        string _presence;

        public bool HasPending => _tick.HasValue;

        /// <summary>Offers a snapshot to the slot. Returns whether it replaced what was queued -
        /// the decision itself is <see cref="SnapshotRebaseBatch.ShouldReplacePending"/>, shared
        /// verbatim with GreyboxSession so the two sessions can never drift apart on which
        /// snapshot of a batch wins.</summary>
        public bool TryQueue(long tick, ShipSimState confirmed, uint? ackInputSeq, Guid shipId, string presence)
        {
            if (!SnapshotRebaseBatch.ShouldReplacePending(tick, _tick)) return false;

            _tick = tick;
            _confirmed = confirmed;
            _ackInputSeq = ackInputSeq;
            _shipId = shipId;
            _presence = presence;
            return true;
        }

        /// <summary>Takes the queued snapshot and empties the slot, so a frame that drained no
        /// snapshots reconciles nothing rather than re-applying the previous frame's - a second
        /// call without an intervening TryQueue always returns false.</summary>
        public bool TryTake(out long tick, out ShipSimState confirmed, out uint? ackInputSeq, out Guid shipId, out string presence)
        {
            tick = default;
            confirmed = default;
            ackInputSeq = default;
            shipId = default;
            presence = default;

            if (!_tick.HasValue) return false;

            tick = _tick.Value;
            confirmed = _confirmed;
            ackInputSeq = _ackInputSeq;
            shipId = _shipId;
            presence = _presence;
            Clear();
            return true;
        }

        /// <summary>Drops anything queued. Called when a session ends so a fresh controller can
        /// never be handed the previous session's snapshot.</summary>
        public void Clear()
        {
            _tick = null;
            _confirmed = default;
            _ackInputSeq = null;
            _shipId = default;
            _presence = null;
        }
    }
}
