// Hand-written. C4 (p1-02-mining, SC-68 재시도). 사람이 멀리서도 광맥 방향을 잡을 수 있게
// 라벨마다 거리를 붙인다(team-lead 지시) - m/km 선택의 경계만 있는 순수 포맷 함수.

using System.Globalization;

namespace Starfall.Mining
{
    public static class DistanceLabel
    {
        /// <summary>1000 m 미만은 정수 m, 1000 m 이상은 소수 둘째 자리 km. 경계(정확히
        /// 1000.0)는 km 쪽 - "1.00 km"가 "1000 m"보다 먼 거리 어림에 더 맞는 표기다.</summary>
        public static string Format(double meters)
        {
            double abs = System.Math.Abs(meters);
            if (abs < 1000.0)
                return meters.ToString("F0", CultureInfo.InvariantCulture) + " m";
            return (meters / 1000.0).ToString("F2", CultureInfo.InvariantCulture) + " km";
        }
    }
}
