// The exact log lines QA parses. Changing a string here changes a contract with the QA
// harness, not just a log message.
//
// Fixed in _workspace/p0-02-networking-spike/02_client_ack.md section 1 and referenced by
// SC-49, SC-50, SC-51 and SC-61. Field order inside a line is part of the contract: the QA
// regex anchors on "session_id=<uuid> correlation_id=<uuid>".

using System;
using System.Globalization;

namespace Starfall.Net
{
    /// <summary>Why this client closed a connection. Not the contract's close_reason - the
    /// server decides that. The two are compared during QA: every value here leaves as a
    /// normal close (1000), so the server must record CLIENT_CLOSED for all of them. A
    /// TRANSPORT_ERROR row with an EDITOR_RELOAD line next to it means the close raced the
    /// reload; a TRANSPORT_ERROR row with no line at all means the hook never ran.</summary>
    public enum ClientCloseReason
    {
        /// <summary>The caller asked to disconnect.</summary>
        ClientClosed,

        /// <summary>Editor domain reload is about to unload this assembly.</summary>
        EditorReload,

        /// <summary>Editor is leaving play mode.</summary>
        PlayModeExit,

        /// <summary>The application is quitting.</summary>
        AppQuit,
    }

    /// <summary>Builds the fixed log lines. Static and allocation-light: every line is built
    /// once per session or per reconnect, never per frame.</summary>
    public static class StarfallNetLog
    {
        /// <summary>Every line this client writes starts with this.</summary>
        public const string Prefix = "starfall.net: ";

        /// <summary>Token that QA greps for to collect the 31st session's correlation id.</summary>
        public const string SessionReadyTag = "SESSION_READY";

        /// <summary>
        /// <c>starfall.net: SESSION_READY session_id=… correlation_id=… actor_id=… world_id=…
        /// tick_hz=… server_version=… attempt=…</c>
        /// <para>
        /// attempt is on this line on purpose: SC-51 has to show that the retry counter resets
        /// only when a session actually becomes ready, and that is the moment it resets.
        /// </para>
        /// </summary>
        public static string SessionReady(
            Guid sessionId,
            Guid correlationId,
            Guid actorId,
            Guid worldId,
            int tickHz,
            string serverVersion,
            int attempt) =>
            Prefix + SessionReadyTag +
            " session_id=" + sessionId.ToString("D", CultureInfo.InvariantCulture) +
            " correlation_id=" + correlationId.ToString("D", CultureInfo.InvariantCulture) +
            " actor_id=" + actorId.ToString("D", CultureInfo.InvariantCulture) +
            " world_id=" + worldId.ToString("D", CultureInfo.InvariantCulture) +
            " tick_hz=" + tickHz.ToString(CultureInfo.InvariantCulture) +
            " server_version=" + (serverVersion ?? "") +
            " attempt=" + attempt.ToString(CultureInfo.InvariantCulture);

        /// <summary>
        /// <c>starfall.net: dropping N in-flight command(s) on disconnect (no resend, I-23)</c>
        /// <para>Written even when N is 0. "Nothing was in flight" is also evidence.</para>
        /// </summary>
        public static string DroppedInFlight(int count) =>
            Prefix + "dropping " + count.ToString(CultureInfo.InvariantCulture) +
            " in-flight command(s) on disconnect (no resend, I-23)";

        /// <summary>
        /// <c>starfall.net: reconnect attempt=n delay_ms=ms (counter resets only on SESSION_READY)</c>
        /// </summary>
        public static string Reconnect(int attempt, int delayMs) =>
            Prefix + "reconnect attempt=" + attempt.ToString(CultureInfo.InvariantCulture) +
            " delay_ms=" + delayMs.ToString(CultureInfo.InvariantCulture) +
            " (counter resets only on SESSION_READY)";

        /// <summary>
        /// <c>starfall.net: closing session_id=… reason=… code=1000</c>
        /// </summary>
        public static string Closing(Guid sessionId, ClientCloseReason reason) =>
            Prefix + "closing session_id=" + sessionId.ToString("D", CultureInfo.InvariantCulture) +
            " reason=" + Name(reason) + " code=1000";

        /// <summary>SCREAMING_SNAKE_CASE name, written by hand so it survives assembly
        /// stripping and never depends on Enum.ToString formatting.</summary>
        public static string Name(ClientCloseReason reason)
        {
            switch (reason)
            {
                case ClientCloseReason.ClientClosed: return "CLIENT_CLOSED";
                case ClientCloseReason.EditorReload: return "EDITOR_RELOAD";
                case ClientCloseReason.PlayModeExit: return "PLAYMODE_EXIT";
                case ClientCloseReason.AppQuit: return "APP_QUIT";
                default: return "UNKNOWN";
            }
        }
    }
}
