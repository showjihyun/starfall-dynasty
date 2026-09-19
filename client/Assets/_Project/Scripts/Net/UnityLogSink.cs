// Where the transport's diagnostics land in the Editor.
//
// Two destinations on purpose:
//   * UnityEngine.Debug, so a developer sees it in the Console;
//   * client/Logs/starfall-net.log, so QA can grep one line per session without wading
//     through Editor.log, which interleaves every other subsystem and appends a stack trace
//     under every Debug.Log call.
//
// The line text itself is fixed in StarfallNetLog and is a contract with the QA harness
// (02_client_ack.md section 1).

using System;
using System.Globalization;
using System.IO;
using System.Text;
using UnityEngine;

namespace Starfall.Net
{
    public sealed class UnityLogSink : ILogSink
    {
        /// <summary>Relative to the project folder (the parent of Assets/), which is where
        /// Unity already writes Logs/Editor.log. That folder is gitignored.</summary>
        public const string RelativeLogPath = "Logs/starfall-net.log";

        readonly object _fileGate = new object();
        readonly string _path;
        readonly bool _toFile;

        public UnityLogSink(bool alsoWriteFile = true)
        {
            _toFile = alsoWriteFile;
            _path = ResolvePath();
        }

        /// <summary>Absolute path of the file this sink appends to, for the implementation
        /// summary and for QA.</summary>
        public string FilePath => _path;

        public void Info(string message) { Debug.Log(message); Append("INFO", message); }
        public void Warn(string message) { Debug.LogWarning(message); Append("WARN", message); }
        public void Error(string message) { Debug.LogError(message); Append("ERROR", message); }

        static string ResolvePath()
        {
            // Application.dataPath is <project>/Assets.
            string assets = Application.dataPath;
            string project = Directory.GetParent(assets)?.FullName ?? assets;
            return Path.Combine(project, RelativeLogPath.Replace('/', Path.DirectorySeparatorChar));
        }

        void Append(string level, string message)
        {
            if (!_toFile || string.IsNullOrEmpty(_path)) return;

            // Called from the main thread today, but the lock costs nothing and a future
            // caller on another thread would otherwise interleave half-lines.
            lock (_fileGate)
            {
                try
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(_path));
                    var line = new StringBuilder(message.Length + 40);
                    line.Append(DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture));
                    line.Append(' ').Append(level).Append(' ').Append(message).Append('\n');
                    File.AppendAllText(_path, line.ToString(), Encoding.UTF8);
                }
                catch (Exception ex)
                {
                    // Never let logging take the session down. One warning, then stay quiet:
                    // Debug.LogWarning here cannot recurse because Append is not called for it.
                    Debug.LogWarning("starfall.net: could not append to " + _path + " (" + ex.GetType().Name + ")");
                }
            }
        }
    }
}
