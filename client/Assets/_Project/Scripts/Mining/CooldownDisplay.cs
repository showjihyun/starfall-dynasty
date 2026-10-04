// Hand-written. C4 (p1-02-mining, SC-68 재시도). Design doc §8 item 3: "쿨다운 표시(3 s)" - 기준은
// 서버 응답(수락 COMMAND_RESULT의 tick)이고, 클라이언트 추정(경과 실시간)은 표시 용도로만 쓴다
// (team-lead 지시). 이 파일은 "쿨다운 시작 이후 경과 초"를 받아 "남은 초"를 돌리는 순수 함수다 -
// 시계 자체(Time.realtimeSinceStartup)는 GreyboxMiningSession이 읽고 여기 넘긴다.

using System.Globalization;

namespace Starfall.Mining
{
    public static class CooldownDisplay
    {
        /// <summary>경과 시간이 쿨다운 길이 이상이면 0 (음수로 내려가지 않는다). 경계: 경과 ==
        /// 쿨다운이면 정확히 0(다 씀, 아직 남지 않음).</summary>
        public static double RemainingSeconds(double cooldownSeconds, double elapsedSeconds)
        {
            double remaining = cooldownSeconds - elapsedSeconds;
            return remaining > 0.0 ? remaining : 0.0;
        }

        /// <summary>남은 초가 0이면 null(화면에 쿨다운 줄 자체를 안 그린다) - 0인데 "쿨다운 0.0s"
        /// 를 계속 보여주면 "채굴 가능"과 구별이 안 된다.</summary>
        public static string FormatOrNull(double remainingSeconds)
        {
            if (remainingSeconds <= 0.0) return null;
            return "쿨다운 " + remainingSeconds.ToString("F1", CultureInfo.InvariantCulture) + "s";
        }
    }
}
