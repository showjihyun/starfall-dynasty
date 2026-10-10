// Hand-written. Editor-only. team-lead request (2026-10-09, relaying the user: "다음부턴 Play
// 버튼도 니가 알아서 눌러서 진행해줘") - a headless agent cannot click Unity's Play button, so
// this script clicks it for the agent, but ONLY when explicitly asked to (STARFALL_AUTOPLAY=1)
// so it never fires during an ordinary human Editor session or a CI script-compile check that
// never meant to enter Play mode.
//
// qa3's FIRST live-fire finding (2026-10-09, autoplay-check round 1): the original design (one
// delayCall after InitializeOnLoad, set the SessionState flag THEN call EnterPlaymode) never
// entered Play mode. Fixed in round 2 by polling EditorApplication.update instead of a single
// delayCall, gating on !isCompiling && !isUpdating, and confirming
// isPlayingOrWillChangePlaymode immediately after the call before persisting the flag.
//
// qa3's SECOND live-fire finding (round 2, same symptom: Editor loaded in 9s, never entered
// Play, open scene was "Untitled") - team-lead's diagnosis of what round 2 still missed:
//   (1) nothing in the log proved [InitializeOnLoad]'s static constructor had even RUN at all -
//       the round-1 evidence only showed the compiled DLL CONTAINED the calls, never that they
//       executed. This revision logs a line from the static constructor itself, before anything
//       else, so the next run can tell "never loaded/never ran" apart from "ran but the attempt
//       logic didn't fire" from "attempted but EnterPlaymode was refused" purely by reading
//       unity_editor.log.
//   (2) EnterPlaymode() plays whatever scene happens to be OPEN in the Editor at that moment -
//       and a batchmode/-nographics launch with no scene explicitly loaded opens a blank
//       "Untitled" scene, not this slice's grey-box scene (Assets/Scenes/SampleScene.unity).
//       Nothing was ever wrong with GreyboxSession.AutoBuildIfRequested's own logic (it builds
//       everything in code, scene-independent - confirmed by reading it, round 2) - the scene
//       being open was never a BLOCKER for the grey-box build, but it IS the scene a human
//       looking at the Play session would see, and more importantly there is no guarantee an
//       "Untitled" scene even has the GameObjects GreyboxSession.Awake() expects to find (camera
//       rig, etc. depending on what else lives in SampleScene.unity vs. a truly blank scene).
//       Fixed by setting EditorSceneManager.playModeStartScene to SampleScene.unity's SceneAsset
//       right before the EnterPlaymode() call - the documented mechanism for "which scene Play
//       mode starts from" that does NOT disturb whatever scene is open for editing.
//
// qa3 confirmed rounds 1-2 fixed the actual Play entry (live-fire: env=1, attemptNow=True,
// playModeStartScene set, "EnterPlaymode accepted" logged once, live_connections reached 1 in
// 15.7s). team-lead's round-3 request (2026-10-09), independent of that result: the per-tick
// Debug.Log in TryEnterPlayMode floods unity_editor.log while EditorApplication.update polls
// every frame. Fixed by only logging when the logged state actually changes (_lastLoggedSignature
// below) - once per real transition, not once per frame.

using System;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

namespace Starfall.Greybox.Editor
{
    [InitializeOnLoad]
    static class AutoPlayOnLoad
    {
        /// <summary>team-lead request (round 3): a log line every single
        /// EditorApplication.update tick floods unity_editor.log. This remembers the last
        /// logged (env, alreadyEntered, isCompiling, isUpdating, attemptNow) tuple as a plain
        /// string and only logs again when it actually CHANGES - so a long compile/import
        /// stretch where nothing changes produces exactly one log line, not one per frame.</summary>
        static string _lastLoggedSignature;

        /// <summary>Project-relative path to this slice's grey-box scene - the one
        /// EditorSceneManager.playModeStartScene is pointed at before EnterPlaymode(), so Play
        /// mode starts from a known scene regardless of whatever happens to be open for editing
        /// (team-lead finding (2): a batchmode launch with no scene loaded opens a blank
        /// "Untitled" scene otherwise).</summary>
        const string GreyboxScenePath = "Assets/Scenes/SampleScene.unity";

        static AutoPlayOnLoad()
        {
            // team-lead finding (1): prove the static constructor itself ran, BEFORE anything
            // else - a log line here is the one unambiguous signal that [InitializeOnLoad]
            // actually fired this Editor session, independent of env var state, asmdef platform
            // inclusion, or anything TryEnterPlayMode does afterward.
            Debug.Log("starfall.autoplay: AutoPlayOnLoad static constructor ran - subscribing to EditorApplication.update");
            EditorApplication.update += TryEnterPlayMode;
        }

        static void TryEnterPlayMode()
        {
            string envVarValue = Environment.GetEnvironmentVariable(AutoPlayDecision.EnvVarName);
            bool alreadyEntered = SessionState.GetBool(AutoPlayDecision.SessionStateKey, false);

            if (!AutoPlayDecision.ShouldEnterPlayMode(envVarValue, alreadyEntered))
            {
                // Disabled, or already entered once this session - nothing left to retry.
                EditorApplication.update -= TryEnterPlayMode;
                return;
            }

            bool editorBusy = EditorApplication.isCompiling || EditorApplication.isUpdating;
            bool attemptNow = AutoPlayDecision.ShouldAttemptNow(envVarValue, alreadyEntered, editorBusy);

            string signature = "env=" + (envVarValue ?? "(null)") + " alreadyEntered=" + alreadyEntered +
                                " isCompiling=" + EditorApplication.isCompiling + " isUpdating=" + EditorApplication.isUpdating +
                                " attemptNow=" + attemptNow;
            if (signature != _lastLoggedSignature)
            {
                Debug.Log("starfall.autoplay: " + signature);
                _lastLoggedSignature = signature;
            }

            if (!attemptNow) return; // Editor still settling - retry next tick, no call made yet.

            SetPlayModeStartScene();
            EditorApplication.EnterPlaymode();

            if (EditorApplication.isPlayingOrWillChangePlaymode)
            {
                // Confirmed the call took (set AFTER confirming, not before - round 1's fix: the
                // original "set first" ordering meant a silently-ignored call still burned the
                // one-shot flag with nothing to show for it).
                SessionState.SetBool(AutoPlayDecision.SessionStateKey, true);
                EditorApplication.update -= TryEnterPlayMode;
                Debug.Log("starfall.autoplay: EnterPlaymode accepted - flag set, stopped polling");
            }
            else
            {
                Debug.Log("starfall.autoplay: EnterPlaymode call did not take - will retry next tick");
            }
        }

        /// <summary>Points Play mode at the grey-box scene without touching whatever scene is
        /// currently open for editing (EditorSceneManager.playModeStartScene's documented
        /// purpose). Logs and returns without throwing if the scene asset cannot be found - a
        /// missing/renamed scene should not crash the Editor's update loop, just fall back to
        /// Unity's default "play the open scene" behaviour.</summary>
        static void SetPlayModeStartScene()
        {
            var sceneAsset = AssetDatabase.LoadAssetAtPath<SceneAsset>(GreyboxScenePath);
            if (sceneAsset == null)
            {
                Debug.LogWarning("starfall.autoplay: could not load scene asset at " + GreyboxScenePath +
                                  " - EnterPlaymode will play whatever scene is currently open instead");
                return;
            }
            EditorSceneManager.playModeStartScene = sceneAsset;
            Debug.Log("starfall.autoplay: playModeStartScene set to " + GreyboxScenePath);
        }
    }
}
