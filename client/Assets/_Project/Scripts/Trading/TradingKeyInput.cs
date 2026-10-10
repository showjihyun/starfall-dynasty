// Hand-written. C3 (p1-03-trading). The five booleans GreyboxTradingSession.Update() needs per
// frame (AC-17: UpArrow/DownArrow select, LeftArrow/RightArrow preset, Enter sell). A plain
// value type with no UnityEngine.InputSystem dependency - Starfall.Trading has
// noEngineReferences: true - so PlayMode tests can inject an arbitrary frame's key state through
// a Func<TradingKeyInput> delegate (same pattern as the ship-state/discoveries/inventory
// providers GreyboxTradingSession.Init() already takes) instead of needing a live Keyboard
// device or the InputSystem test-tools package (not referenced by this project, qa3 Q-4/SC-104:
// "Keyboard 장치가 없어도 된다... 테스트용으로 주입하면").

namespace Starfall.Trading
{
    public readonly struct TradingKeyInput
    {
        public readonly bool UpPressed;
        public readonly bool DownPressed;
        public readonly bool LeftPressed;
        public readonly bool RightPressed;
        public readonly bool EnterPressed;

        public TradingKeyInput(bool upPressed, bool downPressed, bool leftPressed, bool rightPressed, bool enterPressed)
        {
            UpPressed = upPressed;
            DownPressed = downPressed;
            LeftPressed = leftPressed;
            RightPressed = rightPressed;
            EnterPressed = enterPressed;
        }

        public static readonly TradingKeyInput None = new TradingKeyInput(false, false, false, false, false);
    }
}
