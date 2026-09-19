// The single MonoBehaviour that owns a RealtimeClient: it pumps it once per frame and closes
// it cleanly on every path that can unload this assembly.
//
// No scene and no prefab. This slice has no visuals (00_request.md) and creating a scene by
// hand means hand-editing YAML, which breaks GUID references. A bootstrap that runs after
// scene load lets an operator press Play in the default empty scene and hold a connection -
// which is exactly what SC-61 needs from the 31st client.

using System;
using System.Globalization;
using UnityEngine;

namespace Starfall.Net
{
    [DisallowMultipleComponent]
    public sealed class StarfallNetHost : MonoBehaviour
    {
        /// <summary>Default endpoint. The server binds to loopback only (ADR-0008 section 4).</summary>
        public const string DefaultUrl = "ws://127.0.0.1:8080/ws";

        /// <summary>Set to 1 to connect automatically when play mode starts.</summary>
        public const string AutoConnectVariable = "STARFALL_NET_AUTOCONNECT";

        /// <summary>Overrides <see cref="DefaultUrl"/>.</summary>
        public const string UrlVariable = "STARFALL_WS_URL";

        /// <summary>How long a close hook waits for the close frame before giving up. The
        /// Editor does not wait for us, so this is a budget, not a guarantee.</summary>
        public const int CloseBudgetMs = 1500;

        static StarfallNetHost _instance;

        RealtimeClient _client;
        UnityLogSink _log;
        bool _closed;

        /// <summary>The live host, or null.</summary>
        public static StarfallNetHost Instance => _instance;

        public RealtimeClient Client => _client;

        /// <summary>Creates the host if it does not exist yet.</summary>
        public static StarfallNetHost Ensure()
        {
            if (_instance != null) return _instance;

            var go = new GameObject("StarfallNetHost");
            DontDestroyOnLoad(go);
            return go.AddComponent<StarfallNetHost>();
        }

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void AutoConnectIfRequested()
        {
            if (Environment.GetEnvironmentVariable(AutoConnectVariable) != "1") return;

            StarfallNetHost host = Ensure();
            if (!host.TryConnect(out string error))
            {
                Debug.LogError("starfall.net: autoconnect failed - " + error);
            }
        }

        void Awake()
        {
            if (_instance != null && _instance != this)
            {
                Destroy(gameObject);
                return;
            }

            _instance = this;
            _log = new UnityLogSink();
            _client = new RealtimeClient(new PcWebSocketTransport(_log), _log, jitterSeed: Environment.TickCount);

            Application.quitting += OnApplicationQuitting;
#if UNITY_EDITOR
            UnityEditor.AssemblyReloadEvents.beforeAssemblyReload += OnBeforeAssemblyReload;
            UnityEditor.EditorApplication.playModeStateChanged += OnPlayModeStateChanged;
#endif
            _log.Info(StarfallNetLog.Prefix + "host ready, log file: " + _log.FilePath);
        }

        /// <summary>
        /// Connects with a token built from the environment. Returns false with a reason
        /// instead of throwing: a missing secret is an operator mistake, not a crash.
        /// </summary>
        public bool TryConnect(out string error)
        {
            error = null;

            string token;
            try
            {
                token = DevAuthToken.FromEnvironment();
            }
            catch (InvalidOperationException ex)
            {
                error = ex.Message;
                return false;
            }

            string url = Environment.GetEnvironmentVariable(UrlVariable);
            if (string.IsNullOrEmpty(url)) url = DefaultUrl;

            _closed = false;
            _log.Info(StarfallNetLog.Prefix + "connecting to " + url + " as subject " + DevAuthToken.ResolveSubject());
            _client.Connect(url, token);
            return true;
        }

        /// <summary>Sends n pings, returning how many actually went out.</summary>
        public int SendPings(int count)
        {
            int sent = 0;
            for (int i = 0; i < count; i++)
            {
                if (_client.SendPing().HasValue) sent++;
            }
            return sent;
        }

        public void Disconnect() => CloseOnce(ClientCloseReason.ClientClosed);

        void Update()
        {
            if (_client != null) _client.Pump();
        }

        void OnApplicationQuitting() => CloseOnce(ClientCloseReason.AppQuit);

        void OnDestroy()
        {
            // OnDestroy also fires on a normal scene teardown, so the reason is the mildest
            // one that fits; the specific hooks above set a better reason first and CloseOnce
            // keeps the first.
            CloseOnce(ClientCloseReason.AppQuit);

            Application.quitting -= OnApplicationQuitting;
#if UNITY_EDITOR
            UnityEditor.AssemblyReloadEvents.beforeAssemblyReload -= OnBeforeAssemblyReload;
            UnityEditor.EditorApplication.playModeStateChanged -= OnPlayModeStateChanged;
#endif
            if (_instance == this) _instance = null;
            if (_client != null) { _client.Dispose(); _client = null; }
        }

#if UNITY_EDITOR
        void OnBeforeAssemblyReload() => CloseOnce(ClientCloseReason.EditorReload);

        void OnPlayModeStateChanged(UnityEditor.PlayModeStateChange change)
        {
            if (change == UnityEditor.PlayModeStateChange.ExitingPlayMode)
                CloseOnce(ClientCloseReason.PlayModeExit);
        }
#endif

        /// <summary>
        /// Blocking close, at most once. Blocking is the point: domain reload and play mode
        /// exit do not await anything, so an async close would be torn down mid-handshake and
        /// the server would record TRANSPORT_ERROR instead of CLIENT_CLOSED (AC-13c).
        /// </summary>
        void CloseOnce(ClientCloseReason reason)
        {
            if (_closed || _client == null) return;
            _closed = true;

            bool completed = _client.DisconnectBlocking(reason, CloseBudgetMs);
            if (!completed)
            {
                Debug.LogWarning(StarfallNetLog.Prefix + "close did not finish within " +
                                 CloseBudgetMs.ToString(CultureInfo.InvariantCulture) +
                                 " ms (reason=" + StarfallNetLog.Name(reason) +
                                 "); the server may record TRANSPORT_ERROR");
            }
        }
    }
}
