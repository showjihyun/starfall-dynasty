// Headless entry points for QA: hold a realtime connection open from a Unity Editor process,
// and probe what a domain reload does to it.
//
// Why this exists instead of "unity command editor_play": `unity command` and `unity run
// --command` talk to the Pipeline package, which this project does not have (measured -
// "No Pipeline instance found for project"). Adding a package that runs an RPC server inside
// the Editor, right before a latency measurement, is not a trade worth making for one
// connection.
//
// Play mode is not needed either. The 31st connection only has to be a real Starfall.Net
// client inside a long-lived Editor process, and pumping it from here is what
// MonoBehaviour.Update does in play mode. So this runs headless, in EditMode, no extra
// package:
//
//   unity run client -- -executeMethod Starfall.Net.EditorTools.StarfallNetHold.HoldOpen
//
// It stops when the sentinel file appears, when the budget expires, or on an error - never
// by being killed, because a killed Editor closes the socket without a close frame and the
// server then records TRANSPORT_ERROR instead of CLIENT_CLOSED.

using System;
using System.Globalization;
using System.IO;
using UnityEditor;
using UnityEngine;
using Starfall.Net;

namespace Starfall.Net.EditorTools
{
    public static class StarfallNetHold
    {
        /// <summary>File whose appearance releases the held connection.</summary>
        public const string SentinelVariable = "STARFALL_NET_HOLD_SENTINEL";

        /// <summary>Seconds to hold before giving up on the sentinel. Default 300.</summary>
        public const string BudgetVariable = "STARFALL_NET_HOLD_SECONDS";

        /// <summary>Where the harness writes session_id / correlation_id for the QA set.</summary>
        public const string OutputVariable = "STARFALL_NET_HOLD_OUTPUT";

        static RealtimeClient _client;
        static UnityLogSink _log;
        static string _sentinel;
        static string _output;
        static DateTime _deadline;
        static bool _reported;

        /// <summary>
        /// Connects and holds the connection until released, <b>without returning</b>.
        /// This is the entry point for a headless hold:
        /// <code>
        /// unity run client -- -executeMethod Starfall.Net.EditorTools.StarfallNetHold.HoldOpen
        /// </code>
        /// <para>
        /// The loop is here rather than on <c>EditorApplication.update</c> because
        /// <c>unity run</c> passes <c>-quit</c> and the Editor exits the moment
        /// <c>-executeMethod</c> returns (measured: the connection opened, then the process
        /// tore down without a close frame and the server recorded TRANSPORT_ERROR). Holding
        /// the main thread is what keeps the Editor - and the connection - alive.
        /// </para>
        /// </summary>
        public static void HoldOpen()
        {
            if (!Start(reloadProbe: false)) return;

            while (true)
            {
                _client.Pump();

                if (!string.IsNullOrEmpty(_sentinel) && File.Exists(_sentinel)) { Finish("sentinel", 0); return; }
                if (DateTime.UtcNow > _deadline) { Finish("budget expired", 3); return; }

                System.Threading.Thread.Sleep(10);
            }
        }

        /// <summary>
        /// U-5b: connects, then forces a domain reload while connected, so the server's
        /// SESSION_CLOSED.close_reason can be compared against the reason this client logged.
        /// </summary>
        public static void ReloadProbe()
        {
            if (!Start(reloadProbe: true)) return;

            // Block only until the session is up, then hand the main thread back. A reload
            // cannot be processed while this method holds the thread, so unlike HoldOpen this
            // one has to return - and what happens next is exactly the measurement: either
            // the Editor runs beforeAssemblyReload (and the hook closes the socket, so the
            // server records CLIENT_CLOSED), or it tears down first (TRANSPORT_ERROR).
            while (!_reported && DateTime.UtcNow <= _deadline)
            {
                _client.Pump();
                System.Threading.Thread.Sleep(10);
            }

            if (!_reported)
            {
                Finish("never became ready", 4);
                return;
            }

            _log.Info(StarfallNetLog.Prefix + "reload probe: requesting a script reload while connected");
            EditorUtility.RequestScriptReload();
        }

        static bool Start(bool reloadProbe)
        {
            _sentinel = Environment.GetEnvironmentVariable(SentinelVariable);
            _output = Environment.GetEnvironmentVariable(OutputVariable);

            int budgetSeconds = 300;
            string budget = Environment.GetEnvironmentVariable(BudgetVariable);
            if (!string.IsNullOrEmpty(budget))
                int.TryParse(budget, NumberStyles.Integer, CultureInfo.InvariantCulture, out budgetSeconds);
            _deadline = DateTime.UtcNow.AddSeconds(budgetSeconds);

            string token;
            try
            {
                token = DevAuthToken.FromEnvironment();
            }
            catch (InvalidOperationException ex)
            {
                Debug.LogError("starfall.net: hold failed - " + ex.Message);
                EditorApplication.Exit(2);
                return false;
            }

            string url = Environment.GetEnvironmentVariable(StarfallNetHost.UrlVariable);
            if (string.IsNullOrEmpty(url)) url = StarfallNetHost.DefaultUrl;

            _log = new UnityLogSink();
            _client = new RealtimeClient(new PcWebSocketTransport(_log), _log, jitterSeed: Environment.TickCount);
            _client.SessionReady += OnSessionReady;

            // The same hook StarfallNetHost installs, for the same reason: a domain reload
            // unloads this assembly without awaiting anything, so the close has to be blocking
            // or the server records TRANSPORT_ERROR (AC-13c).
            AssemblyReloadEvents.beforeAssemblyReload += OnBeforeAssemblyReload;

            _log.Info(StarfallNetLog.Prefix + "hold starting: url=" + url +
                      " subject=" + DevAuthToken.ResolveSubject() +
                      " sentinel=" + (_sentinel ?? "<none>") +
                      " budget_s=" + budgetSeconds +
                      " reload_probe=" + reloadProbe);

            _client.Connect(url, token);
            return true;
        }

        static void OnSessionReady(SessionIdentity identity)
        {
            if (_reported) return;
            _reported = true;

            if (!string.IsNullOrEmpty(_output))
            {
                try
                {
                    Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(_output)));
                    File.AppendAllText(_output,
                        identity.CorrelationId.ToString("D") + "," +
                        identity.SessionId.ToString("D") + "," +
                        identity.ActorId.ToString("D") + "\n");
                }
                catch (Exception ex)
                {
                    Debug.LogWarning("starfall.net: could not write " + _output + " (" + ex.GetType().Name + ")");
                }
            }

        }

        static void OnBeforeAssemblyReload()
        {
            if (_client == null) return;

            _log.Info(StarfallNetLog.Prefix + "beforeAssemblyReload: closing before the domain goes away");
            bool completed = _client.DisconnectBlocking(ClientCloseReason.EditorReload, StarfallNetHost.CloseBudgetMs);
            _log.Info(StarfallNetLog.Prefix + "beforeAssemblyReload close completed=" + completed);
        }

        static void Finish(string why, int exitCode)
        {
            AssemblyReloadEvents.beforeAssemblyReload -= OnBeforeAssemblyReload;

            if (_client != null)
            {
                _log.Info(StarfallNetLog.Prefix + "hold finishing (" + why + ")");
                _client.DisconnectBlocking(ClientCloseReason.ClientClosed, StarfallNetHost.CloseBudgetMs);
                _client.Dispose();
                _client = null;
            }

            EditorApplication.Exit(exitCode);
        }
    }
}
