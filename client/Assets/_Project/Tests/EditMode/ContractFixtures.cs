// Locates contracts/ from inside the Unity project and enumerates fixtures.
//
// This lives in the test assembly, not in Starfall.Contracts: Application.dataPath needs
// UnityEngine, and Starfall.Contracts is compiled with noEngineReferences: true.

using System;
using System.Collections.Generic;
using System.IO;
using UnityEngine;

namespace Starfall.Tests.EditMode
{
    /// <summary>
    /// Finds the repository root by walking up from Assets/ looking for the registry file,
    /// rather than counting "../.." levels. A moved project folder must make the tests fail
    /// loudly, not make them iterate over an empty set and pass.
    /// </summary>
    public static class ContractFixtures
    {
        /// <summary>The marker that identifies the repository root.</summary>
        public const string RootMarker = "contracts/registry/types.json";

        /// <summary>Valid fixtures the contract currently defines. The loader asserts against
        /// this so a silent zero-fixture run cannot be mistaken for a pass (invariant I-4).</summary>
        public const int ExpectedValidFixtureCount = 4;

        /// <summary>Repository root, or null when the marker was not found.</summary>
        public static string FindRepoRoot()
        {
            var dir = new DirectoryInfo(Application.dataPath);
            while (dir != null)
            {
                string marker = Path.Combine(dir.FullName, RootMarker.Replace('/', Path.DirectorySeparatorChar));
                if (File.Exists(marker)) return dir.FullName;
                dir = dir.Parent;
            }
            return null;
        }

        /// <summary>Repository root. Throws with an actionable message when the marker is missing.</summary>
        public static string RequireRepoRoot()
        {
            string root = FindRepoRoot();
            if (root == null)
            {
                throw new FileNotFoundException(
                    "Could not find the repository root by walking up from " + Application.dataPath +
                    " looking for " + RootMarker + ". The Unity project must sit inside the repository.");
            }
            return root;
        }

        public static string FixturesRoot() => Path.Combine(RequireRepoRoot(), "contracts", "fixtures");

        /// <summary>
        /// Valid fixtures: {TYPE}/*.json only. invalid/ is a sibling directory and must not be
        /// swept up by a recursive search — those files are counter-examples, not examples.
        /// </summary>
        public static IReadOnlyList<FixtureFile> ValidFixtures()
        {
            var result = new List<FixtureFile>();
            foreach (string typeDir in SortedDirectories(FixturesRoot()))
            {
                string typeName = Path.GetFileName(typeDir);
                foreach (string file in SortedFiles(typeDir))
                    result.Add(new FixtureFile(typeName, file));
            }
            return result;
        }

        /// <summary>Valid fixtures, with the zero-iteration guard applied.</summary>
        public static IReadOnlyList<FixtureFile> RequireValidFixtures()
        {
            IReadOnlyList<FixtureFile> fixtures = ValidFixtures();
            EnsureEnough(fixtures);
            return fixtures;
        }

        /// <summary>
        /// The guard itself, as a pure function so a test can prove it is live rather than
        /// merely present. A suite that iterates zero fixtures and reports success is
        /// indistinguishable from a suite with a dead validator (invariant I-4).
        /// </summary>
        public static void EnsureEnough(IReadOnlyList<FixtureFile> fixtures)
        {
            int count = fixtures == null ? 0 : fixtures.Count;
            if (count < ExpectedValidFixtureCount)
            {
                throw new InvalidOperationException(
                    "Expected at least " + ExpectedValidFixtureCount + " valid fixtures, found " + count +
                    ". An empty or short fixture set must fail the suite, not pass it silently.");
            }
        }

        /// <summary>
        /// Reads the UuidV7 regular expression straight out of the contract, so the generated
        /// ids are checked against the schema rather than against a copy of it.
        /// </summary>
        public static string UuidV7Pattern()
        {
            string path = Path.Combine(RequireRepoRoot(), "contracts", "common", "primitives.schema.json");
            var schema = Starfall.Contracts.ContractJson.ReadObject(File.ReadAllText(path));
            var pattern = schema["$defs"]?["UuidV7"]?["pattern"];
            if (pattern == null)
                throw new InvalidOperationException("primitives.schema.json has no $defs/UuidV7/pattern");
            return (string)pattern;
        }

        /// <summary>Counter-example fixtures: {TYPE}/invalid/*.json.</summary>
        public static IReadOnlyList<FixtureFile> InvalidFixtures()
        {
            var result = new List<FixtureFile>();
            foreach (string typeDir in SortedDirectories(FixturesRoot()))
            {
                string typeName = Path.GetFileName(typeDir);
                string invalidDir = Path.Combine(typeDir, "invalid");
                if (!Directory.Exists(invalidDir)) continue;
                foreach (string file in SortedFiles(invalidDir))
                    result.Add(new FixtureFile(typeName, file));
            }
            return result;
        }

        /// <summary>Finds one counter-example by file name, e.g. "actor-field-injected.json".</summary>
        public static FixtureFile RequireInvalid(string fileName)
        {
            foreach (FixtureFile fixture in InvalidFixtures())
                if (string.Equals(fixture.FileName, fileName, StringComparison.Ordinal))
                    return fixture;

            throw new FileNotFoundException("No counter-example fixture named " + fileName + " under " + FixturesRoot());
        }

        /// <summary>Finds one valid fixture by file name, e.g. "basic.json" under PING_SERVER.</summary>
        public static FixtureFile RequireValid(string typeName, string fileName)
        {
            foreach (FixtureFile fixture in ValidFixtures())
                if (fixture.TypeName == typeName && string.Equals(fixture.FileName, fileName, StringComparison.Ordinal))
                    return fixture;

            throw new FileNotFoundException("No valid fixture " + typeName + "/" + fileName + " under " + FixturesRoot());
        }

        static IEnumerable<string> SortedDirectories(string path)
        {
            if (!Directory.Exists(path))
                throw new DirectoryNotFoundException("Fixture directory not found: " + path);

            var dirs = new List<string>(Directory.GetDirectories(path));
            dirs.Sort(StringComparer.Ordinal);
            return dirs;
        }

        static IEnumerable<string> SortedFiles(string path)
        {
            var files = new List<string>(Directory.GetFiles(path, "*.json", SearchOption.TopDirectoryOnly));
            files.Sort(StringComparer.Ordinal);
            return files;
        }
    }

    /// <summary>One fixture file. ToString is the NUnit test-case name, so the report names
    /// the exact file that was visited.</summary>
    public sealed class FixtureFile
    {
        public FixtureFile(string typeName, string fullPath)
        {
            TypeName = typeName;
            FullPath = fullPath;
            FileName = Path.GetFileName(fullPath);
        }

        public string TypeName { get; }
        public string FullPath { get; }
        public string FileName { get; }

        public string ReadText() => File.ReadAllText(FullPath);

        public override string ToString() => TypeName + "/" + FileName;
    }
}
