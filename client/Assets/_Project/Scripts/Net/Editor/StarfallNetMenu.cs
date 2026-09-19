// Manual control of the realtime connection from the Editor.
//
// Why a menu and not a PlayMode test: `unity test --mode PlayMode` boots an Editor, runs, and
// exits, so the connection dies with the run. SC-61 needs the 31st connection to stay open
// across the whole 60 s load phase, which means a human-held Editor in play mode. These items
// are how that human opens, probes and closes it.

using UnityEditor;
using UnityEngine;

namespace Starfall.Net.EditorTools
{
    public static class StarfallNetMenu
    {
        const string Root = "Starfall/Net/";

        [MenuItem(Root + "Connect", validate = false)]
        public static void Connect()
        {
            if (!EditorApplication.isPlaying)
            {
                Debug.LogWarning("starfall.net: enter play mode first - the connection lives in the player loop.");
                return;
            }

            StarfallNetHost host = StarfallNetHost.Ensure();
            if (!host.TryConnect(out string error))
            {
                Debug.LogError("starfall.net: connect failed - " + error);
            }
        }

        [MenuItem(Root + "Send PING_SERVER x3")]
        public static void SendThreePings()
        {
            StarfallNetHost host = StarfallNetHost.Instance;
            if (host == null || !host.Client.IsReady)
            {
                Debug.LogWarning("starfall.net: no ready session. Connect first and wait for SESSION_READY.");
                return;
            }

            int sent = host.SendPings(3);
            Debug.Log("starfall.net: sent " + sent + " of 3 PING_SERVER");
        }

        [MenuItem(Root + "Disconnect")]
        public static void Disconnect()
        {
            StarfallNetHost host = StarfallNetHost.Instance;
            if (host == null)
            {
                Debug.LogWarning("starfall.net: no host.");
                return;
            }
            host.Disconnect();
        }

        [MenuItem(Root + "Print Dev Token Subject")]
        public static void PrintSubject()
        {
            // The subject, never the token: the token is a credential even in development and
            // the Console is copied into bug reports.
            bool hasSecret = DevAuthToken.ResolveSecret() != null;
            Debug.Log("starfall.net: subject=" + DevAuthToken.ResolveSubject() +
                      " secret_configured=" + hasSecret +
                      (hasSecret ? "" : " (export " + DevAuthToken.SecretVariable + " before starting the Editor)"));
        }
    }
}
