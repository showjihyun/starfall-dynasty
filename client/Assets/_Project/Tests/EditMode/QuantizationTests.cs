// C3. ADR-0009 section 2's rounding-mode table, executed on this runtime (Unity-bundled Mono)
// and asserted, not just quoted in a comment. Sprint contract SC-51/SC-52 depend on this being
// right: get the rounding wrong and every reconciliation test fails for a reason nobody can
// see from the assertion message alone.

using NUnit.Framework;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class QuantizationTests
    {
        // ADR-0009 section 2's measured table (client, 2026-09-20, Unity-bundled Mono):
        //   x       Rust f64::round()   Mono Math.Round(x)   Mono Math.Round(x, AwayFromZero)
        //   0.5     1                   0 (!)                1
        //   2.5     3                   2 (!)                3
        //  -2.5    -3                  -2 (!)                -3
        //  1234.5   1235                1234 (!)              1235
        [TestCase(0.5, 1)]
        [TestCase(2.5, 3)]
        [TestCase(-2.5, -3)]
        [TestCase(1234.5, 1235)]
        public void Quantize_RoundsHalfAwayFromZero_MatchingRust(double x, long expected)
        {
            // scale=1, lo/hi wide enough to never clamp - isolates the rounding behaviour.
            long actual = Quantization.Quantize(x, 1.0, long.MinValue, long.MaxValue);
            TestContext.WriteLine(x + " -> " + actual + " (expected " + expected + ")");
            Assert.That(actual, Is.EqualTo(expected),
                "Math.Round without MidpointRounding.AwayFromZero rounds half-to-even on this " +
                "runtime and disagrees with Rust f64::round() at every .5 boundary (ADR-0009 section 2).");
        }

        [Test]
        public void Quantize_ClampsToDeclaredRange()
        {
            Assert.That(Quantization.Quantize(1e20, 1.0, -100, 100), Is.EqualTo(100));
            Assert.That(Quantization.Quantize(-1e20, 1.0, -100, 100), Is.EqualTo(-100));
        }

        [Test]
        public void PositionRoundTrip_QuantizeThenDequantize_MatchesWithinHalfMillimetre()
        {
            const double meters = 1234.5678;
            long mm = Quantization.QuantizePosition(meters);
            double back = Quantization.DequantizePosition(mm);
            TestContext.WriteLine(meters + " m -> " + mm + " mm -> " + back + " m");
            Assert.That(back, Is.EqualTo(meters).Within(0.0005), "position quantisation loses at most 0.5 mm (ADR-0009 section 2)");
        }

        [Test]
        public void ControlAxis_QuantizeThenDequantize_RoundTripsExactlyAtWireResolution()
        {
            // The control axis wire resolution is 1/1000; a value already on that grid must
            // survive round-trip exactly (no accumulated error at the boundary the client
            // itself controls - I-36's "predict from your own sent integer" only works if this
            // holds).
            for (int milli = -1000; milli <= 1000; milli += 137)
            {
                double normalized = Quantization.DequantizeControlAxis(milli);
                long roundTripped = Quantization.QuantizeControlAxis(normalized);
                Assert.That(roundTripped, Is.EqualTo(milli), "milli=" + milli);
            }
        }
    }
}
