// SC-59 (b1) "배치" (contract 9차, architect R4 보충 §G): "기준 마커가 4개 존재하고 정해진 좌표에
// 있다" is closed by an EditMode data assertion, not video - (b2) "가시성" (at least one marker
// visible throughout the recording) stays a video requirement, since visibility is a camera/
// framing property this test cannot see. Splitting them means a recording that only shows one
// marker no longer silently passes "there are 4 markers" too - the two claims are independent
// and this file only ever answers the first.
//
// Reads data/world/systems/cradle.json the same way GreyboxSession.LoadData() does
// (StarSystemData.FromJson), so this is the actual data the greybox scene builds markers from -
// not a hand-copied expectation that could drift from the file.

using System.IO;
using NUnit.Framework;
using Starfall.Sim;
using UnityEngine;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class ReferenceMarkerPlacementTests
    {
        StarSystemData _starSystem;

        [SetUp]
        public void SetUp()
        {
            string cradlePath = Path.Combine(Application.dataPath, "_Project", "Data", "world", "systems", "cradle.json");
            Assert.That(File.Exists(cradlePath), Is.True, "cradle.json must exist for this test to mean anything");
            _starSystem = StarSystemData.FromJson(File.ReadAllText(cradlePath));
        }

        [Test]
        public void Cradle_HasExactlyFourReferenceMarkers()
        {
            Assert.That(_starSystem.ReferenceMarkers, Is.Not.Null);
            Assert.That(_starSystem.ReferenceMarkers.Count, Is.EqualTo(4),
                "SC-59 (b1): the recording proves only that ONE marker stayed visible - this " +
                "assertion is what proves the other three still exist at all");
        }

        [TestCase("star-cradle", 0.0, 0.0, 0.0)]
        [TestCase("planet-vela", 3000.0, -200.0, -900.0)]
        [TestCase("beacon-meridian", -1800.0, 900.0, 2200.0)]
        [TestCase("derelict-unnamed", 700.0, -1400.0, -3900.0)]
        public void Cradle_MarkerIsAtItsDesignatedCoordinate(string id, double x, double y, double z)
        {
            ReferenceMarker found = null;
            foreach (ReferenceMarker marker in _starSystem.ReferenceMarkers)
            {
                if (marker.Id == id) { found = marker; break; }
            }

            Assert.That(found, Is.Not.Null, "marker id='" + id + "' must exist in cradle.json");
            Assert.That(found.PositionM.X, Is.EqualTo(x).Within(1e-9));
            Assert.That(found.PositionM.Y, Is.EqualTo(y).Within(1e-9));
            Assert.That(found.PositionM.Z, Is.EqualTo(z).Within(1e-9));
        }

        [Test]
        public void Cradle_NoTwoMarkersShareAnId()
        {
            var seen = new System.Collections.Generic.HashSet<string>();
            foreach (ReferenceMarker marker in _starSystem.ReferenceMarkers)
            {
                Assert.That(seen.Add(marker.Id), Is.True,
                    "duplicate marker id '" + marker.Id + "' - GreyboxSession would build two " +
                    "primitives for one designated marker and a viewer could not tell");
            }
        }
    }
}
