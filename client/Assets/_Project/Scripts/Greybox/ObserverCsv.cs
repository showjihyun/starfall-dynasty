// Hand-written. SC-64/65 (sprint contract section I, AC-16 / designer S-3): the row format two
// independent observers of the same world write, so QA's tests/e2e/two_client_view.py can join
// them on (tick, ship_id). Pure C# (no UnityEngine types) so it is testable in EditMode without
// a live connection - only ObserverSession (Greybox, MonoBehaviour) needs a Player/Editor.
//
// Column names/order are QA-owned (tests/e2e/two_client_view.py's COLUMNS constant, contract
// section 3.1) - "안 맞으면 QA에게 맞춰 달라고 하지 말고 네가 맞춰라" (client task doc). Do not
// reorder or rename without re-reading that script first.

using System;
using System.Globalization;
using System.IO;

namespace Starfall.Greybox
{
    /// <summary>One observation: what one observer's WORLD_SNAPSHOT-driven pipeline (prediction
    /// for its own ship, interpolation for every other ship - ADR-0012 section 1) currently
    /// shows for one ship, tagged with the snapshot's envelope tick.</summary>
    public readonly struct ObserverCsvRow
    {
        public readonly long Tick;
        public readonly Guid ObserverActorId;
        public readonly Guid ShipId;
        public readonly string Presence;
        public readonly long PositionXMm;
        public readonly long PositionYMm;
        public readonly long PositionZMm;
        public readonly long VelocityXMmS;
        public readonly long VelocityYMmS;
        public readonly long VelocityZMmS;

        /// <summary>R23 (architect R10 후속 판정 §1/§4, 01_architect_decisions.md "## R10 후속
        /// 판정 (R22 이후)"): the F-33 render smoothing offset baked into THIS row's position (0
        /// for every other-ship row - those are already pure presentation/interpolation values
        /// with no offset of their own to report). Carried as a SEPARATE column, not just folded
        /// into px/py/pz, so a judge reading a near-threshold SC-64/65 difference can tell
        /// "smoothing accounts for this much of it" from "the states actually differ" without
        /// re-deriving it from the session log.</summary>
        public readonly long RenderOffsetMm;

        /// <summary>Same reasoning as RenderOffsetMm, for the orientation offset's magnitude in
        /// degrees. Not quantised to the wire's milli-scale (no _m/_mm suffix) - this is a
        /// diagnostic-only column, never round-tripped as a command/state field.</summary>
        public readonly double RenderOffsetDeg;

        public ObserverCsvRow(
            long tick, Guid observerActorId, Guid shipId, string presence,
            long positionXMm, long positionYMm, long positionZMm,
            long velocityXMmS, long velocityYMmS, long velocityZMmS,
            long renderOffsetMm = 0, double renderOffsetDeg = 0.0)
        {
            Tick = tick;
            ObserverActorId = observerActorId;
            ShipId = shipId;
            Presence = presence ?? "ACTIVE";
            PositionXMm = positionXMm;
            PositionYMm = positionYMm;
            PositionZMm = positionZMm;
            VelocityXMmS = velocityXMmS;
            VelocityYMmS = velocityYMmS;
            VelocityZMmS = velocityZMmS;
            RenderOffsetMm = renderOffsetMm;
            RenderOffsetDeg = renderOffsetDeg;
        }

        /// <summary>Must match tests/e2e/two_client_view.py's COLUMNS exactly (name, order,
        /// count) - that script rejects a header mismatch as "NotImplementedYet" rather than
        /// silently accepting a different shape. R23: two columns ADDED AT THE END
        /// (render_offset_mm, render_offset_deg) - the original ten are untouched, per
        /// architect's explicit instruction not to reorder/rename existing columns. qa is making
        /// the SAME two-column addition to two_client_view.py's COLUMNS in the same round -
        /// block 7 (SC-63/64/65) must not run until both sides have landed (architect R10 후속
        /// §5.5).</summary>
        public const string Header =
            "tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s,render_offset_mm,render_offset_deg";

        public string ToCsvLine()
        {
            return string.Join(",",
                Tick.ToString(CultureInfo.InvariantCulture),
                ObserverActorId.ToString(),
                ShipId.ToString(),
                Presence,
                PositionXMm.ToString(CultureInfo.InvariantCulture),
                PositionYMm.ToString(CultureInfo.InvariantCulture),
                PositionZMm.ToString(CultureInfo.InvariantCulture),
                VelocityXMmS.ToString(CultureInfo.InvariantCulture),
                VelocityYMmS.ToString(CultureInfo.InvariantCulture),
                VelocityZMmS.ToString(CultureInfo.InvariantCulture),
                RenderOffsetMm.ToString(CultureInfo.InvariantCulture),
                RenderOffsetDeg.ToString("F4", CultureInfo.InvariantCulture));
        }
    }

    /// <summary>Appends rows to one observer's CSV file, header written once at construction.
    /// Flushes every write: this is a diagnostic tool run at snapshot rate (10 Hz), not a hot
    /// path, and QA (or a human) may tail the file while the session is still running.</summary>
    public sealed class ObserverCsvWriter : IDisposable
    {
        readonly StreamWriter _writer;

        public string Path { get; }

        public ObserverCsvWriter(string path)
        {
            if (string.IsNullOrEmpty(path)) throw new ArgumentException("path is required", nameof(path));
            Path = path;

            string directory = System.IO.Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

            _writer = new StreamWriter(path, append: false);
            _writer.WriteLine(ObserverCsvRow.Header);
            _writer.Flush();
        }

        public void Write(ObserverCsvRow row)
        {
            _writer.WriteLine(row.ToCsvLine());
            _writer.Flush();
        }

        public void Dispose() => _writer.Dispose();
    }
}
