// Hand-written. SC-64/65 (sprint contract section 0.11 / section I; client ack issue 6,
// "채택하는 형태"): boots TWO independent WebSocket sessions in ONE Unity process instead of two
// Editor instances. "A" is a normal interactive observer (ShipInputSampler-driven, same as
// GreyboxSession); "B" is a second, non-interactive observer that only watches A. Because
// Starfall.Sim/Flight/Remote have no Unity types (C3/C5 directive), B's interpolation pipeline
// is exactly as real as a second Editor's would be - what a second PROCESS would additionally
// give (GC/render-timing isolation) is not what SC-64/65 measure.
//
// Why this must not be replaced by a bot (contract section 0.11, client ack issue 6): a bot has
// no interpolation, so its view of A carries the RAW wire position (server truth for that tick),
// while A is running ahead on its own prediction - the sign of the resulting gap flips (ahead by
// 0-7 m instead of behind by 28 m) and SC-65's "ahead means over-extrapolation" scoring rule
// would read normal behaviour as a bug. B here is a real interpolation pipeline, so the sign
// comes out right.
//
// Opt-in, mirroring GreyboxSession.AutoBuildVariable / StarfallNetHost.AutoConnectVariable: a
// normal single-observer SC-59 human-observation session is unaffected unless this is set.

using System;
using System.IO;
using Starfall.Net;
using UnityEngine;

namespace Starfall.Greybox
{
    public static class TwoSessionHarness
    {
        /// <summary>Set to 1 to boot both observers at play mode start.</summary>
        public const string AutoBuildVariable = "STARFALL_TWO_SESSION_AUTOBUILD";

        /// <summary>Overrides observer A's CSV path. Default: DefaultCsvDirectory/observer-a.csv.</summary>
        public const string CsvPathAVariable = "STARFALL_OBSERVER_A_CSV";

        /// <summary>Overrides observer B's CSV path. Default: DefaultCsvDirectory/observer-b.csv.</summary>
        public const string CsvPathBVariable = "STARFALL_OBSERVER_B_CSV";

        /// <summary>Repo-root-relative default output directory - tests/e2e/two_client_view.py
        /// takes explicit --a/--b paths, so this is a convenience default, not a contract.</summary>
        public const string DefaultCsvDirectory = "_workspace/p1-01-ship-movement/two-session";

        public static ObserverSession ObserverA { get; private set; }
        public static ObserverSession ObserverB { get; private set; }

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void AutoBuildIfRequested()
        {
            if (Environment.GetEnvironmentVariable(AutoBuildVariable) != "1") return;
            Build();
        }

        /// <summary>Idempotent - a second call after the pair already exists is a no-op, same
        /// discipline as GreyboxSession.Ensure()/StarfallNetHost.Ensure().</summary>
        public static void Build()
        {
            if (ObserverA != null && ObserverB != null) return;

            string csvDir = ResolveCsvDirectory();
            string csvPathA = ResolvePath(CsvPathAVariable, Path.Combine(csvDir, "observer-a.csv"));
            string csvPathB = ResolvePath(CsvPathBVariable, Path.Combine(csvDir, "observer-b.csv"));

            string subjectA = DevAuthToken.ResolveSubject();
            string subjectB = DevAuthToken.SecondObserverSubject;

            Debug.Log("starfall.two_session: booting A(subject=" + subjectA + ", csv=" + csvPathA +
                      ") and B(subject=" + subjectB + ", csv=" + csvPathB + ")");

            ObserverA = ObserverSession.Create("A", subjectA, csvPathA, isInteractive: true);
            ObserverB = ObserverSession.Create("B", subjectB, csvPathB, isInteractive: false);
        }

        static string ResolvePath(string envVariable, string fallback)
        {
            string overridden = Environment.GetEnvironmentVariable(envVariable);
            return string.IsNullOrEmpty(overridden) ? fallback : overridden;
        }

        static string ResolveCsvDirectory()
        {
            string repoRoot = FindRepoRootFromDataPath();
            return repoRoot != null
                ? Path.Combine(repoRoot, DefaultCsvDirectory.Replace('/', Path.DirectorySeparatorChar))
                : Path.Combine(Application.persistentDataPath, "two-session");
        }

        static string FindRepoRootFromDataPath()
        {
            var dir = new DirectoryInfo(Application.dataPath);
            while (dir != null)
            {
                if (File.Exists(Path.Combine(dir.FullName, "contracts", "registry", "types.json"))) return dir.FullName;
                dir = dir.Parent;
            }
            return null;
        }
    }
}
