// F-7 (05_qa_report_r5.md, round R5). Sprint contract SC-59 (b2) requires the reference markers'
// IDs and distances on the HUD of a recording's opening frame - that is how a viewer of the
// recording knows WHICH four markers the ship's motion is being judged against, without trusting
// a caption written afterwards. BuildMarkers() had existed since C3 and put the spheres in the
// scene, but nothing ever drew that line, so (b2) had no path to being closed by a recording at
// all (qa r5 F-7). Four rounds of recordings could never have satisfied it.
//
// Pure and free of UnityEngine so the format is unit-testable without a scene, a camera or an
// OnGUI call stack - same discipline as TickCatchUp, SnapshotRebaseBatch and PeriodicStatusLog
// in this folder.

using System.Collections.Generic;
using System.Globalization;
using System.Text;
using Starfall.Sim;

namespace Starfall.Greybox
{
    public static class MarkerHudLine
    {
        /// <summary>One `markers=` line: every marker's id paired with its distance from
        /// <paramref name="shipPositionM"/>, in metres. Markers are listed in the order the star
        /// system data declares them (never sorted by distance) so the line reads the same across
        /// frames and two recordings can be diffed field by field.
        ///
        /// Returns a line with an explicit "none" rather than an empty string when there are no
        /// markers: §7a - a blank HUD row is indistinguishable from the row not being drawn, and
        /// that ambiguity is precisely how F-1 hid three counters for four rounds.</summary>
        public static string Format(IReadOnlyList<ReferenceMarker> markers, Vec3d shipPositionM)
        {
            if (markers == null || markers.Count == 0) return "markers=none";

            var sb = new StringBuilder("markers=");
            for (int i = 0; i < markers.Count; i++)
            {
                ReferenceMarker marker = markers[i];
                if (i > 0) sb.Append(' ');

                double dx = marker.PositionM.X - shipPositionM.X;
                double dy = marker.PositionM.Y - shipPositionM.Y;
                double dz = marker.PositionM.Z - shipPositionM.Z;
                double distance = new Vec3d(dx, dy, dz).Length();

                sb.Append(marker.Id)
                  .Append('=')
                  .Append(distance.ToString("F1", CultureInfo.InvariantCulture))
                  .Append('m');
            }

            return sb.ToString();
        }
    }
}
