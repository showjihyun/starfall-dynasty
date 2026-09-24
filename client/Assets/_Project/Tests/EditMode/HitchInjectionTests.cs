// F-23 (team-lead R18/R19 assignment). Tests for Starfall.Greybox.HitchInjection - the pure half
// (Format(), DefaultStallMs). The keyboard-edge-detection/Thread.Sleep half
// (GreyboxSession.MaybeInjectHitch()) is device- and MonoBehaviour-bound and untestable in
// EditMode without a live scene, the same limitation ShipInputSampler.Sample() already has (see
// that file's header) - this file covers what CAN be covered without one.

using System.IO;
using NUnit.Framework;
using Starfall.Greybox;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class HitchInjectionTests
    {
        // F-31 (architect R10 판정 §3.2 item 2, team-lead R21). The two bounds this test checks
        // DefaultStallMs against are DERIVED from sync-tuning.json, not hand-copied literals -
        // a hardcoded (100, 500) would stay green even after a designer lowered
        // carry_forward_max_ticks to 8, silently making the injection window false while the
        // test kept passing (exactly the "green tells you nothing" shape CLAUDE.md warns about).
        static SyncTuningData LoadTuning()
        {
            string repoRoot = ContractFixtures.RequireRepoRoot();
            string path = Path.Combine(repoRoot, "data", "movement", "sync-tuning.json");
            return SyncTuningData.FromJson(File.ReadAllText(path));
        }

        [Test]
        public void DerivedBounds_AreBothNonZero()
        {
            // §7a paired assertion: if parsing sync-tuning.json silently died (wrong section
            // name, moved file) both bounds would read 0 and the positive test below would pass
            // vacuously (0 < 400 < 0 is false, so it would actually FAIL loudly - but a version
            // of this window derived the OTHER way, e.g. both bounds collapsing to the same
            // value, could pass silently). Asserting both are distinctly positive first makes a
            // dead parse fail HERE, not read as "the window shrank".
            SyncTuningData tuning = LoadTuning();
            double lowerBoundMs = tuning.SnapshotHz > 0 ? (1000.0 / tuning.SnapshotHz) * 2.0 : 0.0;
            double upperBoundMs = tuning.CarryForwardMaxTicks * (1000.0 / tuning.ClientSendHz);

            Assert.That(lowerBoundMs, Is.GreaterThan(0.0), "sync-tuning.json parse likely dead - snapshot_hz missing/zero");
            Assert.That(upperBoundMs, Is.GreaterThan(0.0), "sync-tuning.json parse likely dead - carry_forward_max_ticks/client_send_hz missing/zero");
        }

        [Test]
        public void DefaultStallMs_IsInsideTheVisibleDriftWindow()
        {
            // HitchInjection.cs's header derivation: a hitch shorter than one snapshot interval
            // resolves BETWEEN two snapshots (lower bound = 2 snapshot intervals, comfortably
            // past the 1-snapshot floor); a hitch at or past carry_forward_max_ticks switches
            // both sides to dormant input (upper bound). tick_ms comes from client_send_hz (=
            // tick_hz, ADR-0012 section 6) - the same clock InputRecord.ServerTick advances on.
            SyncTuningData tuning = LoadTuning();
            double tickMs = 1000.0 / tuning.ClientSendHz;
            double lowerBoundMs = (1000.0 / tuning.SnapshotHz) * 2.0;
            double upperBoundMs = tuning.CarryForwardMaxTicks * tickMs;

            Assert.That(HitchInjection.DefaultStallMs, Is.GreaterThan(lowerBoundMs),
                "must clear at least one full snapshot interval, or the drift resolves between snapshots and is never seen");
            Assert.That(HitchInjection.DefaultStallMs, Is.LessThan(upperBoundMs),
                "must stay under carry_forward_max_ticks worth of time, or the server switches to dormant input before the client sees drift at speed");
        }

        [Test]
        public void Format_ContainsAllThreeFields_WithTheirOwnDistinctValues()
        {
            string line = HitchInjection.Format(stallMs: 250, tick: 607724, speedMps: 140.0);

            Assert.That(line, Does.Contain("hitch_injected"));
            Assert.That(line, Does.Contain("stall_ms=250"));
            Assert.That(line, Does.Contain("tick=607724"));
            Assert.That(line, Does.Contain("speed_mps=140.0"));
        }

        [Test]
        public void Format_IsOneLine_GrepFriendly()
        {
            string line = HitchInjection.Format(1, 2, 3.0);
            Assert.That(line, Does.Not.Contain("\n"));
            Assert.That(line, Does.Not.Contain("\r"));
        }

        [Test]
        public void Format_DifferentStallAndTickValues_DoNotAlias()
        {
            // §7a-style field-swap guard: stall_ms and tick are both integers of similar
            // magnitude in a real session (250 vs. six-digit tick numbers, but not always) - a
            // formatter that swapped the two arguments would still "look plausible". Distinct,
            // easily-told-apart values catch that.
            string line = HitchInjection.Format(stallMs: 999, tick: 111, speedMps: 0.0);
            Assert.That(line, Does.Contain("stall_ms=999"));
            Assert.That(line, Does.Contain("tick=111"));
            Assert.That(line, Does.Not.Contain("stall_ms=111"));
            Assert.That(line, Does.Not.Contain("tick=999"));
        }
    }
}
