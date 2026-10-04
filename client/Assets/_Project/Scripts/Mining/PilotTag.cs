// Hand-written. C3 (p1-02-mining). "Pilot-xxxx" display tag - spec I-66, architect confirmed
// 2026-09-27 (_workspace/p1-02-mining/01_architect_tasks.md, relayed via 02_client_ack.md):
// the LAST 4 characters of actor_id (a UUIDv7 string), not the first 4 - the leading hex of a
// UUIDv7 is its creation-time bits, so two characters made close together in real time would
// collide there far more often than at the tail.
//
// WARNING (team-lead, confirmed in this project's own dev id scheme): even the last 4 characters
// collide in practice. Unity's DefaultSubject (...-8e57-000000000001) and bot-001
// (...-8000-000000000001) both produce "Pilot-0001"; SecondObserverSubject and bot-002 both
// produce "Pilot-0002". This tag is DISPLAY ONLY. Any test, log line or SQL cross-check MUST key
// on actor_id itself, never on the tag - see sprint contract section 0.11 and SC-110.

using System;

namespace Starfall.Mining
{
    public static class PilotTag
    {
        /// <summary>Pure function: actor_id -> "Pilot-xxxx" (last 4 hex characters of the
        /// "D"-format guid string, e.g. "01a0b1c2-7e57-7c11-8e57-000000000001" -> "Pilot-0001").
        /// Never used as a lookup key - see the file header.</summary>
        public static string From(Guid actorId)
        {
            string text = actorId.ToString("D");
            string last4 = text.Length >= 4 ? text.Substring(text.Length - 4) : text;
            return "Pilot-" + last4;
        }
    }
}
