// EditMode tests for AutoPlayDecision (team-lead request 2026-10-09, relaying the user's "Play
// 버튼도 니가 알아서 눌러서 진행해줘"). Pure string/bool logic only - never touches
// EditorApplication.EnterPlaymode() or SessionState (those live in the Editor-only
// AutoPlayOnLoad.cs, not covered by an EditMode test since actually entering Play mode is the
// one thing team-lead explicitly deferred to after this pre-session ends).

using NUnit.Framework;
using Starfall.Greybox;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class AutoPlayDecisionTests
    {
        [Test]
        public void IsEnabled_OnlyExactly1_EverythingElseFalse()
        {
            Assert.That(AutoPlayDecision.IsEnabled("1"), Is.True);

            Assert.That(AutoPlayDecision.IsEnabled(null), Is.False, "unset must be false");
            Assert.That(AutoPlayDecision.IsEnabled(""), Is.False);
            Assert.That(AutoPlayDecision.IsEnabled("0"), Is.False);
            Assert.That(AutoPlayDecision.IsEnabled("true"), Is.False, "only the literal \"1\" counts - not \"true\"");
            Assert.That(AutoPlayDecision.IsEnabled("TRUE"), Is.False);
            Assert.That(AutoPlayDecision.IsEnabled("01"), Is.False, "not a numeric parse - exact string match only");
            Assert.That(AutoPlayDecision.IsEnabled(" 1"), Is.False, "no implicit trimming - a stray space must not count");
        }

        [Test]
        public void ShouldEnterPlayMode_EnabledAndNotYetEntered_IsTheOnlyTrueCase()
        {
            // The four (enabled, alreadyEntered) combinations, each asserted - no combination
            // left to "probably works the same way".
            Assert.That(AutoPlayDecision.ShouldEnterPlayMode("1", alreadyEnteredThisSession: false), Is.True);
            Assert.That(AutoPlayDecision.ShouldEnterPlayMode("1", alreadyEnteredThisSession: true), Is.False,
                "already entered once this session - must not enter again (team-lead: 다시 들어가지 않게)");
            Assert.That(AutoPlayDecision.ShouldEnterPlayMode(null, alreadyEnteredThisSession: false), Is.False,
                "not enabled at all - must not enter even on a fresh session");
            Assert.That(AutoPlayDecision.ShouldEnterPlayMode(null, alreadyEnteredThisSession: true), Is.False);
        }

        [Test]
        public void ShouldAttemptNow_GatesOnEditorBusy_OnTopOfShouldEnterPlayMode()
        {
            // qa3 r1 live-fire finding: a single delayCall right after Editor launch can fire
            // while the Editor is still importing/compiling, and EnterPlaymode() called in that
            // window is silently ignored. ShouldAttemptNow adds that gate.
            Assert.That(AutoPlayDecision.ShouldAttemptNow("1", alreadyEnteredThisSession: false, editorBusy: false), Is.True);
            Assert.That(AutoPlayDecision.ShouldAttemptNow("1", alreadyEnteredThisSession: false, editorBusy: true), Is.False,
                "Editor still compiling/updating - must not attempt yet, but must not burn the one-shot flag either");
            Assert.That(AutoPlayDecision.ShouldAttemptNow("1", alreadyEnteredThisSession: true, editorBusy: false), Is.False,
                "already entered once - editorBusy being false must not override that");
            Assert.That(AutoPlayDecision.ShouldAttemptNow(null, alreadyEnteredThisSession: false, editorBusy: false), Is.False,
                "not enabled - editorBusy being false must not override that either");
        }

        [Test]
        public void Constants_AreNonEmptyAndDistinct()
        {
            // Not "the constants have some literal value" (that is a tautology for a const
            // field) - this asserts the two keys this feature depends on are non-empty and
            // distinct from each other, which a copy-paste mistake (reusing one name for both)
            // would violate.
            Assert.That(AutoPlayDecision.EnvVarName, Is.Not.Null.And.Not.Empty);
            Assert.That(AutoPlayDecision.SessionStateKey, Is.Not.Null.And.Not.Empty);
            Assert.That(AutoPlayDecision.EnvVarName, Is.Not.EqualTo(AutoPlayDecision.SessionStateKey));
        }
    }
}
