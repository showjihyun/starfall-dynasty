// EditMode test for C3 (p1-03-trading), SC-105 (AC-17): the new trading keys (UpArrow/DownArrow/
// LeftArrow/RightArrow/Enter) must not collide with any EXISTING binding. "Existing bindings"
// is collected from source (regex scan over the three files that define them), not a hardcoded
// count - qa3 Q-4: a hardcoded expected number would pass for free if the existing list were
// ever emptied by accident (the same "0 == 0" failure mode CLAUDE.md's verification discipline
// warns about).

using System;
using System.Collections.Generic;
using System.IO;
using System.Text.RegularExpressions;
using NUnit.Framework;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class TradingKeyTests
    {
        // ------------------------------------------------------------------ SC-105

        /// <summary>
        /// Collects the existing key-binding tokens straight from the three files that define
        /// them, via three different source patterns (each file writes its bindings a different
        /// way - a scan that only recognised one pattern would silently miss the other two):
        /// ShipInputSampler.cs's keyboard.&lt;x&gt;Key checks (W/S/A/D/R/F/Q/E/X/Z),
        /// GreyboxSession.cs's hKey.wasPressedThisFrame (H), and GreyboxMiningSession.cs's
        /// "const Key MineKey = Key.G" (G) - 12 distinct keys as of this slice (design doc
        /// section 8.1).
        /// </summary>
        static HashSet<string> CollectExistingBindingKeys(string repoRoot)
        {
            var keys = new HashSet<string>(StringComparer.Ordinal);

            string shipInputText = File.ReadAllText(Path.Combine(repoRoot, "client", "Assets", "_Project", "Scripts", "Greybox", "ShipInputSampler.cs"));
            foreach (Match m in Regex.Matches(shipInputText, @"keyboard\.(\w)Key\b"))
                keys.Add(m.Groups[1].Value.ToUpperInvariant());

            string greyboxSessionText = File.ReadAllText(Path.Combine(repoRoot, "client", "Assets", "_Project", "Scripts", "Greybox", "GreyboxSession.cs"));
            foreach (Match m in Regex.Matches(greyboxSessionText, @"\.(\w)Key\.wasPressedThisFrame"))
                keys.Add(m.Groups[1].Value.ToUpperInvariant());

            string miningSessionText = File.ReadAllText(Path.Combine(repoRoot, "client", "Assets", "_Project", "Scripts", "Greybox", "GreyboxMiningSession.cs"));
            foreach (Match m in Regex.Matches(miningSessionText, @"Key\s+\w+\s*=\s*Key\.(\w+)\s*;"))
                keys.Add(m.Groups[1].Value.ToUpperInvariant());

            return keys;
        }

        /// <summary>The new trading keys, collected from GreyboxTradingSession.cs's own source
        /// the same way - not hardcoded as a literal array either, so a future rename of the
        /// scheme (e.g. WASD-style instead of arrow keys) is caught here automatically.</summary>
        static HashSet<string> CollectTradingBindingKeys(string repoRoot)
        {
            string tradingSessionText = File.ReadAllText(Path.Combine(repoRoot, "client", "Assets", "_Project", "Scripts", "Greybox", "GreyboxTradingSession.cs"));
            var keys = new HashSet<string>(StringComparer.Ordinal);
            // Every Keyboard.current.<name>Key property ends in literal "Key" ("upArrowKey",
            // "enterKey", ...) - capture everything before it, then strip a trailing "Arrow" so
            // "upArrow" -> "UP" (matches the direction names used elsewhere in this project).
            foreach (Match m in Regex.Matches(tradingSessionText, @"Keyboard\.current\.(\w+)Key\b"))
            {
                string name = m.Groups[1].Value;
                if (name.EndsWith("Arrow", StringComparison.Ordinal)) name = name.Substring(0, name.Length - "Arrow".Length);
                keys.Add(name.ToUpperInvariant());
            }
            return keys;
        }

        [Test]
        public void TradingKeys_NoOverlapWithExistingBindings()
        {
            string repoRoot = ContractFixtures.RequireRepoRoot();

            HashSet<string> existing = CollectExistingBindingKeys(repoRoot);
            foreach (string key in existing) TestContext.WriteLine("existing binding: " + key);
            TestContext.WriteLine("existing bindings collected from source: " + existing.Count);

            // MUTATION-sensitive positive control: the collected set must not be empty (an empty
            // set would make the overlap check below pass vacuously, 0 == 0).
            Assert.That(existing.Count, Is.GreaterThan(0), "the existing-bindings scan found nothing - a regex or file path broke silently");
            Assert.That(existing.Count, Is.EqualTo(12),
                "design doc section 8.1 lists 12 existing bindings (W/S/A/D/R/F/Q/E/X/Z/H/G) - " +
                "collected from source, not hardcoded as the expected VALUE, only as the comparison constant.");

            HashSet<string> tradingKeys = CollectTradingBindingKeys(repoRoot);
            foreach (string key in tradingKeys) TestContext.WriteLine("trading binding: " + key);
            Assert.That(tradingKeys.Count, Is.GreaterThan(0), "the trading-bindings scan found nothing");
            Assert.That(tradingKeys, Is.EquivalentTo(new[] { "UP", "DOWN", "LEFT", "RIGHT", "ENTER" }));

            var overlap = new List<string>();
            foreach (string key in tradingKeys)
                if (existing.Contains(key)) overlap.Add(key);

            TestContext.WriteLine("overlap: " + overlap.Count);
            Assert.That(overlap, Is.Empty, "the new trading keys must not collide with any existing binding");
        }
    }
}
