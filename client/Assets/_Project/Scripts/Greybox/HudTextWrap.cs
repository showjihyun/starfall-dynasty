// F-1 fix (05_qa_report_r5.md 6.1, round R5): GreyboxSession.OnGUI() used to draw every HUD
// line in a single fixed Rect (width 400px, height 18px). Unity's default GUI.Label does not
// wrap and does not ellipsize when a line's rendered text is wider than its Rect - the
// overflowing part is simply never drawn, with no visual sign that anything is missing. Three
// counters (SC-56 (d)②③, SC-89's send_burst_max_sends_per_frame) sat past that point on their
// lines and were never visible in any recording (qa r5 §6.1).
//
// The fix is structural, not a wider constant: GreyboxSession no longer hands GUI.Label a raw
// line and a fixed Rect at all. It first runs every line through Wrap() below, a pure
// (no UnityEngine.GUI dependency) greedy word-wrap that always accounts for 100% of the input
// text in its output rows - by construction, not by picking a width that happens to be wide
// enough today. A field growing tomorrow makes MORE rows, never a shorter, silently-clipped one.
// The row count this returns is what GreyboxSession sizes its background Box and per-row Label
// Rects from (see GreyboxSession.cs BuildHudRows/OnGUI), so the drawn area always matches the
// content instead of the content being cropped to a pre-decided area.
//
// Kept free of UnityEngine.GUI/GUIStyle so it can be unit tested in EditMode without an OnGUI
// call stack (GUIStyle.CalcSize/CalcHeight require one; GUIStyle itself does not, but relying on
// it would make this file's only test coverage conditional on Unity's text metrics instead of on
// the wrap algorithm itself).

using System.Collections.Generic;
using System.Text;

namespace Starfall.Greybox
{
    public static class HudTextWrap
    {
        /// <summary>Greedy word-wrap. Splits <paramref name="text"/> on spaces into rows no
        /// longer than <paramref name="maxCharsPerLine"/> characters. A single token longer than
        /// the limit gets its own row, unsplit - never dropped, only possibly wide (a rendering
        /// nicety, not a data-loss bug). Always returns at least one row, and joining the
        /// returned rows with single spaces reproduces <paramref name="text"/> exactly (modulo
        /// the original run of separating spaces, which single-space normalises) - that identity
        /// is what guarantees no field is silently lost.</summary>
        public static List<string> Wrap(string text, int maxCharsPerLine)
        {
            var rows = new List<string>();

            if (string.IsNullOrEmpty(text))
            {
                rows.Add(text ?? string.Empty);
                return rows;
            }

            if (maxCharsPerLine <= 0)
            {
                rows.Add(text);
                return rows;
            }

            string[] words = text.Split(' ');
            var current = new StringBuilder();

            foreach (string word in words)
            {
                if (current.Length == 0)
                {
                    current.Append(word);
                }
                else if (current.Length + 1 + word.Length <= maxCharsPerLine)
                {
                    current.Append(' ').Append(word);
                }
                else
                {
                    rows.Add(current.ToString());
                    current.Clear();
                    current.Append(word);
                }
            }

            if (current.Length > 0 || rows.Count == 0) rows.Add(current.ToString());
            return rows;
        }
    }
}
