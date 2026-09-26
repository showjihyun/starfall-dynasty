// PR #1 code review, client defect 1 (2026-09-26). Direct unit coverage of the accumulation
// state machine GreyboxSession.ApplyPendingRebase() delegates to (RebaseHoldTimer.cs) - see that
// file's header for the exact bug (the timer it replaces could never advance past 0 seconds, so
// RebaseHold.Evaluate's 500ms force-rebase threshold was unreachable from GreyboxSession).
//
// RebaseHold.Evaluate itself (RebaseHold.cs) already has its own tests and is not re-tested here
// - this file is only the piece that hid the defect: the running-duration accumulation across
// repeated Sample/Continue calls, which RebaseHold.Evaluate has no visibility into (it just
// receives whatever heldSecondsSoFar its caller computed).

using NUnit.Framework;
using Starfall.Flight;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class RebaseHoldTimerTests
    {
        [Test]
        public void Sample_BeforeAnyContinue_ReturnsZero()
        {
            var timer = new RebaseHoldTimer();
            Assert.That(timer.IsHolding, Is.False, "test setup: a fresh timer must not already be holding");
            Assert.That(timer.Sample(now: 100f), Is.EqualTo(0.0));
        }

        [Test]
        public void Continue_ThenSampleLater_AccumulatesElapsedRealTime()
        {
            // The headline regression: a hold that starts at heldSeconds == 0.0 (the first
            // snapshot RebaseHold.Evaluate holds on) must still accumulate on the NEXT Sample -
            // the exact case the "_rebaseHoldSeconds > 0.0" guard broke, because it could never
            // tell "holding, currently at exactly 0s" apart from "not holding".
            var timer = new RebaseHoldTimer();

            double firstSample = timer.Sample(now: 10.0f);
            Assert.That(firstSample, Is.EqualTo(0.0), "first snapshot of a hold: nothing accumulated yet");
            timer.Continue(firstSample, now: 10.0f);
            Assert.That(timer.IsHolding, Is.True);

            double secondSample = timer.Sample(now: 10.2f);
            Assert.That(secondSample, Is.EqualTo(0.2).Within(1e-5),
                "200ms of real time must have accumulated since Continue() - this is exactly " +
                "what the buggy inline ternary (_rebaseHoldSeconds > 0.0 ? ... : 0.0) could never " +
                "do starting from a 0.0 baseline");
        }

        [Test]
        public void RepeatedContinue_AccumulatesAcrossManySnapshots_UntilForceRebaseThreshold()
        {
            // §7a: assert BOTH that the hold actually started (IsHolding) and that it crosses
            // RebaseHold.ForcedRebaseAfterSeconds (0.5s) - either alone would let "reached 0.5s"
            // pass vacuously (e.g. a timer that returns 0 forever would never cross it, proving
            // nothing was held; a timer that is always "holding" without accumulating would never
            // cross it either).
            var timer = new RebaseHoldTimer();
            float now = 0f;
            double lastSample = 0.0;

            // Ten snapshots, 60ms apart (600ms total) - mirrors a hold that spans several
            // WORLD_SNAPSHOT arrivals before the server resolves the un-sent entry, or never does.
            for (int i = 0; i < 10; i++)
            {
                now += 0.06f;
                lastSample = timer.Sample(now);
                timer.Continue(lastSample, now);
            }

            Assert.That(timer.IsHolding, Is.True, "test setup: the hold must actually still be in progress");
            Assert.That(lastSample, Is.GreaterThanOrEqualTo(RebaseHold.ForcedRebaseAfterSeconds),
                "600ms of repeated holding must cross the 500ms force-rebase threshold - this is " +
                "the exact value GreyboxSession.ApplyPendingRebase feeds into RebaseHold.Evaluate, " +
                "and the value that stayed pinned at 0.0 under the old inline accumulation");

            // Pairing: run the SAME sequence through RebaseHold.Evaluate (the real caller-facing
            // API) with a history containing one unresolved un-sent entry, to confirm the
            // accumulated duration this test measured is enough to flip Evaluate's own decision -
            // not just a number that happens to be >= 0.5 in isolation.
            var unresolvedHistory = new System.Collections.Generic.List<Starfall.Flight.InputRecord>
            {
                new Starfall.Flight.InputRecord(
                    inputSeq: 5, input: default, stateAfter: default, serverTick: 100, derivedFromSeq: 4),
            };
            RebaseHold.Action action = RebaseHold.Evaluate(unresolvedHistory, ackInputSeq: null, heldSecondsSoFar: lastSample);
            Assert.That(action, Is.EqualTo(RebaseHold.Action.ForceRebaseDiscardingUnsent),
                "the accumulated duration must actually drive RebaseHold.Evaluate to force a rebase");
        }

        [Test]
        public void Reset_StopsHolding_AndNextSampleIsZeroRegardlessOfElapsedTime()
        {
            var timer = new RebaseHoldTimer();
            timer.Continue(timer.Sample(now: 0f), now: 0f);
            timer.Continue(timer.Sample(now: 0.3f), now: 0.3f);
            Assert.That(timer.IsHolding, Is.True, "test setup: must be holding before Reset");

            timer.Reset(now: 0.3f);
            Assert.That(timer.IsHolding, Is.False);

            // A long real-time gap after Reset (e.g. the next snapshot arrives much later) must
            // NOT be reported as held time - Reset means "nothing pending", not "holding at 0".
            Assert.That(timer.Sample(now: 5.0f), Is.EqualTo(0.0));
        }

        [Test]
        public void Continue_AfterReset_StartsANewHoldFromZero_NotFromTheOldBaseline()
        {
            var timer = new RebaseHoldTimer();
            timer.Continue(0.4, now: 0f); // an earlier hold that almost reached the threshold
            timer.Reset(now: 0.4f);       // ...then resolved before forcing

            double freshSample = timer.Sample(now: 0.4f);
            Assert.That(freshSample, Is.EqualTo(0.0), "a new hold must not inherit the previous hold's baseline");
            timer.Continue(freshSample, now: 0.4f);

            Assert.That(timer.Sample(now: 0.45f), Is.EqualTo(0.05).Within(1e-5));
        }
    }
}
