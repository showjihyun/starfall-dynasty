// Hand-written. Raw player input -> normalized flight intent (design section 3.4's key
// layout). This is the ONLY file in the greybox layer that reads a device directly.
//
// The project has activeInputHandler = 1 (Input System package only, legacy UnityEngine.Input
// is disabled project-wide - client/ProjectSettings/ProjectSettings.asset), so this polls
// Mouse.current / Keyboard.current directly. No .inputactions asset: polling avoids adding an
// asset file for four keys and a mouse delta, and this is a diagnostic greybox controller, not
// rebindable game-facing input.
//
// This class produces intent, not quantized wire values - SetShipControlBuilder.Build quantizes
// it (Starfall.Flight), and ToDequantizedInput re-derives the prediction input from THAT output
// (I-36). This class must never be asked to predict from what it read here directly.

using Starfall.Sim;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Starfall.Greybox
{
    public sealed class ShipInputSampler : MonoBehaviour
    {
        [Tooltip("Degrees of aim rotation per pixel of mouse movement.")]
        public double MouseSensitivityDegPerPixel = 0.12;

        [Tooltip("Maximum pitch, degrees, to keep the aim target from flipping past straight up/down.")]
        public double MaxPitchDeg = 89.0;

        double _yawDeg;
        double _pitchDeg;
        bool _flightAssist = true; // default true (design section 3.4 / SET_SHIP_CONTROL default)
        bool _assistTogglePrevFrame;

        /// <summary>Local-axis thrust intent, each component in [-1, 1]. +X ship-right,
        /// +Y ship-up, +Z ship-forward (ADR-0009 section 1).</summary>
        public Vec3d Thrust { get; private set; }

        /// <summary>Manual roll intent, [-1, 1].</summary>
        public double Roll { get; private set; }

        /// <summary>Accumulated world-space look target. Only its facing (and the server's own
        /// roll-axis authority split, ADR-0010 section 2 step 2) matters - the up/roll component
        /// this produces is not authoritative and is intentionally ignored server-side.</summary>
        public Quatd AimTargetWorld { get; private set; } = Quatd.Identity;

        public bool Brake { get; private set; }
        public bool FlightAssist => _flightAssist;

        void Awake()
        {
            // Start aiming along the ship's own spawn-time forward so the first frame does not
            // snap the nose somewhere unexpected before the player has moved the mouse.
            _yawDeg = 0.0;
            _pitchDeg = 0.0;
        }

        public void Sample()
        {
            Mouse mouse = Mouse.current;
            Keyboard keyboard = Keyboard.current;

            if (mouse != null)
            {
                Vector2 delta = mouse.delta.ReadValue();
                _yawDeg += delta.x * MouseSensitivityDegPerPixel;
                _pitchDeg -= delta.y * MouseSensitivityDegPerPixel;
                if (_pitchDeg > MaxPitchDeg) _pitchDeg = MaxPitchDeg;
                if (_pitchDeg < -MaxPitchDeg) _pitchDeg = -MaxPitchDeg;
                // Yaw wraps freely - it is an angle, not a position; no reason to clamp it.
            }

            AimTargetWorld = YawPitchToQuaternion(_yawDeg, _pitchDeg);

            double thrustZ = 0.0, thrustX = 0.0, thrustY = 0.0, roll = 0.0;
            bool brake = false;
            bool assistTogglePressed = false;

            if (keyboard != null)
            {
                if (keyboard.wKey.isPressed) thrustZ += 1.0;
                if (keyboard.sKey.isPressed) thrustZ -= 1.0;
                if (keyboard.dKey.isPressed) thrustX += 1.0;
                if (keyboard.aKey.isPressed) thrustX -= 1.0;
                if (keyboard.rKey.isPressed) thrustY += 1.0;
                if (keyboard.fKey.isPressed) thrustY -= 1.0;
                if (keyboard.eKey.isPressed) roll += 1.0;
                if (keyboard.qKey.isPressed) roll -= 1.0;
                brake = keyboard.xKey.isPressed;
                assistTogglePressed = keyboard.zKey.isPressed;
            }

            // Toggle on the rising edge only - holding Z must not rapid-fire the toggle.
            if (assistTogglePressed && !_assistTogglePrevFrame) _flightAssist = !_flightAssist;
            _assistTogglePrevFrame = assistTogglePressed;

            Thrust = new Vec3d(thrustX, thrustY, thrustZ);
            Roll = roll;
            Brake = brake;
        }

        static Quatd YawPitchToQuaternion(double yawDeg, double pitchDeg)
        {
            // Yaw about world +Y, then pitch about the yawed local +X - standard look-rotation
            // composition. Uses Math.Sin/Cos deliberately: this builds an INTENT to send, not a
            // simulation state, so it is outside ADR-0010 section 3's transcendental-function
            // ban (that ban is scoped to state advancement in Starfall.Sim.ShipIntegrator).
            double halfYaw = yawDeg * 0.5 * (System.Math.PI / 180.0);
            var qYaw = new Quatd(0.0, System.Math.Sin(halfYaw), 0.0, System.Math.Cos(halfYaw));

            double halfPitch = pitchDeg * 0.5 * (System.Math.PI / 180.0);
            var qPitch = new Quatd(System.Math.Sin(halfPitch), 0.0, 0.0, System.Math.Cos(halfPitch));

            return (qYaw * qPitch).Normalized();
        }
    }
}
