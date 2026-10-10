// Hand-written. team-lead request (2026-10-09, user: "다음부턴 Play 버튼도 니가 알아서 눌러서
// 진행해줘"): a headless agent cannot click the Play button, so an Editor-only script
// (Scripts/Greybox/Editor/AutoPlayOnLoad.cs) enters Play mode itself when an environment
// variable asks it to. This file holds the two decisions that script makes, as plain string/bool
// functions with zero UnityEditor dependency - so EditMode tests can assert them directly
// without touching EditorApplication.EnterPlaymode() or SessionState at all (and so this file can
// live in the ordinary Starfall.Greybox assembly, not the Editor-only one).

using System;

namespace Starfall.Greybox
{
    public static class AutoPlayDecision
    {
        /// <summary>The one environment variable that turns this on at all. Read with
        /// Environment.GetEnvironmentVariable by the Editor-only caller; passed into IsEnabled
        /// as a plain string here so "unset" (null) and "set to something else" are both
        /// expressible without needing a live environment to test against.</summary>
        public const string EnvVarName = "STARFALL_AUTOPLAY";

        /// <summary>SessionState key the Editor-only caller uses to remember "already entered
        /// Play mode once this Editor session" - survives a domain reload (SessionState's whole
        /// purpose) but not an Editor restart, which is exactly the "once per session" scope
        /// team-lead asked for ("한 번만 들어가도록").</summary>
        public const string SessionStateKey = "Starfall.AutoPlayOnLoad.Entered";

        /// <summary>True iff the raw environment variable value is EXACTLY "1" - not "true", not
        /// "TRUE", not "01". Unset (null) or any other value is false; this is deliberately the
        /// narrowest possible reading (team-lead's spec: "1이면 참, 없거나 다른 값이면 거짓"), so
        /// a typo in the value never accidentally turns this on.</summary>
        public static bool IsEnabled(string envVarValue) =>
            string.Equals(envVarValue, "1", StringComparison.Ordinal);

        /// <summary>True iff auto-play is enabled AND this Editor session has not already
        /// entered Play mode once before (team-lead: "플레이 모드에서 나갔다가 도메인 리로드가
        /// 일어나도 다시 들어가지 않게"). The caller is responsible for setting the
        /// "already entered" flag to true the moment it decides to enter - this function only
        /// decides, it never has a side effect itself (so it stays pure and callable from
        /// EditMode tests with any (envVarValue, alreadyEntered) combination).</summary>
        public static bool ShouldEnterPlayMode(string envVarValue, bool alreadyEnteredThisSession) =>
            IsEnabled(envVarValue) && !alreadyEnteredThisSession;

        /// <summary>qa3 r1 live-fire finding (2026-10-09, autoplay-check): the first
        /// EditorApplication.delayCall after a fresh Editor launch can fire while the Editor is
        /// still importing assets / finishing the initial compile, and EditorApplication.
        /// EnterPlaymode() called in that window is silently ignored (no exception, no log,
        /// isPlayingOrWillChangePlaymode never flips true). This adds the "is the Editor free to
        /// try right now" gate on top of ShouldEnterPlayMode, so the caller can poll
        /// (EditorApplication.update) and retry instead of attempting exactly once.</summary>
        public static bool ShouldAttemptNow(string envVarValue, bool alreadyEnteredThisSession, bool editorBusy) =>
            ShouldEnterPlayMode(envVarValue, alreadyEnteredThisSession) && !editorBusy;
    }
}
