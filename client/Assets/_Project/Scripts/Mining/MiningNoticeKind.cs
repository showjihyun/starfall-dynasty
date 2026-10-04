// Hand-written. C4 (p1-02-mining, SC-68 재시도). Design doc §8 item 5: "알림 두 종: 산출
// 알림(작게), 발견 배너(크게, 다른 색·위치). 성계 전체 배너(다른 사람의 발견)." SC-68 1차 FAIL이
// 지적한 것: 지금은 산출 알림과 발견 배너가 HUD 목록에 "*** ... ***" 한 줄로 섞여 있어 다른
// 것으로 안 읽힌다. 이 파일은 그 세 가지(산출/내 발견/남의 발견)를 분류하는 순수 로직이다 -
// 실제 큰 글씨·색·위치는 GreyboxMiningSession.OnGUI가 그린다(UnityEngine.GUI는 여기서 안 쓴다,
// Starfall.Mining은 noEngineReferences).

using System;

namespace Starfall.Mining
{
    public enum MiningNoticeKind
    {
        /// <summary>산출 알림 - 작게, 짧게. 내 MINE_RESOURCE의 성공/거절, INVENTORY_STATE 반영.</summary>
        Yield,

        /// <summary>내가 처음 발견한 광물의 발견 배너 - 크게, 다른 위치·문구.</summary>
        OwnDiscovery,

        /// <summary>다른 조종사가 발견한 것을 알리는 성계 전체 배너 - 크게, 내 발견과는 다른
        /// 문구로 구별된다(design doc §3.4 B·C: "A가 처음으로 발견했다").</summary>
        SystemWideDiscovery,
    }

    public static class MiningNoticeClassifier
    {
        /// <summary>발견자 actor_id가 이 세션의 actor_id와 같으면 내 발견, 아니면 성계 전체
        /// 배너. HISTORICAL_EVENT_NOTICE 자체에는 "이게 나다"라는 필드가 없어 클라이언트가
        /// RealtimeClient.ActorId와 비교해 분류한다 - 새 사실을 만드는 게 아니라 이미 받은 사실을
        /// 표현만 나누는 것(design doc §3.4 마지막 문단, principle 1/2와 충돌 없음).</summary>
        public static MiningNoticeKind ClassifyDiscovery(Guid discovererActorId, Guid selfActorId) =>
            discovererActorId == selfActorId ? MiningNoticeKind.OwnDiscovery : MiningNoticeKind.SystemWideDiscovery;

        /// <summary>배너 문구. 내 발견은 "역사적 발견 — ...이 기록은 남는다"(design doc §3.3),
        /// 남의 발견은 "{tag}가 ...에서 처음으로 ...를 발견했다"(design doc §3.4 표 B행) - 같은
        /// 사건이라도 누가 보느냐에 따라 다른 문장이어야 "내 발견"과 "남의 발견"이 안 섞인다.</summary>
        public static string FormatBanner(MiningNoticeKind kind, string mineralDisplayName, string pilotTag, string depositId)
        {
            switch (kind)
            {
                case MiningNoticeKind.OwnDiscovery:
                    return "역사적 발견 — 당신은 Cradle 성계에서 처음으로 " + mineralDisplayName +
                           "을 채굴했다. 이 기록은 남는다. (" + depositId + ")";
                case MiningNoticeKind.SystemWideDiscovery:
                    return pilotTag + "가 Cradle 성계에서 처음으로 " + mineralDisplayName +
                           "을 발견했다 — " + depositId;
                default:
                    throw new ArgumentOutOfRangeException(nameof(kind), kind, "Yield는 배너가 아니다 - FormatYieldNotice를 쓴다");
            }
        }

        /// <summary>산출 알림 문구 - 작게 표시될 짧은 문장, 배너와 어휘가 겹치지 않게
        /// "발견"이라는 단어를 쓰지 않는다(섞여 보이지 않기 위한 의도적 선택).</summary>
        public static string FormatYieldNotice(string mineralDisplayName, int quantityKg) =>
            "+" + quantityKg + " kg " + mineralDisplayName;
    }
}
