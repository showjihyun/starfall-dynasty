// SC-56(b) instrumentation gap (qa2, block 6 pre-flight, 2026-09-22): p50/p99/max must be real
// numbers traceable to actual Reconcile() samples, and N=0 must never read as "measured zero"
// (sprint contract §7a). Pure C#, no transport, no scene - Starfall.Greybox's ReconcileErrorStats
// is the only new production code this covers; GreyboxSession's wiring of it is verified by
// compiling clean here and by a real block-6 run (qa2), the same split R1/R2 used for CL-2.

using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReconcileErrorStatsTests
    {
        [Test]
        public void Compute_NoSamples_IsTheEmptySentinel()
        {
            PercentileStats stats = ReconcileErrorStats.Compute(new List<double>());

            Assert.That(stats.N, Is.EqualTo(0));
            Assert.That(double.IsNaN(stats.P50), Is.True, "N=0 must not read as p50=0");
            Assert.That(double.IsNaN(stats.P99), Is.True, "N=0 must not read as p99=0");
            Assert.That(double.IsNaN(stats.Max), Is.True, "N=0 must not read as max=0");
        }

        [Test]
        public void Compute_NullSamples_IsAlsoTheEmptySentinel()
        {
            PercentileStats stats = ReconcileErrorStats.Compute(null);
            Assert.That(stats.N, Is.EqualTo(0));
        }

        [Test]
        public void Compute_SingleSample_AllThreeStatsEqualIt()
        {
            PercentileStats stats = ReconcileErrorStats.Compute(new List<double> { 3.5 });

            Assert.That(stats.N, Is.EqualTo(1));
            Assert.That(stats.P50, Is.EqualTo(3.5));
            Assert.That(stats.P99, Is.EqualTo(3.5));
            Assert.That(stats.Max, Is.EqualTo(3.5));
        }

        [Test]
        public void Compute_TenAscendingSamples_MatchesNearestRankByHand()
        {
            // 1..10, rank(q) = ceil(q*10): p50 -> rank 5 -> value 5. p99 -> rank 10 -> value 10,
            // same as max - nearest-rank on a small N pushes p99 to the top sample. This mirrors
            // tools/bots/src/stats.rs::nearest_rank's own behaviour, not a bug in this port.
            var samples = new List<double> { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 };
            PercentileStats stats = ReconcileErrorStats.Compute(samples);

            Assert.That(stats.N, Is.EqualTo(10));
            Assert.That(stats.P50, Is.EqualTo(5));
            Assert.That(stats.P99, Is.EqualTo(10));
            Assert.That(stats.Max, Is.EqualTo(10));
        }

        [Test]
        public void Compute_OneHundredAscendingSamples_P99IsThe99th()
        {
            var samples = new List<double>();
            for (int i = 1; i <= 100; i++) samples.Add(i);

            PercentileStats stats = ReconcileErrorStats.Compute(samples);

            Assert.That(stats.N, Is.EqualTo(100));
            Assert.That(stats.P50, Is.EqualTo(50));
            Assert.That(stats.P99, Is.EqualTo(99));
            Assert.That(stats.Max, Is.EqualTo(100));
        }

        [Test]
        public void Compute_UnsortedInput_SortsBeforeRanking()
        {
            // sorted: 1,3,5,7,9 -> p50 rank=ceil(0.5*5)=3 -> index 2 -> value 5.
            var samples = new List<double> { 9, 1, 5, 3, 7 };
            PercentileStats stats = ReconcileErrorStats.Compute(samples);

            Assert.That(stats.Max, Is.EqualTo(9));
            Assert.That(stats.P50, Is.EqualTo(5));
        }

        [Test]
        public void Compute_DoesNotMutateTheCallersList()
        {
            // GreyboxSession keeps appending to this same list across the session - Compute()
            // runs on every HasError reconcile, so it must never reorder the caller's copy.
            var samples = new List<double> { 5, 1, 3 };
            ReconcileErrorStats.Compute(samples);

            Assert.That(samples, Is.EqualTo(new List<double> { 5, 1, 3 }));
        }
    }
}
