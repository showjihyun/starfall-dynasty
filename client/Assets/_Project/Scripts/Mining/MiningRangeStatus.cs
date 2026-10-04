// Hand-written. C4 (p1-02-mining, SC-68 재시도). Design doc §8 item 2: "채굴 가능 표시" - 가장
// 가까운 광맥까지 표면 거리, 사거리 안인가, 속도가 10 m/s 이하인가. 셋 다 초록일 때만 채굴 키가
// 켜진 것처럼 보인다(판정은 여전히 서버) - 이 파일은 그 세 값을 계산하는 순수 함수다.
//
// 표면 거리 정의는 서버(server/crates/sim/src/mining.rs: within_mining_range)와 같아야 한다:
// 서버는 |ship-deposit|^2 <= (radius_m + range_m)^2 (제곱 비교, sqrt 없음, ADR-0010 §3)를 쓴다.
// 이 파일은 표시용이라 sqrt를 쓴다 - surface_distance_m = |ship-deposit| - radius_m이고,
// in_range = surface_distance_m <= range_m 은 sqrt가 단조증가 함수이므로 서버의 제곱 비교와
// 정확히 같은 경계에서 참/거짓이 갈린다(서버는 상태를 이걸로 "판정"하지 않는다 - 그 판정은
// 여전히 서버의 MINE_RESOURCE 처리뿐, 이 파일은 화면 힌트만 만든다).

using System;

namespace Starfall.Mining
{
    /// <summary>Design doc §8 item 2의 세 신호. 셋 다 켜져야(<see cref="ReadyToMine"/>) 채굴
    /// 키 안내가 "켜진 것처럼" 보인다 - 실제 서버 판정을 막지 않는다(principle 1).</summary>
    public readonly struct MiningRangeStatus
    {
        public readonly double SurfaceDistanceM;
        public readonly bool InRange;
        public readonly bool SpeedOk;

        public MiningRangeStatus(double surfaceDistanceM, bool inRange, bool speedOk)
        {
            SurfaceDistanceM = surfaceDistanceM;
            InRange = inRange;
            SpeedOk = speedOk;
        }

        public bool ReadyToMine => InRange && SpeedOk;
    }

    public static class MiningRangeEvaluator
    {
        /// <summary>표면 거리·사거리 안·속도 판정. <paramref name="distanceToCenterM"/>은 이미
        /// 계산된 중심 거리(sqrt 적용 완료) - 호출자(Vec3d 거리)가 넘긴다.</summary>
        public static MiningRangeStatus Evaluate(
            double distanceToCenterM, double depositRadiusM, double shipSpeedMps,
            double rangeFromSurfaceM, double maxSpeedMps)
        {
            double surfaceDistanceM = distanceToCenterM - depositRadiusM;
            bool inRange = surfaceDistanceM <= rangeFromSurfaceM;
            bool speedOk = shipSpeedMps <= maxSpeedMps;
            return new MiningRangeStatus(surfaceDistanceM, inRange, speedOk);
        }

        /// <summary>디자인 §8 item 2의 세 줄, 초록/빨강 표시는 "[OK]"/"[NG]" 접두로(그레이박스는
        /// 색 텍스트가 아니라 평문 - GreyboxMiningSession.OnGUI가 색을 입히는 건 이 접두를 보고
        /// 한다). 순서는 표면 거리 → 사거리 → 속도(디자인 §8 순서 그대로).</summary>
        public static string[] FormatStatusLines(MiningRangeStatus status, double shipSpeedMps)
        {
            return new[]
            {
                "표면 거리 " + DistanceLabel.Format(status.SurfaceDistanceM),
                (status.InRange ? "[OK] " : "[NG] ") + "사거리 안",
                (status.SpeedOk ? "[OK] " : "[NG] ") + "속도 " + shipSpeedMps.ToString("F1", System.Globalization.CultureInfo.InvariantCulture) + " m/s",
            };
        }
    }
}
