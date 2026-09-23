// Hand-written. Wire quantisation (ADR-0009 section 2). One function, used everywhere a
// physical quantity crosses to/from the wire, so there is exactly one place the rounding mode
// can be gotten wrong.
//
// q(x, scale, lo, hi) = clamp(round_half_away_from_zero(x * scale), lo, hi)
//
// Math.Round(double) defaults to banker's rounding (round-half-to-even) on this runtime -
// measured, not assumed (client, 2026-09-20, Unity-bundled Mono): 2.5 rounds to 2, not 3.
// Rust's f64::round() is round-half-away-from-zero. Omitting MidpointRounding.AwayFromZero
// here sends a DIFFERENT integer than the server on every exact .5 boundary.

using System;

namespace Starfall.Sim
{
    public static class Quantization
    {
        /// <summary>ADR-0009 section 2's q(x, scale, lo, hi). MidpointRounding.AwayFromZero is
        /// not optional - see the file header and ADR-0009 section 2's measured table.</summary>
        public static long Quantize(double x, double scale, long lo, long hi)
        {
            double rounded = Math.Round(x * scale, MidpointRounding.AwayFromZero);
            if (rounded < lo) return lo;
            if (rounded > hi) return hi;
            return (long)rounded;
        }

        /// <summary>x = i / scale. Both sides divide by the same constant, so both sides get
        /// the same bits (ADR-0009 section 2).</summary>
        public static double Dequantize(long i, double scale) => i / scale;

        // ---------------------------------------------------------------------------------
        // Scales and ranges (ADR-0009 section 2 table). Centralised so a client author reaches
        // for these instead of retyping "1000.0" at a call site.
        // ---------------------------------------------------------------------------------

        public const double PositionScale = 1000.0;             // m -> mm
        public const long PositionMin = -1_000_000_000_000L;
        public const long PositionMax = 1_000_000_000_000L;

        public const double VelocityScale = 1000.0;              // m/s -> mm/s
        public const long VelocityMin = -100_000_000L;
        public const long VelocityMax = 100_000_000L;

        public const double QuaternionScale = 1_000_000.0;       // unit -> micro
        public const long QuaternionMin = -1_000_000L;
        public const long QuaternionMax = 1_000_000L;

        public const double AngularVelocityScale = 1000.0;       // deg/s -> mdeg/s
        public const long AngularVelocityMin = -3_600_000L;
        public const long AngularVelocityMax = 3_600_000L;

        public const double ControlAxisScale = 1000.0;           // [-1,1] -> milli
        public const long ControlAxisMin = -1000L;
        public const long ControlAxisMax = 1000L;

        public static long QuantizePosition(double meters) => Quantize(meters, PositionScale, PositionMin, PositionMax);
        public static double DequantizePosition(long mm) => Dequantize(mm, PositionScale);

        public static long QuantizeVelocity(double metersPerSecond) => Quantize(metersPerSecond, VelocityScale, VelocityMin, VelocityMax);
        public static double DequantizeVelocity(long mmPerSecond) => Dequantize(mmPerSecond, VelocityScale);

        public static long QuantizeQuaternionComponent(double component) => Quantize(component, QuaternionScale, QuaternionMin, QuaternionMax);
        public static double DequantizeQuaternionComponent(long micro) => Dequantize(micro, QuaternionScale);

        public static long QuantizeAngularVelocity(double degreesPerSecond) => Quantize(degreesPerSecond, AngularVelocityScale, AngularVelocityMin, AngularVelocityMax);
        public static double DequantizeAngularVelocity(long mdegPerSecond) => Dequantize(mdegPerSecond, AngularVelocityScale);

        public static long QuantizeControlAxis(double normalized) => Quantize(normalized, ControlAxisScale, ControlAxisMin, ControlAxisMax);
        public static double DequantizeControlAxis(long milli) => Dequantize(milli, ControlAxisScale);
    }
}
