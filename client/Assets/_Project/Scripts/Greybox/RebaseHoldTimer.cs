// PR #1 code review, client defect 1 (2026-09-26, team-lead). Pulled out of
// GreyboxSession.ApplyPendingRebase() for the same reason SnapshotRebaseBatch (H-9) and
// PendingRebaseSlot (qa r6 F-2) were pulled out of it before: the state machine lived as bare
// fields plus an inline ternary, and that shape is exactly what hid the bug.
//
// The bug: GreyboxSession kept the held duration in one double field and used
// "_rebaseHoldSeconds > 0.0" as its OWN "am I currently holding" flag:
//
//   double heldSeconds = _rebaseHoldSeconds > 0.0 ? _rebaseHoldSeconds + (now - _rebaseHoldLastRealTime) : 0.0;
//   ...
//   if (holdAction == RebaseHold.Action.HoldAndKeepPredicting) { _rebaseHoldSeconds = heldSeconds; ... }
//
// The FIRST snapshot of a hold has nothing accumulated yet, so heldSeconds is legitimately 0.0
// that time - but that 0.0 is then saved back into _rebaseHoldSeconds. Every following snapshot
// re-reads "_rebaseHoldSeconds > 0.0" as false (the stored value IS 0.0), takes the ": 0.0"
// branch, and saves 0.0 again. The timer can never advance past 0.0. RebaseHold.Evaluate
// (RebaseHold.cs) only returns ForceRebaseDiscardingUnsent once heldSecondsSoFar reaches
// ForcedRebaseAfterSeconds (0.5s, RebaseHold.ForcedRebaseAfterSeconds) - with the timer stuck at
// 0.0 that threshold is unreachable, so a hold started by an unresolved un-sent (carry-forward)
// entry never force-rebases. If the entries that started the hold never resolve (e.g. the
// transport stalls after a carry-forward run, so no later snapshot's ack_input_seq ever catches
// up), the client predicts against that stale, RebaseHold-judged-unsafe history forever.
//
// The fix: separate "currently holding" (a bool) from "how long so far" (the double) - a snapshot
// held for exactly 0 seconds so far is a different state from not holding at all, and the field
// this replaces conflated the two. See RebaseHoldTimerTests.cs for the accumulation contract, and
// the two of GreyboxSession's own fields this type replaces for the wiring (there is exactly one
// implementation now, not a session-local copy that could drift from a tested one).

namespace Starfall.Greybox
{
    /// <summary>Pure state machine: how long (real seconds) a rebase has been held off, across
    /// however many snapshots the hold has spanned. Callers own the clock (Sample/Continue/Reset
    /// take <c>now</c> as a parameter, same as PendingRebaseSlot takes tick as a parameter) so this
    /// is unit-testable without UnityEngine.Time, which does not advance in EditMode outside Play
    /// mode.</summary>
    public sealed class RebaseHoldTimer
    {
        double _heldSeconds;
        float _lastRealTime;
        bool _holding;

        /// <summary>True once <see cref="Continue"/> has run more recently than <see cref="Reset"/>
        /// - i.e. a hold is currently in progress. Exposed for tests; GreyboxSession itself only
        /// needs <see cref="Sample"/>.</summary>
        public bool IsHolding => _holding;

        /// <summary>The accumulated hold duration to hand RebaseHold.Evaluate for THIS snapshot:
        /// 0.0 if no hold is currently in progress, otherwise the running total from the last
        /// <see cref="Continue"/> call plus real time elapsed since then. Call this BEFORE
        /// RebaseHold.Evaluate, then feed its own result to <see cref="Continue"/> or <see cref="Reset"/>
        /// depending on what Evaluate returned - see GreyboxSession.ApplyPendingRebase.</summary>
        public double Sample(float now) => _holding ? _heldSeconds + (now - _lastRealTime) : 0.0;

        /// <summary>Call when RebaseHold.Evaluate returns HoldAndKeepPredicting: records the
        /// duration <see cref="Sample"/> just produced (pass the SAME heldSeconds and now) as the
        /// new baseline for the next snapshot's Sample() call.</summary>
        public void Continue(double heldSeconds, float now)
        {
            _heldSeconds = heldSeconds;
            _lastRealTime = now;
            _holding = true;
        }

        /// <summary>Call when a rebase actually applies this snapshot (RebaseHold.Evaluate returned
        /// RebaseNormally or ForceRebaseDiscardingUnsent) - nothing left to hold.</summary>
        public void Reset(float now)
        {
            _heldSeconds = 0.0;
            _lastRealTime = now;
            _holding = false;
        }
    }
}
