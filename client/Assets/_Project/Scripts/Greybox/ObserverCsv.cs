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

        public ObserverCsvRow(
            long tick, Guid observerActorId, Guid shipId, string presence,
            long positionXMm, long positionYMm, long positionZMm,
            long velocityXMmS, long velocityYMmS, long velocityZMmS)
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
        }

        /// <summary>Must match tests/e2e/two_client_view.py's COLUMNS exactly (name, order,
        /// count) - that script rejects a header mismatch as "NotImplementedYet" rather than
        /// silently accepting a different shape.</summary>
        public const string Header =
            "tick,observer_actor_id,ship_id,presence,px_mm,py_mm,pz_mm,vx_mm_s,vy_mm_s,vz_mm_s";

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
                VelocityZMmS.ToString(CultureInfo.InvariantCulture));
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
