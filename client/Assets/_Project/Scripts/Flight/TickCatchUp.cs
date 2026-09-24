// R4 판정 (_workspace/p1-01-ship-movement/01_architect_decisions.md, "R4 판정 - 히치 catch-up
// 버스트" + "R4 보충 판정") / ADR-0012 section 6.1-6.3 (architect revision). This is H-1 of CL-H.
//
// Root cause this replaces (03_client_impl.md R6): GreyboxSession.Update()'s catch-up loop had
// no per-frame cap, so a single hitch (Editor GC pause, or - per the corrected repro, a
// backgrounded Editor tick-ing at 2-4 Hz - many frames in a row) drained and SENT every
// backlogged tick in one synchronous burst, tripping the server's per-tick command cap
// (ADR-0011 section 5.2, MAX_COMMANDS_PER_SESSION_PER_TICK=8).
//
// The fix separates two questions SendAndPredictOneTick() used to answer with one number:
//   - how many ticks must LOCAL PREDICTION advance by, to stay in step with the server's own
//     wall-clock simulation (the server keeps advancing every tick, carrying the last applied
//     input forward up to carry_forward_max_ticks - ADR-0011 section 6.1)?
//   - how many of those ticks should actually go over the WIRE?
//
// K (maxSendsPerFrame) = 1 in production is NOT a tuning value - it is derived: commands sent
// in the same client frame arrive at (as good as) the same server tick, and the server applies
// only the LAST one it receives per tick (ADR-0011 section 4, last-write-wins within a tick).
// Every command beyond the last is guaranteed to be discarded server-side - sending it spends
// queue depth and moves the session toward the protocol-violation budget for zero effect on
// what the server ends up doing. K=1 is the only value with zero waste, not a compromise.
//
// M (maxPredictedTicksPerFrame) = 20 (1 second) in production bounds how far local prediction
// will single-handedly try to catch up after a hitch. Past M, the backlog is discarded and the
// next WORLD_SNAPSHOT is trusted outright (R4 판정 rule 5) - a hitch that large means the local
// simulation was starved long enough that replaying it tick-by-tick is not worth the cost, and
// reconciliation against the server's actual state is strictly more correct anyway.
//
// Deliberately NOT here: a boolean that switches between "predict beyond the send cap" and
// "don't" (R4 보충 판정: "정책은 하나다 - 밀린 tick은 언제나 전부 예측한다"). A policy-selecting
// parameter is exactly the shape this slice has been burned by four times already (world_full,
// SET_SHIP_CONTROL rejection, the bot pairing gate, the wall-clock flaky test) plus this file's
// own earlier draft (predictBeyondSendCap) as a fifth - a flag a future change can flip back to
// the wrong value while every test that only ever exercised the correct branch stays green.
// There is exactly one policy. If a caller needs to exercise the "don't predict beyond the cap"
// shape for comparison, it does so in the test file, not by calling in here with a different flag.

using System;

namespace Starfall.Flight
{
    public static class TickCatchUp
    {
        /// <summary>K in production (ADR-0012 section 6.2 / R4 판정): commands sent in the same
        /// frame land in the same server tick and the server keeps only the last one it
        /// receives (ADR-0011 section 4) - sending more than 1 per frame is guaranteed waste,
        /// never a reliability margin.</summary>
        public const int ProductionMaxSendsPerFrame = 1;

        /// <summary>M in production (ADR-0012 section 6.2 / R4 판정, rule 5): 1 second at
        /// tick_hz=20. Past this many backlogged ticks in one frame, local prediction stops
        /// trying to catch up tick-by-tick and defers to the next WORLD_SNAPSHOT instead.</summary>
        public const int ProductionMaxPredictedTicksPerFrame = 20;

        /// <summary>carry_forward_max_ticks (ADR-0011 section 6.1): how many consecutive
        /// UNSENT predicted ticks may reuse the last actually-sent input before the client
        /// switches to the dormant input the server itself falls back to (R4 판정 rule 5).
        /// 10 ticks = 500ms at tick_hz=20 - the same value RebaseHold.ForcedRebaseAfterSeconds
        /// (in seconds) is derived from.</summary>
        public const int ProductionCarryForwardMaxTicks = 10;

        /// <summary>What one frame should do with its accumulated tick backlog.</summary>
        public readonly struct Plan
        {
            /// <summary>How many ticks local prediction should advance by this frame (R4 판정
            /// rule 1: every backlogged tick, up to <see cref="ProductionMaxPredictedTicksPerFrame"/>
            /// - never fewer just because sends are capped).</summary>
            public readonly int TicksToPredict;

            /// <summary>How many of the LAST <see cref="TicksToPredict"/> ticks should carry an
            /// actual network send (R4 판정 rule 2: only the most recent input is worth sending -
            /// the caller sends real, current input for these; the earlier
            /// <c>TicksToPredict - TicksToSend</c> ticks are carry-forward/dormant-input
            /// prediction only, per rules 3-4, not this type's concern).</summary>
            public readonly int TicksToSend;

            /// <summary>Accumulator seconds left over for the next frame. 0 whenever
            /// <see cref="Truncated"/> is true (R4 판정 rule 5: the discarded backlog is not
            /// carried forward - the next snapshot is trusted outright instead).</summary>
            public readonly double RemainingAccumulatorSeconds;

            /// <summary>True when the backlog exceeded the frame's predicted-tick cap and the
            /// excess was discarded rather than predicted (R4 판정 rule 5). The caller counts
            /// this (H-8/CL-H: catchup_truncated_total) - it is evidence a hitch was large
            /// enough that reconciliation, not replay, resolved it.</summary>
            public readonly bool Truncated;

            public Plan(int ticksToPredict, int ticksToSend, double remainingAccumulatorSeconds, bool truncated)
            {
                TicksToPredict = ticksToPredict;
                TicksToSend = ticksToSend;
                RemainingAccumulatorSeconds = remainingAccumulatorSeconds;
                Truncated = truncated;
            }
        }

        /// <summary>Pure. No UnityEngine reference, no wall clock read here - the caller
        /// (GreyboxSession.Update()) is the only place that reads Time.unscaledDeltaTime and
        /// passes the result in, so this function is callable byte-for-byte from both
        /// production code and an EditMode test (the exact gap R7/qa3/architect flagged in
        /// GreyboxSendBurstTests.cs's first draft, which reimplemented this loop's shape
        /// instead of calling it).</summary>
        /// <param name="accumulatorSecondsBeforeThisFrame">The tick accumulator's value after
        /// adding this frame's elapsed real time, before any ticks are drained from it.</param>
        /// <param name="tickDurationSeconds">1 / tick_hz.</param>
        /// <param name="maxSendsPerFrame">K. Production callers pass
        /// <see cref="ProductionMaxSendsPerFrame"/>; tests may pass other values to explore the
        /// design space, but production code paths must not.</param>
        /// <param name="maxPredictedTicksPerFrame">M. Production callers pass
        /// <see cref="ProductionMaxPredictedTicksPerFrame"/>.</param>
        public static Plan Compute(
            double accumulatorSecondsBeforeThisFrame,
            double tickDurationSeconds,
            int maxSendsPerFrame,
            int maxPredictedTicksPerFrame)
        {
            if (tickDurationSeconds <= 0.0)
                throw new ArgumentOutOfRangeException(nameof(tickDurationSeconds), "tick duration must be positive");
            if (maxSendsPerFrame < 0)
                throw new ArgumentOutOfRangeException(nameof(maxSendsPerFrame), "must not be negative");
            if (maxPredictedTicksPerFrame < 0)
                throw new ArgumentOutOfRangeException(nameof(maxPredictedTicksPerFrame), "must not be negative");

            if (accumulatorSecondsBeforeThisFrame < tickDurationSeconds)
            {
                // No whole tick has accumulated yet - nothing to predict or send, and nothing
                // to discard. This is the common case at 20 Hz with no hitch.
                return new Plan(0, 0, accumulatorSecondsBeforeThisFrame, truncated: false);
            }

            int backlogTicks = (int)Math.Floor(accumulatorSecondsBeforeThisFrame / tickDurationSeconds);

            if (backlogTicks > maxPredictedTicksPerFrame)
            {
                // R4 판정 rule 5: the excess is discarded outright, not carried to next frame -
                // carrying it forward would just recreate the same oversized backlog next frame.
                int ticksToPredict = maxPredictedTicksPerFrame;
                int ticksToSend = Math.Min(ticksToPredict, maxSendsPerFrame);
                return new Plan(ticksToPredict, ticksToSend, 0.0, truncated: true);
            }
            else
            {
                double remaining = accumulatorSecondsBeforeThisFrame - backlogTicks * tickDurationSeconds;
                int ticksToSend = Math.Min(backlogTicks, maxSendsPerFrame);
                return new Plan(backlogTicks, ticksToSend, remaining, truncated: false);
            }
        }
    }
}
