// Reconnect backoff. Pure functions only: no clock, no Unity API, no sockets.
// Rules: docs/adr/0005-realtime-transport-and-message-framing.md section 5.

using System;

namespace Starfall.Net
{
    /// <summary>
    /// Exponential backoff with full jitter, as a pure function of the attempt number and a
    /// caller-supplied random source.
    /// <para>
    /// Full jitter rather than a fixed schedule because this slice creates 30 clients that
    /// lose their connection at the same instant; a fixed schedule would reconnect them all
    /// in the same millisecond, forever.
    /// </para>
    /// </summary>
    public static class ReconnectPolicy
    {
        /// <summary>First ceiling, in milliseconds.</summary>
        public const int BaseDelayMs = 500;

        /// <summary>Ceiling multiplier per attempt.</summary>
        public const int Factor = 2;

        /// <summary>Longest a client ever waits, in milliseconds.</summary>
        public const int CapMs = 10_000;

        /// <summary>
        /// Largest exponent that can still change the ceiling: 500 ms * 2^5 = 16 s, already
        /// past the 10 s cap.
        /// <para>
        /// Clamping is not an optimization, it is a correctness fix. C# masks a shift count by
        /// 63, so <c>500L &lt;&lt; 62</c> is <b>0</b> and <c>500L &lt;&lt; 64</c> is back to
        /// 500 (measured). Without the clamp a client that has been retrying for ~10 minutes
        /// against a dead server starts reconnecting with zero delay - exactly the
        /// reconnect storm full jitter exists to prevent.
        /// </para>
        /// </summary>
        public const int MaxExponent = 5;

        /// <summary>The ceiling for an attempt: <c>min(cap, base * 2^min(n, 5))</c>.</summary>
        /// <param name="attempt">0 for the first retry after a lost connection.</param>
        public static int CeilingMs(int attempt)
        {
            if (attempt < 0) throw new ArgumentOutOfRangeException(nameof(attempt), attempt, "attempt must be >= 0");

            int exponent = attempt < MaxExponent ? attempt : MaxExponent;

            // Factor^exponent by repeated multiplication rather than a shift: the shift is
            // what breaks (see MaxExponent), and with the exponent clamped to 5 this loop
            // runs at most five times.
            long ceiling = BaseDelayMs;
            for (int i = 0; i < exponent; i++) ceiling *= Factor;

            return ceiling < CapMs ? (int)ceiling : CapMs;
        }

        /// <summary>
        /// The delay to wait before attempt <paramref name="attempt"/>:
        /// <c>random(1, ceiling)</c> milliseconds.
        /// <para>
        /// The lower bound is 1 ms rather than 0 so that "the delay is never zero" is a
        /// property of the function instead of a probabilistic outcome; a zero-length wait is
        /// indistinguishable from a missing backoff when reading a log.
        /// </para>
        /// </summary>
        /// <param name="attempt">0 for the first retry after a lost connection.</param>
        /// <param name="random">Injected so the test is deterministic with a fixed seed.
        /// Never <c>UnityEngine.Random</c>: it is a main-thread global and this runs on the
        /// reconnect task.</param>
        public static int DelayMs(int attempt, Random random)
        {
            if (random == null) throw new ArgumentNullException(nameof(random));

            int ceiling = CeilingMs(attempt);
            return random.Next(1, ceiling + 1);
        }
    }
}
