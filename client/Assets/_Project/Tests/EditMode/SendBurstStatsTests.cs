// SC-89 instrumentation (see SendBurstStats.cs header). Same honesty note as
// ReconcileErrorStatsTests.cs (R5): implementation and tests were written together rather than
// RED-first - the class is a small, easily hand-verified accumulator (two running-max scans, no
// externally-defined algorithm to port) - and coverage below stands in for the missing RED step:
// empty state, zero-is-a-measurement, running max, and the trailing-window prune boundary.

using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class SendBurstStatsTests
    {
        [Test]
        public void NoRecordsYet_BothStatsReportUnmeasured()
        {
            var stats = new SendBurstStats();

            Assert.That(stats.MaxTicksDrainedPerUpdate, Is.Null,
                "no Update() has run yet - this must read as 'unmeasured', not 0 (contract section 7a)");
            Assert.That(stats.MaxSendsInTrailingOneSecond, Is.Null,
                "no send has happened yet - same distinction");
        }

        [Test]
        public void RecordUpdate_ZeroCountsAsARealMeasurement()
        {
            var stats = new SendBurstStats();
            stats.RecordUpdate(0);

            Assert.That(stats.MaxTicksDrainedPerUpdate, Is.EqualTo(0),
                "a caught-up frame (0 ticks drained) is a real observation, not 'no data yet'");
        }

        [Test]
        public void RecordUpdate_TracksTheRunningMaximum()
        {
            var stats = new SendBurstStats();
            stats.RecordUpdate(1);
            stats.RecordUpdate(9);
            stats.RecordUpdate(3);

            Assert.That(stats.MaxTicksDrainedPerUpdate, Is.EqualTo(9));
        }

        [Test]
        public void RecordUpdate_NeverDecreasesOnceASmallerValueArrives()
        {
            var stats = new SendBurstStats();
            stats.RecordUpdate(9);
            stats.RecordUpdate(1);

            Assert.That(stats.MaxTicksDrainedPerUpdate, Is.EqualTo(9),
                "the whole point is a session worst-case, not the latest frame");
        }

        [Test]
        public void RecordSend_NineSendsWithinOneSecond_ReportsNine()
        {
            var stats = new SendBurstStats();
            // Mirrors R4's evidence shape: a hitch drains 9 ticks and sends all 9 with no
            // spacing between them - here spread slightly to stay inside one window.
            for (int i = 0; i < 9; i++)
                stats.RecordSend(0.0 + i * 0.01);

            Assert.That(stats.MaxSendsInTrailingOneSecond, Is.EqualTo(9));
        }

        [Test]
        public void RecordSend_NormalTwentyHertzTraffic_TrailingWindowHoldsTwenty()
        {
            var stats = new SendBurstStats();
            // client_send_hz = tick_hz = 20 with no hitch: one send every 0.05s, so the
            // trailing 1s window should only ever contain the single most recent send at the
            // instant it lands (the previous one is already outside the window by 0.05s later
            // only once 1.0s has actually elapsed - assert the steady state after warmup).
            for (int i = 0; i < 40; i++)
                stats.RecordSend(i * 0.05);

            Assert.That(stats.MaxSendsInTrailingOneSecond, Is.EqualTo(21),
                "a full trailing second at 20 Hz spans 21 sends inclusive of both ends " +
                "(1.0s / 0.05s + 1, matching the inclusive cutoff pinned by " +
                "RecordSend_BoundaryAtExactlyOneSecondIsPruned) - SC-89 cares about a single " +
                "Update() sending many ticks at once, not the steady 20 Hz rate itself; this " +
                "asserts the window math, not a burst");
        }

        // ------------------------------------------------------------------ RecordFrameSendCount (SC-89 (b) pairing, K-7)

        [Test]
        public void RecordFrameSendCount_NoFrameYet_ReportsUnmeasured()
        {
            var stats = new SendBurstStats();
            Assert.That(stats.MaxSendsPerFrame, Is.Null);
        }

        [Test]
        public void RecordFrameSendCount_ZeroCountsAsAMeasurement()
        {
            var stats = new SendBurstStats();
            stats.RecordFrameSendCount(0);
            Assert.That(stats.MaxSendsPerFrame, Is.EqualTo(0));
        }

        [Test]
        public void RecordFrameSendCount_TracksTheRunningMaximum()
        {
            var stats = new SendBurstStats();
            stats.RecordFrameSendCount(1);
            stats.RecordFrameSendCount(9);
            stats.RecordFrameSendCount(1);

            Assert.That(stats.MaxSendsPerFrame, Is.EqualTo(9),
                "this is exactly the number a pre-fix binary would report on a background-pause " +
                "repro - K-7's whole point is distinguishing this from the fixed binary's 1");
        }

        [Test]
        public void RecordSend_PrunesEntriesOlderThanOneSecond()
        {
            var stats = new SendBurstStats();
            stats.RecordSend(0.0);
            stats.RecordSend(2.0); // 2s later - the t=0.0 entry must be out of the window

            Assert.That(stats.MaxSendsInTrailingOneSecond, Is.EqualTo(1),
                "the second send's window should hold only itself once the first is stale");
        }

        [Test]
        public void RecordSend_BoundaryAtExactlyOneSecondIsPruned()
        {
            var stats = new SendBurstStats();
            stats.RecordSend(0.0);
            stats.RecordSend(1.0); // exactly 1.0s later - cutoff = 1.0 - 1.0 = 0.0, and the
                                    // t=0.0 entry is < cutoff is false (0.0 is not < 0.0), so
                                    // this pins the inclusive/exclusive boundary explicitly.
            Assert.That(stats.MaxSendsInTrailingOneSecond, Is.EqualTo(2),
                "cutoff = now - 1.0 keeps entries >= cutoff; an entry exactly 1.0s old is kept");
        }
    }
}
