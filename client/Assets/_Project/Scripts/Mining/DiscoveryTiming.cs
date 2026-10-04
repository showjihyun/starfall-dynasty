// Hand-written. C3 (p1-02-mining). "A가 0.35초 먼저 발견했다" / "A가 한발 먼저 발견했다" -
// design doc section 3.1/3.4 (S-2/S-2b). This is presentation, not a new fact (design doc
// section 3.4: "개인 첫 획득 표시와 '0.35초 먼저'는 역사 기록이 아니다. 클라이언트가 이미 받은
// Historical Event(발견 tick·발견자)와 자기 채굴 결과(tick)로 계산하는 표현이다").
//
// Inputs, per 02_client_ack.md C3 section: this session's own extraction tick comes from its
// COMMAND_RESULT (accepted) envelope tick; the discoverer's tick comes from
// HISTORICAL_EVENT_NOTICE.historical_event.tick (the decisive source domain event's tick, HSE
// envelope). tick_hz = 20 (ADR-0006: 1 tick = 50 ms) is a fixed constant here, not read from
// data/ - this slice never varies it.

using System;
using System.Globalization;

namespace Starfall.Mining
{
    public static class DiscoveryTiming
    {
        public const int TickHz = 20;

        /// <summary>"A가 0.35초 먼저 발견했다." when the two ticks differ, or "A가 한발 먼저
        /// 발견했다." when they are exactly equal (design doc: never write "0.00초 먼저" for a
        /// tie - S-2's same-tick case). <paramref name="discovererPilotTag"/> is a display tag
        /// only (see PilotTag.cs); the ticks are the only inputs, in a fixed order so a caller
        /// cannot accidentally swap them ("나" 관점이 아니라 "발견자" 관점의 문장이다).</summary>
        public static string Describe(string discovererPilotTag, long discovererTick, long myTick)
        {
            if (discovererPilotTag == null) throw new ArgumentNullException(nameof(discovererPilotTag));

            long deltaTicks = Math.Abs(myTick - discovererTick);
            if (deltaTicks == 0)
                return discovererPilotTag + "가 한발 먼저 발견했다.";

            double seconds = deltaTicks / (double)TickHz;
            return discovererPilotTag + "가 " + seconds.ToString("0.00", CultureInfo.InvariantCulture) + "초 먼저 발견했다.";
        }
    }
}
