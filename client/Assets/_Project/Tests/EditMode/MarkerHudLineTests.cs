// F-7 (05_qa_report_r5.md, round R5). SC-59 (b2) requires the reference markers' IDs and
// distances on the HUD; nothing drew that line until R13, so (b2) had no path to being closed by
// a recording. These tests cover the formatting half - see MarkerHudLine.cs's header.

using System.Collections.Generic;
using NUnit.Framework;
using Starfall.Greybox;
using Starfall.Sim;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class MarkerHudLineTests
    {
        static ReferenceMarker Marker(string id, double x, double y, double z) => new ReferenceMarker
        {
            Id = id,
            DisplayName = id,
            Kind = "test",
            PositionM = new Vec3d(x, y, z),
            VisualRadiusM = 100.0,
        };

        [Test]
        public void Format_EveryMarkerIdAndItsOwnDistanceAppear()
        {
            // Distinct axes and distinct distances: a formatter that printed one marker's
            // distance under another's id would satisfy any "contains every id" check, and
            // mis-pairing is the failure this line exists to rule out.
            var markers = new List<ReferenceMarker>
            {
                Marker("alpha", 300, 0, 0),
                Marker("beta", 0, 500, 0),
                Marker("gamma", 0, 0, 700),
                Marker("delta", 0, 0, -900),
            };

            string line = MarkerHudLine.Format(markers, new Vec3d(0, 0, 0));

            Assert.That(line, Does.Contain("alpha=300.0m"));
            Assert.That(line, Does.Contain("beta=500.0m"));
            Assert.That(line, Does.Contain("gamma=700.0m"));
            Assert.That(line, Does.Contain("delta=900.0m"), "distance is a magnitude - a marker behind the origin is 900m away, not -900m");
        }

        [Test]
        public void Format_DistanceIsMeasuredFromTheShip_NotFromTheOrigin()
        {
            // §7b(1) pairing for the test above, whose ship sits at the origin: with the ship at
            // the origin, a Format() that ignored shipPositionM entirely would pass it. This is
            // the case that separates the two.
            var markers = new List<ReferenceMarker> { Marker("alpha", 300, 0, 0) };

            string atOrigin = MarkerHudLine.Format(markers, new Vec3d(0, 0, 0));
            string alongside = MarkerHudLine.Format(markers, new Vec3d(250, 0, 0));

            Assert.That(atOrigin, Does.Contain("alpha=300.0m"));
            Assert.That(alongside, Does.Contain("alpha=50.0m"));
        }

        [Test]
        public void Format_OrderFollowsTheDataNotTheDistance()
        {
            // Stable across frames so two recordings can be diffed field by field: the nearest
            // marker changing must not reshuffle the line.
            var markers = new List<ReferenceMarker>
            {
                Marker("far", 9000, 0, 0),
                Marker("near", 10, 0, 0),
            };

            string line = MarkerHudLine.Format(markers, new Vec3d(0, 0, 0));

            Assert.That(line.IndexOf("far=", System.StringComparison.Ordinal),
                Is.LessThan(line.IndexOf("near=", System.StringComparison.Ordinal)));
        }

        [Test]
        public void Format_NoMarkers_SaysNone_NotAnEmptyString()
        {
            // §7a: a blank HUD row is indistinguishable from the row not being drawn at all -
            // exactly the ambiguity that let F-1 hide three counters for four rounds.
            Assert.That(MarkerHudLine.Format(new List<ReferenceMarker>(), new Vec3d(0, 0, 0)), Is.EqualTo("markers=none"));
            Assert.That(MarkerHudLine.Format(null, new Vec3d(0, 0, 0)), Is.EqualTo("markers=none"));
        }

        [Test]
        public void Format_FourMarkerLine_SurvivesTheHudWrapWithEveryFieldIntact()
        {
            // The marker line is the longest on the HUD, so it is the most likely next victim of
            // F-1. Ties this file to the wrap fix: after BuildHudRows, all four ids must still be
            // present somewhere in the drawn rows.
            var markers = new List<ReferenceMarker>
            {
                Marker("marker-alpha-long-id", 1234.5, 0, 0),
                Marker("marker-beta-long-id", 0, 2345.6, 0),
                Marker("marker-gamma-long-id", 0, 0, 3456.7),
                Marker("marker-delta-long-id", 4567.8, 0, 0),
            };

            string line = MarkerHudLine.Format(markers, new Vec3d(0, 0, 0));
            List<string> rows = GreyboxSession.BuildHudRows(new List<string> { line });
            string joined = string.Join("\n", rows);

            Assert.That(joined, Does.Contain("marker-alpha-long-id="));
            Assert.That(joined, Does.Contain("marker-beta-long-id="));
            Assert.That(joined, Does.Contain("marker-gamma-long-id="));
            Assert.That(joined, Does.Contain("marker-delta-long-id="));
            foreach (string row in rows)
                Assert.That(row.Length, Is.LessThanOrEqualTo(GreyboxSession.HudMaxCharsPerLine));
        }
    }
}
