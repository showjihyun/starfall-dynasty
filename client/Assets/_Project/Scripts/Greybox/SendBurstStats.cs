// SC-89 instrumentation (leader-approved 2026-09-23, R8/R9 finding: GreyboxSession.cs:418-431's
// catch-up loop can drain and send multiple ticks in one Update() call with no per-frame cap -
// this is what trips the server's protocol-violation budget, ADR-0011 section 5.2 / S10).
//
// Without a measured worst case, "it didn't disconnect this run" proves nothing (contract
// section 7a) - SC-89 needs to tell "the catch-up loop was actually bounded" from "this
// particular run got lucky". This class is the measurement, not the fix: it observes what
// GreyboxSession already does and does not change catch-up/send behavior in any way.
//
// Pure C#, no UnityEngine reference - same discipline as ReconcileErrorStats.cs in this folder
// (testable without a scene or Play mode). Unlike that file this class is stateful (an
// accumulator, not a one-shot compute over a caller-held list) because "max sends in the
// trailing 1 second" needs its own rolling window of timestamps to prune - the caller passes in
// realtime seconds (GreyboxSession passes Time.realtimeSinceStartupAsDouble) so this class never
// touches UnityEngine.Time itself.

using System.Collections.Generic;

namespace Starfall.Greybox
{
    public sealed class SendBurstStats
    {
        readonly List<double> _sendTimestampsSeconds = new List<double>();

        int? _maxTicksDrainedPerUpdate;
        int? _maxSendsInTrailingOneSecond;
        int? _maxSendsPerFrame;

        /// <summary>Highest number of ticks a single Update() call has drained from the catch-up
        /// loop this session, or null if no Update() has run a drain yet. 0 is a real, distinct
        /// measurement (the frame was caught up already) - same "null means unmeasured, not
        /// measured-and-zero" discipline as PercentileStats.Empty (ReconcileErrorStats.cs).</summary>
        public int? MaxTicksDrainedPerUpdate => _maxTicksDrainedPerUpdate;

        /// <summary>Highest count of successful sends observed in any trailing 1-second window
        /// this session, or null if no send has happened yet. At client_send_hz=20 with no
        /// hitch this should stay at 1 (one send per tick, ticks spread across the second) -
        /// anything higher within a burst is exactly what SC-89 watches for.</summary>
        public int? MaxSendsInTrailingOneSecond => _maxSendsInTrailingOneSecond;

        /// <summary>Highest number of successful sends observed in any SINGLE frame this
        /// session, or null if no Update() has run yet. This is the K-2/K-7 (architect) pairing
        /// value: SC-89 (b) pairs "a drain > 1 happened" (evidence a hitch/low-frame-rate
        /// condition actually occurred) with "sends per frame stayed == 1" (evidence the fix,
        /// not luck, is why the connection survived) - a background PAUSE (one huge catch-up
        /// frame, one violation, under budget) would pass a drain-only check on the pre-fix
        /// binary too; this value is what tells the two apart.</summary>
        public int? MaxSendsPerFrame => _maxSendsPerFrame;

        /// <summary>Call once per Update(), after the catch-up loop finishes, with how many
        /// ticks it drained this frame.</summary>
        public void RecordUpdate(int ticksDrainedThisUpdate)
        {
            if (_maxTicksDrainedPerUpdate == null || ticksDrainedThisUpdate > _maxTicksDrainedPerUpdate.Value)
                _maxTicksDrainedPerUpdate = ticksDrainedThisUpdate;
        }

        /// <summary>Call once per Update(), after the catch-up loop finishes, with how many of
        /// this frame's predicted ticks actually carried a successful network send (0 or 1 in
        /// production, since K=1 - test callers may explore other K values).</summary>
        public void RecordFrameSendCount(int sendsThisFrame)
        {
            if (_maxSendsPerFrame == null || sendsThisFrame > _maxSendsPerFrame.Value)
                _maxSendsPerFrame = sendsThisFrame;
        }

        /// <summary>Call once per actual successful send (TrySendJson returned true - a send
        /// GreyboxSession.SendAndPredictOneTick's queue-full/not-ready path skipped must NOT be
        /// recorded here, it never went on the wire). <paramref name="nowSeconds"/> is the
        /// caller's realtime clock, monotonic and in seconds.</summary>
        public void RecordSend(double nowSeconds)
        {
            _sendTimestampsSeconds.Add(nowSeconds);

            double cutoff = nowSeconds - 1.0;
            _sendTimestampsSeconds.RemoveAll(t => t < cutoff);

            int count = _sendTimestampsSeconds.Count;
            if (_maxSendsInTrailingOneSecond == null || count > _maxSendsInTrailingOneSecond.Value)
                _maxSendsInTrailingOneSecond = count;
        }
    }
}
