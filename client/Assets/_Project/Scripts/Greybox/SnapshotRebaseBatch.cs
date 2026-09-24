// H-9 (수신측 스냅샷 중복 처리, 02_sprint_contract.md 리더 메시지 2026-09-23).
//
// RealtimeClient.Pump() drains every queued WORLD_SNAPSHOT synchronously, one OnMessage() call
// per frame's worth of backlog (RealtimeClient.cs OnMessage -> handler(contract), no batching or
// coalescing anywhere in that path). If two WORLD_SNAPSHOT for the controlled ship land in the
// same drain (a real possibility whenever the client frame is slower than snapshot_interval_ticks
// - the exact hitch scenario SC-89/TickCatchUp already handles on the SEND side), GreyboxSession
// used to run RebaseHold.Evaluate + PredictedShipController.Reconcile for EACH one in turn. The
// older snapshot's reconcile executing after the newer one had already been processed could
// rewind CurrentState to a stale tick - a rebase must only ever be driven by the batch's latest
// snapshot.
//
// This is the pure "which one wins" decision, isolated so it is testable without RealtimeClient,
// GreyboxSession or a real transport (the exact gap T-4/T-6 closed for TickCatchUp - see that
// file's header). It says nothing about the interpolation buffer: older snapshots in the same
// batch still belong there in full (RemoteShipRegistry.OnSnapshot is called unconditionally, for
// every message, by the caller) - this type only ever gates the SELF-SHIP rebase/reconcile path.

namespace Starfall.Greybox
{
    public static class SnapshotRebaseBatch
    {
        /// <summary>True when a snapshot at <paramref name="candidateTick"/> should replace
        /// whatever tick is currently pending as "the one to rebase on this frame".</summary>
        /// <param name="candidateTick">The just-arrived snapshot's tick.</param>
        /// <param name="pendingTick">The tick already queued for rebase this frame, or null if
        /// none is queued yet.</param>
        /// <remarks>Strictly greater-than, not greater-or-equal: a same-tick resend (the server
        /// retransmitting, or a duplicate frame) does not need to replace an already-pending
        /// snapshot of that same tick - there is nothing newer to gain from doing so, and
        /// keeping the first-seen copy for a tied tick makes the selection deterministic under
        /// FIFO arrival instead of "last of any ties wins" for no reason.</remarks>
        public static bool ShouldReplacePending(long candidateTick, long? pendingTick)
        {
            return !pendingTick.HasValue || candidateTick > pendingTick.Value;
        }
    }
}
