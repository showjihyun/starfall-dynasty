// Hand-written. Per-remote-ship snapshot buffer: fixed-delay interpolation, then a short
// capped extrapolation, then freeze (ADR-0012 section 5).
//
// Samples are keyed by the envelope tick (the server's clock, I-37/AC-7(a)), not by client
// arrival time - two clients that both received the same two snapshots compute the same
// bracket and the same t, which is what SC-63 checks across two observers.

using System.Collections.Generic;
using Starfall.Sim;

namespace Starfall.Remote
{
    public sealed class RemoteShipBuffer
    {
        public readonly struct Sample
        {
            public readonly long Tick;
            public readonly ShipSimState State;
            public readonly string Presence;

            public Sample(long tick, ShipSimState state, string presence)
            {
                Tick = tick;
                State = state;
                Presence = presence;
            }
        }

        public enum DisplayMode { NoData, Interpolated, Extrapolated, Frozen }

        public readonly struct Display
        {
            public readonly DisplayMode Mode;
            public readonly ShipSimState State;
            public readonly string Presence;

            public Display(DisplayMode mode, ShipSimState state, string presence)
            {
                Mode = mode;
                State = state;
                Presence = presence;
            }
        }

        /// <summary>Bounded so a ship that stops sending (frozen forever, never removed for
        /// some reason) cannot grow this list without limit. Comfortably larger than the 2
        /// samples any single interpolation needs.</summary>
        const int MaxRetainedSamples = 8;

        readonly List<Sample> _samples = new List<Sample>();

        public IReadOnlyList<Sample> Samples => _samples;

        /// <summary>Appends one WORLD_SNAPSHOT observation of this ship. Out-of-order or
        /// duplicate ticks are dropped - envelope tick only increases across a session
        /// (AC-7(a)), so anything else is a bug upstream, not a case to interpolate around.</summary>
        public void AddSample(long tick, ShipSimState state, string presence)
        {
            if (_samples.Count > 0 && tick <= _samples[_samples.Count - 1].Tick) return;

            _samples.Add(new Sample(tick, state, presence));
            if (_samples.Count > MaxRetainedSamples) _samples.RemoveAt(0);
        }

        /// <param name="renderTick">The tick-as-a-real-number to draw, normally
        /// (latest known server tick) - (remote_interp_delay_ms converted to ticks) -
        /// computed by the caller from its own clock, not by this class (this class has no
        /// clock - noEngineReferences, and purity for testing).</param>
        /// <param name="tickDurationSeconds">1 / tick_hz.</param>
        /// <param name="extrapolateMaxTicks">remote_extrapolate_max_ms converted to ticks.</param>
        public Display GetDisplay(double renderTick, double tickDurationSeconds, double extrapolateMaxTicks)
        {
            if (_samples.Count == 0) return new Display(DisplayMode.NoData, ShipSimState.Zero, null);

            if (_samples.Count == 1)
            {
                Sample only = _samples[0];
                double elapsed = renderTick - only.Tick;
                return elapsed <= 0.0
                    ? new Display(DisplayMode.Interpolated, only.State, only.Presence)
                    : ExtrapolateOrFreeze(only, elapsed, tickDurationSeconds, extrapolateMaxTicks);
            }

            int bracketStart = -1;
            for (int i = 0; i < _samples.Count; i++)
            {
                if (_samples[i].Tick > renderTick) break;
                bracketStart = i;
            }

            if (bracketStart < 0)
            {
                // renderTick is before the earliest retained sample (just started watching this
                // ship) - nothing to interpolate from yet, show the earliest sample as-is.
                return new Display(DisplayMode.Interpolated, _samples[0].State, _samples[0].Presence);
            }

            if (bracketStart == _samples.Count - 1)
            {
                Sample latest = _samples[bracketStart];
                double elapsed = renderTick - latest.Tick;
                return ExtrapolateOrFreeze(latest, elapsed, tickDurationSeconds, extrapolateMaxTicks);
            }

            Sample from = _samples[bracketStart];
            Sample to = _samples[bracketStart + 1];
            double span = to.Tick - from.Tick;
            double t = span > 0.0 ? (renderTick - from.Tick) / span : 0.0;
            if (t < 0.0) t = 0.0;
            if (t > 1.0) t = 1.0;

            ShipSimState interpolated = RemoteInterpolation.Interpolate(from.State, to.State, t);
            // The later sample's presence: crossing into a segment that ends in LINGERING should
            // show LINGERING for that segment, not wait until fully past it.
            return new Display(DisplayMode.Interpolated, interpolated, to.Presence);
        }

        static Display ExtrapolateOrFreeze(Sample latest, double elapsedTicks, double tickDurationSeconds, double extrapolateMaxTicks)
        {
            if (elapsedTicks <= 0.0) return new Display(DisplayMode.Interpolated, latest.State, latest.Presence);

            if (elapsedTicks > extrapolateMaxTicks)
            {
                double cappedSeconds = extrapolateMaxTicks * tickDurationSeconds;
                ShipSimState atCap = RemoteInterpolation.Extrapolate(latest.State, cappedSeconds);
                // Frozen means stopped, not "still coasting at the cap point's velocity"
                // (design section 6.4: "그 뒤에는 정지시킨다" - a ship that flew off and snapped
                // back is worse than one that visibly stops).
                var stopped = new ShipSimState(atCap.Position, Vec3d.Zero, atCap.Orientation, Vec3d.Zero, 0.0);
                return new Display(DisplayMode.Frozen, stopped, latest.Presence);
            }

            double seconds = elapsedTicks * tickDurationSeconds;
            return new Display(DisplayMode.Extrapolated, RemoteInterpolation.Extrapolate(latest.State, seconds), latest.Presence);
        }
    }
}
