// The client's view of a command's life. Rules: invariant I-15, ADR-0005 sections 3 and 5.
//
// The shape here is the shape every command in p1 will follow, which is why it exists in a
// slice whose only command is a ping. The one thing it must not do is treat
// COMMAND_RESULT{ACCEPTED} as completion: "the server took it into the simulation on this
// tick" is not "the intent happened". A state machine that collapses the two never exercises
// its own rejection path.

using System;
using System.Collections.Generic;

namespace Starfall.Net
{
    /// <summary>Where a sent command currently is.</summary>
    public enum CommandStatus
    {
        /// <summary>On the wire, no COMMAND_RESULT yet.</summary>
        Sent,

        /// <summary>COMMAND_RESULT{ACCEPTED} arrived. Not done: the type-specific result
        /// follows.</summary>
        Accepted,

        /// <summary>The type-specific result arrived. Terminal.</summary>
        Completed,

        /// <summary>COMMAND_RESULT{REJECTED}. Terminal, with a reason code.</summary>
        Rejected,

        /// <summary>The connection ended before an answer. Terminal, and <b>not</b> retried
        /// (I-23): the server's dedup is per-connection, so a resend after a reconnect could
        /// apply twice.</summary>
        ConnectionLost,
    }

    /// <summary>A command's terminal outcome, handed to the caller.</summary>
    public readonly struct CommandOutcome
    {
        public readonly Guid CommandId;
        public readonly CommandStatus Status;

        /// <summary>Reason code string from COMMAND_RESULT, or null. Deliberately a string,
        /// not an enum: the server may add values and an older client must not drop the
        /// message (ADR-0005 section 4).</summary>
        public readonly string ReasonCode;

        /// <summary>Round trip on the client's own clock, in milliseconds. The client measures
        /// this itself; client_sent_at is never used for timing (I-11).</summary>
        public readonly double ElapsedMs;

        public CommandOutcome(Guid commandId, CommandStatus status, string reasonCode, double elapsedMs)
        {
            CommandId = commandId;
            Status = status;
            ReasonCode = reasonCode;
            ElapsedMs = elapsedMs;
        }
    }

    /// <summary>
    /// Tracks commands from send to a terminal outcome. Main thread only: it is driven from
    /// the transport pump.
    /// </summary>
    public sealed class PendingCommands
    {
        sealed class Entry
        {
            public CommandStatus Status;
            public long SentTicks;
            public uint ProbeSeq;
        }

        readonly Dictionary<Guid, Entry> _entries = new Dictionary<Guid, Entry>();
        readonly List<Guid> _scratch = new List<Guid>();
        readonly Func<long> _clockTicks;

        /// <param name="clockTicks">Monotonic tick source, injected so tests do not sleep.</param>
        public PendingCommands(Func<long> clockTicks)
        {
            _clockTicks = clockTicks ?? throw new ArgumentNullException(nameof(clockTicks));
        }

        /// <summary>Raised once per command, when it reaches a terminal status.</summary>
        public event Action<CommandOutcome> Settled;

        public int Count => _entries.Count;

        public bool TryGetStatus(Guid commandId, out CommandStatus status)
        {
            Entry entry;
            if (_entries.TryGetValue(commandId, out entry)) { status = entry.Status; return true; }
            status = CommandStatus.ConnectionLost;
            return false;
        }

        /// <summary>Records a command as sent.</summary>
        public void Track(Guid commandId, uint probeSeq)
        {
            if (_entries.ContainsKey(commandId))
                throw new InvalidOperationException("command_id " + commandId + " is already in flight. Ids are single use.");

            _entries[commandId] = new Entry
            {
                Status = CommandStatus.Sent,
                SentTicks = _clockTicks(),
                ProbeSeq = probeSeq,
            };
        }

        /// <summary>Applies a COMMAND_RESULT. Returns false when the id is unknown, which is
        /// itself a finding: the server answered something we never sent.</summary>
        public bool OnCommandResult(Guid commandId, string status, string reasonCode)
        {
            Entry entry;
            if (!_entries.TryGetValue(commandId, out entry)) return false;

            if (string.Equals(status, "ACCEPTED", StringComparison.Ordinal))
            {
                // Not terminal. The PING_REPLY that completes it is still to come (I-15).
                entry.Status = CommandStatus.Accepted;
                return true;
            }

            // Anything that is not ACCEPTED is terminal, including a status value this build
            // does not know: an unknown status must not leave a command in flight forever.
            Settle(commandId, entry, CommandStatus.Rejected, reasonCode);
            return true;
        }

        /// <summary>Applies the type-specific result that completes a command.</summary>
        public bool OnTypeResult(Guid commandId, uint probeSeq, out bool orderViolation, out bool probeMismatch)
        {
            orderViolation = false;
            probeMismatch = false;

            Entry entry;
            if (!_entries.TryGetValue(commandId, out entry)) return false;

            // I-15: COMMAND_RESULT is delivered before the type result for the same command.
            // Seeing the reply first is a server-side ordering bug, and swallowing it here is
            // how such a bug survives a load test.
            orderViolation = entry.Status == CommandStatus.Sent;
            probeMismatch = entry.ProbeSeq != probeSeq;

            Settle(commandId, entry, CommandStatus.Completed, null);
            return true;
        }

        /// <summary>
        /// Fails everything still in flight because the connection ended. Returns how many,
        /// for the fixed log line that is AC-14's evidence.
        /// </summary>
        public int FailAllOnDisconnect()
        {
            _scratch.Clear();
            foreach (KeyValuePair<Guid, Entry> pair in _entries) _scratch.Add(pair.Key);

            for (int i = 0; i < _scratch.Count; i++)
            {
                Guid id = _scratch[i];
                Settle(id, _entries[id], CommandStatus.ConnectionLost, null);
            }

            int count = _scratch.Count;
            _scratch.Clear();
            return count;
        }

        void Settle(Guid commandId, Entry entry, CommandStatus status, string reasonCode)
        {
            _entries.Remove(commandId);

            double elapsedMs = (_clockTicks() - entry.SentTicks) / (double)TimeSpan.TicksPerMillisecond;
            Action<CommandOutcome> handler = Settled;
            if (handler != null) handler(new CommandOutcome(commandId, status, reasonCode, elapsedMs));
        }
    }
}
