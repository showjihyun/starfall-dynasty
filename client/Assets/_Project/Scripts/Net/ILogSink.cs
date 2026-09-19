// Logging seam. The transport runs on background tasks and UnityEngine.Debug is main-thread
// only, so the transport never logs directly - it hands text to a sink the owner drains.

using System;
using System.Collections.Generic;

namespace Starfall.Net
{
    /// <summary>Where diagnostics go. Implementations must be safe to call from any thread.</summary>
    public interface ILogSink
    {
        void Info(string message);
        void Warn(string message);
        void Error(string message);
    }

    /// <summary>Drops everything. Default so a transport constructed without a sink is still
    /// usable in a unit test.</summary>
    public sealed class NullLogSink : ILogSink
    {
        public static readonly NullLogSink Instance = new NullLogSink();
        NullLogSink() { }
        public void Info(string message) { }
        public void Warn(string message) { }
        public void Error(string message) { }
    }

    /// <summary>Records every line for assertions. Thread-safe because the transport writes
    /// from its background tasks.</summary>
    public sealed class RecordingLogSink : ILogSink
    {
        readonly object _gate = new object();
        readonly List<string> _lines = new List<string>();

        public void Info(string message) { Add("INFO ", message); }
        public void Warn(string message) { Add("WARN ", message); }
        public void Error(string message) { Add("ERROR ", message); }

        void Add(string level, string message)
        {
            lock (_gate) _lines.Add(level + message);
        }

        /// <summary>Snapshot of the lines so far.</summary>
        public string[] Lines
        {
            get { lock (_gate) return _lines.ToArray(); }
        }

        /// <summary>True when some line contains <paramref name="fragment"/> verbatim.</summary>
        public bool Contains(string fragment)
        {
            if (fragment == null) throw new ArgumentNullException(nameof(fragment));
            lock (_gate)
            {
                for (int i = 0; i < _lines.Count; i++)
                    if (_lines[i].IndexOf(fragment, StringComparison.Ordinal) >= 0) return true;
            }
            return false;
        }

        public void Clear() { lock (_gate) _lines.Clear(); }
    }
}
