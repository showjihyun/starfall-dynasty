// C4/C5. SC-64/65's CSV contract (sprint contract section 3.1): tests/e2e/two_client_view.py
// (QA-owned) rejects any header it does not recognise as "NotImplementedYet" rather than
// tolerating a drift. This suite pins ObserverCsvRow's output against a literal copy of that
// script's COLUMNS constant, so a change to either side breaks a test instead of breaking QA's
// tool silently in the field.
//
// Only the pure C# row/writer types are exercised here (no live WebSocket session - that needs
// a real server, sprint contract section 6/E3/E8). ObserverSession's wiring is covered by
// reading, not by an EditMode test: it has no logic of its own beyond what
// PredictedShipController/RemoteShipBuffer/Reconciliation already test.

using System;
using System.IO;
using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ObserverCsvTests
    {
        /// <summary>Literal copy of tests/e2e/two_client_view.py's COLUMNS, joined the way that
        /// script's csv.DictReader compares fieldnames (order-sensitive). R23 (architect R10
        /// 후속 판정 §1/§4): two columns added at the end (render_offset_mm, render_offset_deg) -
        /// qa is making the identical addition to the .py side in the same round.</summary>
        const string QaExpectedHeader =
            "tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s,render_offset_mm,render_offset_deg";

        [Test]
        public void Header_MatchesQaScriptColumnsExactly()
        {
            Assert.That(ObserverCsvRow.Header, Is.EqualTo(QaExpectedHeader));
        }

        [Test]
        public void ToCsvLine_TwelveFieldsInContractOrder()
        {
            var shipId = Guid.Parse("01a0c000-0000-7000-8000-00000000000a");
            var observer = Guid.Parse("bbbb0000-0000-7000-8000-000000000002");
            var row = new ObserverCsvRow(
                tick: 128, observerActorId: observer, shipId: shipId, presence: "ACTIVE",
                positionXMm: 1_000, positionYMm: -2_000, positionZMm: 3_000,
                velocityXMmS: 4_000, velocityYMmS: -5_000, velocityZMmS: 6_000,
                renderOffsetMm: 315, renderOffsetDeg: 3.2981);

            string line = row.ToCsvLine();
            string[] fields = line.Split(',');

            Assert.That(fields.Length, Is.EqualTo(12));
            Assert.That(fields[0], Is.EqualTo("128"));
            Assert.That(fields[1], Is.EqualTo(observer.ToString()));
            Assert.That(fields[2], Is.EqualTo(shipId.ToString()));
            Assert.That(fields[3], Is.EqualTo("ACTIVE"));
            Assert.That(fields[4], Is.EqualTo("1000"));
            Assert.That(fields[5], Is.EqualTo("-2000"));
            Assert.That(fields[6], Is.EqualTo("3000"));
            Assert.That(fields[7], Is.EqualTo("4000"));
            Assert.That(fields[8], Is.EqualTo("-5000"));
            Assert.That(fields[9], Is.EqualTo("6000"));
            Assert.That(fields[10], Is.EqualTo("315"));
            Assert.That(fields[11], Is.EqualTo("3.2981"));
        }

        [Test]
        public void ToCsvLine_DefaultRenderOffset_IsZero_NotOmitted()
        {
            // R23: a row for another ship's presence (or the pre-F-33 call shape) must still
            // print two trailing fields, not drop them - a Format() that omitted zero-valued
            // trailing columns would break the fixed-column-count contract two_client_view.py
            // depends on.
            var row = new ObserverCsvRow(
                tick: 1, observerActorId: Guid.NewGuid(), shipId: Guid.NewGuid(), presence: "ACTIVE",
                positionXMm: 0, positionYMm: 0, positionZMm: 0,
                velocityXMmS: 0, velocityYMmS: 0, velocityZMmS: 0);

            string[] fields = row.ToCsvLine().Split(',');
            Assert.That(fields.Length, Is.EqualTo(12));
            Assert.That(fields[10], Is.EqualTo("0"));
            Assert.That(fields[11], Is.EqualTo("0.0000"));
        }

        [Test]
        public void Writer_WritesHeaderThenAppendedRows_ReadableAsPlainCsv()
        {
            string path = Path.Combine(Path.GetTempPath(), "starfall-observer-csv-" + Guid.NewGuid().ToString("N") + ".csv");
            try
            {
                var shipId = Guid.NewGuid();
                var observer = Guid.NewGuid();

                using (var writer = new ObserverCsvWriter(path))
                {
                    writer.Write(new ObserverCsvRow(2, observer, shipId, "ACTIVE", 1, 2, 3, 4, 5, 6));
                    writer.Write(new ObserverCsvRow(4, observer, shipId, "LINGERING", 7, 8, 9, 10, 11, 12));
                }

                string[] lines = File.ReadAllLines(path);
                Assert.That(lines.Length, Is.EqualTo(3), "header + 2 rows");
                Assert.That(lines[0], Is.EqualTo(ObserverCsvRow.Header));
                Assert.That(lines[1], Does.StartWith("2," + observer + "," + shipId + ",ACTIVE,1,2,3,4,5,6"));
                Assert.That(lines[2], Does.StartWith("4," + observer + "," + shipId + ",LINGERING,7,8,9,10,11,12"));
            }
            finally
            {
                if (File.Exists(path)) File.Delete(path);
            }
        }

        [Test]
        public void Writer_CreatesMissingParentDirectory()
        {
            string dir = Path.Combine(Path.GetTempPath(), "starfall-observer-csv-dir-" + Guid.NewGuid().ToString("N"));
            string path = Path.Combine(dir, "nested", "observer-a.csv");
            try
            {
                using (var writer = new ObserverCsvWriter(path))
                    writer.Write(new ObserverCsvRow(1, Guid.NewGuid(), Guid.NewGuid(), "ACTIVE", 0, 0, 0, 0, 0, 0));

                Assert.That(File.Exists(path), Is.True);
            }
            finally
            {
                if (Directory.Exists(dir)) Directory.Delete(dir, recursive: true);
            }
        }
    }
}
