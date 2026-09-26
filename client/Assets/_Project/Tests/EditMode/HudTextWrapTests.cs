// F-1 (05_qa_report_r5.md §6.1, round R5): GreyboxSession.OnGUI() used to draw HUD lines in a
// single fixed-width, fixed-height Rect (400px/18px) per line, with no wrap and no ellipsis -
// overflowing text was simply never drawn. Three counters that mattered to SC-56/SC-89 sat past
// that cutoff and never appeared in any recording. These tests pin the fix at two levels: the
// pure wrap algorithm (HudTextWrap.Wrap) and the production row-building step GreyboxSession's
// OnGUI actually calls (BuildHudRows), using the exact composite lines from GreyboxSession.cs
// that qa found silently cut.

using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class HudTextWrapTests
    {
        [Test]
        public void Wrap_ShortLine_ReturnsSingleRowUnchanged()
        {
            List<string> rows = HudTextWrap.Wrap("tick=511524", 110);

            Assert.That(rows.Count, Is.EqualTo(1));
            Assert.That(rows[0], Is.EqualTo("tick=511524"));
        }

        [Test]
        public void Wrap_EmptyOrNull_ReturnsOneEmptyRow_NeverThrows()
        {
            Assert.That(HudTextWrap.Wrap("", 110), Is.EqualTo(new List<string> { "" }));
            Assert.That(HudTextWrap.Wrap(null, 110), Is.EqualTo(new List<string> { "" }));
        }

        [Test]
        public void Wrap_LineLongerThanLimit_SplitsIntoMultipleRowsEachWithinLimit()
        {
            // The exact SC-56(d)②③ composite line (qa r5 §6.1), with worst-case digit counts so
            // this test does not depend on today's small counter values staying small.
            string line =
                "cl2_reconcile_has_error_total=2216 " +
                "cl2_reconcile_replayed_nonzero_total=123456789 " +
                "cl2_reconcile_both_omega_nonzero_total=123456789";

            List<string> rows = HudTextWrap.Wrap(line, GreyboxSession.HudMaxCharsPerLine);

            Assert.That(rows.Count, Is.GreaterThan(1),
                "this line is longer than HudMaxCharsPerLine and must wrap, not fit on one row");
            foreach (string row in rows)
                Assert.That(row.Length, Is.LessThanOrEqualTo(GreyboxSession.HudMaxCharsPerLine));
        }

        [Test]
        public void Wrap_LineLongerThanLimit_LosesNoField()
        {
            // §7a: the condition this guards against (silent drop past a fixed Rect width) is
            // exercised by asserting every field survives in the wrapped output, not merely that
            // wrapping happened.
            string line =
                "cl2_reconcile_has_error_total=2216 " +
                "cl2_reconcile_replayed_nonzero_total=123456789 " +
                "cl2_reconcile_both_omega_nonzero_total=123456789";

            string rejoined = string.Join(" ", HudTextWrap.Wrap(line, GreyboxSession.HudMaxCharsPerLine));

            Assert.That(rejoined, Is.EqualTo(line));
        }

        [Test]
        public void Wrap_SingleTokenLongerThanLimit_KeptWholeOnItsOwnRow_NotDropped()
        {
            string token = new string('x', 250);

            List<string> rows = HudTextWrap.Wrap(token, 110);

            Assert.That(rows.Count, Is.EqualTo(1));
            Assert.That(rows[0], Is.EqualTo(token), "an over-long token must still appear in full, never truncated");
        }

        // ------------------------------------------------------------------ production entry point (GreyboxSession.BuildHudRows)

        [Test]
        public void BuildHudRows_Sc56AndSc89Lines_AllFieldsAppearSomewhereInOutput()
        {
            // The three lines qa r5 §6.1 found silently cut, reconstructed exactly as
            // GreyboxSession.OnGUI() builds them (GreyboxSession.cs, the `lines` list).
            var lines = new List<string>
            {
                "cl2_reconcile_has_error_total=2216 cl2_reconcile_replayed_nonzero_total=99999999 cl2_reconcile_both_omega_nonzero_total=99999999",
                "send_burst_max_ticks_per_update=20 send_burst_max_sends_per_frame=99999999 send_burst_max_sends_per_trailing_1s=99999999",
                "catchup_carry_forward_ticks_total=90 catchup_dormant_ticks_total=99999999 catchup_truncated_total=99999999 reconcile_forced_after_hitch_total=99999999",
            };

            List<string> rows = GreyboxSession.BuildHudRows(lines);
            string joined = string.Join("\n", rows);

            Assert.That(joined, Does.Contain("cl2_reconcile_replayed_nonzero_total=99999999"), "SC-56 (d)② must be on screen");
            Assert.That(joined, Does.Contain("cl2_reconcile_both_omega_nonzero_total=99999999"), "SC-56 (d)③ must be on screen");
            Assert.That(joined, Does.Contain("send_burst_max_sends_per_frame=99999999"), "SC-89 discriminating assertion (K-7) must be on screen");
            Assert.That(joined, Does.Contain("catchup_dormant_ticks_total=99999999"));
            Assert.That(joined, Does.Contain("catchup_truncated_total=99999999"));
            Assert.That(joined, Does.Contain("reconcile_forced_after_hitch_total=99999999"));

            // Every row individually must fit the panel's per-row width budget in characters -
            // this is what makes "fits" checkable without an OnGUI context (see HudTextWrap.cs
            // header for why pixel-metric checks are deliberately avoided here).
            foreach (string row in rows)
                Assert.That(row.Length, Is.LessThanOrEqualTo(GreyboxSession.HudMaxCharsPerLine));
        }
    }
}
