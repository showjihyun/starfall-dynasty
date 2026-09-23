// Nearest-rank percentiles for SC-56(b): "재조정 직전 위치 오차 p50/p99/최대를 HUD·로그에".
//
// Matches tools/bots/src/stats.rs::nearest_rank exactly - same formula, same reason: nearest-
// rank means p(q) is always a value that was actually observed, so a QA number can always be
// traced back to a real Reconcile() call. An interpolated percentile could not be.
//
// Pure C#, no UnityEngine reference - testable without a scene or Play mode, same pattern as
// ObserverCsv.cs in this folder.

using System;
using System.Collections.Generic;

namespace Starfall.Greybox
{
    /// <summary>
    /// p50/p99/max over a sample set, plus how many samples went in.
    /// <para>
    /// Sprint contract §7a: a distribution over zero samples asserts nothing about the condition
    /// it targets. <see cref="Empty"/> is the N=0 case, kept visibly distinct (NaN, not 0) from
    /// "measured, and it happened to be zero" - the exact trap SC-56 itself warns about for
    /// <c>HasError=false</c> reading as an error of 0.
    /// </para>
    /// </summary>
    public readonly struct PercentileStats
    {
        public readonly double P50;
        public readonly double P99;
        public readonly double Max;
        public readonly int N;

        public PercentileStats(double p50, double p99, double max, int n)
        {
            P50 = p50;
            P99 = p99;
            Max = max;
            N = n;
        }

        public static readonly PercentileStats Empty = new PercentileStats(double.NaN, double.NaN, double.NaN, 0);
    }

    public static class ReconcileErrorStats
    {
        /// <summary>
        /// Copies and sorts <paramref name="samples"/> (the caller's list is never mutated - it
        /// is still being appended to across the session) and returns nearest-rank p50/p99/max.
        /// <see cref="PercentileStats.Empty"/> when there are no samples yet.
        /// </summary>
        public static PercentileStats Compute(IReadOnlyList<double> samples)
        {
            if (samples == null || samples.Count == 0) return PercentileStats.Empty;

            var sorted = new List<double>(samples);
            sorted.Sort();
            int n = sorted.Count;

            return new PercentileStats(
                NearestRank(sorted, 0.50),
                NearestRank(sorted, 0.99),
                sorted[n - 1],
                n);
        }

        /// <summary>tools/bots/src/stats.rs::nearest_rank, ported term for term: rank =
        /// ceil(q * n), clamped to [1, n], 1-based into the ascending-sorted samples.</summary>
        static double NearestRank(List<double> sortedAscending, double q)
        {
            int n = sortedAscending.Count;
            int rank = (int)Math.Ceiling(q * n);
            if (rank < 1) rank = 1;
            if (rank > n) rank = n;
            return sortedAscending[rank - 1];
        }
    }
}
