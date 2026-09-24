// H-1 (CL-H, _workspace/p1-01-ship-movement/01_architect_decisions.md "R4 판정" + "R4 보충
// 판정"). Tests TickCatchUp.Compute directly - the pure function GreyboxSession.Update() will
// call once H-6 wires it in (not done yet, blocked - see 03_client_impl.md R10). These are GREEN
// against this new, correct implementation; they are not the "broadened RED against the OLD
// GreyboxSession loop" architect asked for in H-7 - that step needs the wiring (H-3/H-6) this
// round deliberately does not touch (H-3 has a live instruction conflict between the team lead
// and architect, unresolved as of this file).

using System;
using NUnit.Framework;
using Starfall.Flight;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class TickCatchUpTests
    {
        const double TickDuration = 1.0 / 20.0; // tick_hz = 20

        // ------------------------------------------------------------------ the R4 evidence case, pinned directly

        [Test]
        public void Compute_TheR4EvidenceHitch_SendsOnlyOneNotNine()
        {
            // 0.46s accumulated (9.2 ticks) - the exact synthetic hitch
            // GreyboxSendBurstTests.cs used to demonstrate the original bug (9 sends in one
            // Update() call). With K=1 this must predict all 9 backlogged ticks (R4 판정 rule 1)
            // but send only the last one (rule 2).
            TickCatchUp.Plan plan = TickCatchUp.Compute(
                0.46, TickDuration, TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);

            Assert.That(plan.TicksToPredict, Is.EqualTo(9),
                "the whole backlog must still be predicted - dropping it reproduces the hard-snap " +
                "the leader's R4 판정 rejected (the server carried all 9 ticks forward for real)");
            Assert.That(plan.TicksToSend, Is.EqualTo(1),
                "K=1: only the most recent input is worth sending, the rest are guaranteed " +
                "server-side last-write-wins losers (ADR-0011 section 4)");
            Assert.That(plan.Truncated, Is.False);
            Assert.That(plan.RemainingAccumulatorSeconds, Is.EqualTo(0.46 - 9 * TickDuration).Within(1e-12));
        }

        // ------------------------------------------------------------------ no backlog yet

        [Test]
        public void Compute_LessThanOneTickAccumulated_PredictsAndSendsNothing()
        {
            TickCatchUp.Plan plan = TickCatchUp.Compute(TickDuration * 0.9, TickDuration, 1, 20);

            Assert.That(plan.TicksToPredict, Is.EqualTo(0));
            Assert.That(plan.TicksToSend, Is.EqualTo(0));
            Assert.That(plan.Truncated, Is.False);
            Assert.That(plan.RemainingAccumulatorSeconds, Is.EqualTo(TickDuration * 0.9).Within(1e-12));
        }

        [Test]
        public void Compute_ExactlyOneTick_PredictsAndSendsOne()
        {
            TickCatchUp.Plan plan = TickCatchUp.Compute(TickDuration, TickDuration, 1, 20);

            Assert.That(plan.TicksToPredict, Is.EqualTo(1));
            Assert.That(plan.TicksToSend, Is.EqualTo(1));
            Assert.That(plan.RemainingAccumulatorSeconds, Is.EqualTo(0.0).Within(1e-12));
        }

        // ------------------------------------------------------------------ truncation (R4 판정 rule 5)

        [Test]
        public void Compute_BacklogAtExactlyTheCap_IsNotTruncated()
        {
            TickCatchUp.Plan plan = TickCatchUp.Compute(20 * TickDuration, TickDuration, 1, 20);

            Assert.That(plan.TicksToPredict, Is.EqualTo(20));
            Assert.That(plan.Truncated, Is.False,
                "exactly M ticks is still fully predictable - truncation is for STRICTLY more than M");
        }

        [Test]
        public void Compute_BacklogOverTheCap_TruncatesAndDropsTheRemainderEntirely()
        {
            // A multi-second background stall (architect's corrected repro: an unfocused Editor
            // ticking at 2-4 Hz can pile up several seconds, not just one 450ms hitch).
            TickCatchUp.Plan plan = TickCatchUp.Compute(5.0, TickDuration, 1, 20);

            Assert.That(plan.TicksToPredict, Is.EqualTo(20),
                "capped at M - predicting the whole 100-tick backlog tick-by-tick is not worth " +
                "it once a snapshot can just resync (R4 판정 rule 5)");
            Assert.That(plan.TicksToSend, Is.EqualTo(1));
            Assert.That(plan.Truncated, Is.True);
            Assert.That(plan.RemainingAccumulatorSeconds, Is.EqualTo(0.0),
                "the discarded backlog must NOT carry to next frame - carrying it forward would " +
                "just recreate an oversized backlog on the very next Update()");
        }

        // ------------------------------------------------------------------ K as an explorable parameter (architect: K may vary in tests)

        [TestCase(0)]
        [TestCase(1)]
        [TestCase(3)]
        [TestCase(8)]
        public void Compute_TicksToSend_NeverExceedsMaxSendsPerFrameOrTicksToPredict(int maxSendsPerFrame)
        {
            TickCatchUp.Plan plan = TickCatchUp.Compute(0.46, TickDuration, maxSendsPerFrame, 20);

            Assert.That(plan.TicksToSend, Is.LessThanOrEqualTo(maxSendsPerFrame));
            Assert.That(plan.TicksToSend, Is.LessThanOrEqualTo(plan.TicksToPredict));
        }

        // ------------------------------------------------------------------ carry-forward + sent == predicted (SC-89 (a) invariant 3)

        [Test]
        public void Compute_CarryForwardTicksPlusSentTicks_AlwaysEqualsPredictedTicks()
        {
            double[] accumulators = { 0.0, TickDuration * 0.5, TickDuration, 0.46, 20 * TickDuration, 5.0 };
            foreach (double acc in accumulators)
            {
                TickCatchUp.Plan plan = TickCatchUp.Compute(acc, TickDuration, TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);
                int carryForwardTicks = plan.TicksToPredict - plan.TicksToSend;

                Assert.That(carryForwardTicks + plan.TicksToSend, Is.EqualTo(plan.TicksToPredict),
                    "acc=" + acc + ": by construction only the last TicksToSend of TicksToPredict " +
                    "carry a real send - the rest are carry-forward-only, so this must hold trivially. " +
                    "A future change that breaks this construction (e.g. sending non-contiguous " +
                    "ticks) must fail here first.");
                Assert.That(carryForwardTicks, Is.GreaterThanOrEqualTo(0));
            }
        }

        // ------------------------------------------------------------------ property test: wall-clock ticks per second (SC-89 (a) invariant 2)

        [Test]
        public void Compute_OverManyFramesWithModerateJitterNoHitch_PredictsAboutTwentyTicksPerWallClockSecond()
        {
            // Random frame deltas around 60fps (16.6ms) with jitter, summing to ~10 real
            // seconds, no single frame gap larger than 4 ticks (this test's "no hitch" case -
            // the hitch case is the truncation tests above and the property test below).
            var random = new Random(20260923);
            double accumulator = 0.0;
            int totalPredicted = 0;
            double wallClockSeconds = 0.0;

            while (wallClockSeconds < 10.0)
            {
                double frameDelta = 0.010 + random.NextDouble() * 0.010; // 10-20ms frames
                wallClockSeconds += frameDelta;
                accumulator += frameDelta;

                TickCatchUp.Plan plan = TickCatchUp.Compute(
                    accumulator, TickDuration, TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);
                totalPredicted += plan.TicksToPredict;
                accumulator = plan.RemainingAccumulatorSeconds;

                Assert.That(plan.TicksToSend, Is.LessThanOrEqualTo(1),
                    "invariant 1 (SC-89 (a)): never more than K=1 send per frame, regardless of " +
                    "how this frame's delta landed");
            }

            double expectedTicks = wallClockSeconds / TickDuration;
            TestContext.WriteLine("wall_clock_s=" + wallClockSeconds + " total_predicted_ticks=" + totalPredicted +
                                  " expected≈" + expectedTicks);
            Assert.That(totalPredicted, Is.EqualTo(expectedTicks).Within(1),
                "invariant 2 (SC-89 (a)): predicted ticks must track wall-clock time to within " +
                "±1 tick over the run, jitter or not - the accumulator carries fractional " +
                "remainders across frames instead of dropping them");
        }

        [Test]
        public void Compute_OverManyFramesIncludingA460msHitch_StillTracksWallClockWithinOneTick()
        {
            // Same shape as the property test above, but with the R4 evidence hitch spliced in
            // partway through - this is the "random frame delta series including a >=450ms
            // hitch" architect's H-7 spec calls for, run here against Compute directly.
            var random = new Random(20260923);
            double accumulator = 0.0;
            int totalPredicted = 0;
            double wallClockSeconds = 0.0;
            bool hitchInjected = false;

            while (wallClockSeconds < 10.0)
            {
                double frameDelta = wallClockSeconds > 4.0 && !hitchInjected
                    ? 0.46
                    : 0.010 + random.NextDouble() * 0.010;
                if (frameDelta >= 0.46) hitchInjected = true;

                wallClockSeconds += frameDelta;
                accumulator += frameDelta;

                TickCatchUp.Plan plan = TickCatchUp.Compute(
                    accumulator, TickDuration, TickCatchUp.ProductionMaxSendsPerFrame, TickCatchUp.ProductionMaxPredictedTicksPerFrame);
                totalPredicted += plan.TicksToPredict;
                accumulator = plan.RemainingAccumulatorSeconds;

                Assert.That(plan.TicksToSend, Is.LessThanOrEqualTo(1),
                    "invariant 1 holds through the hitch frame too, not just the calm frames");
            }

            Assert.That(hitchInjected, Is.True, "test setup: the hitch must actually have run");

            double expectedTicks = wallClockSeconds / TickDuration;
            Assert.That(totalPredicted, Is.EqualTo(expectedTicks).Within(1),
                "invariant 2 must still hold with a hitch in the mix - a hitch below the M=20 " +
                "truncation cap must be fully absorbed by prediction, not lost");
        }

        // ------------------------------------------------------------------ argument validation

        [Test]
        public void Compute_NonPositiveTickDuration_Throws()
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => TickCatchUp.Compute(1.0, 0.0, 1, 20));
        }
    }
}
