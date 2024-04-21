# OSC Avatar Manager

This is a personal project that includes a face tracking relay from ALVR to OSC (VRCFT replacement), among other things.

Pros vs VRCFT:
- Runs on Linux
- Does not randomly crash

Cons vs VRCFT:
- Fine tuning of face tracking parameters can only be done in code
- Only Quest Pro / Pico 4 Pro supported

# Basic Setup

To build, using [rustup](https://rustup.rs/) is recommended!

If you have a Quest Pro:
- In ALVR, enable `VRCFaceTracking` and `Log Tracking`.

If you use a Pico 4 Pro:
- In ALVR, enable `VRChat Eye OSC` and `Log Tracking`.

# Use with VRC

There is a helper software to handle OscQuery for us at [galister/VrcAdvert](https://github.com/galister/VrcAdvert).

```bash
wget -O VrcAdvert https://github.com/galister/VrcAdvert/releases/latest/download/VrcAdvert
# change this link to babble if using babble
wget -O oscavmgr https://github.com/galister/oscavmgr/releases/latest/download/oscavmgr-alvr
chmod +x VrcAdvert oscavmgr
```

I recommend using this start script:
```bash
#!/usr/bin/env bash

trap 'jobs -p | xargs kill' EXIT

./VrcAdvert 9402 9002 &
./oscavmgr
```

## VRC-Only: Autopilot

This activates when your HMD is in hand-tracking mode. The bottom of the terminal will change from `AP-OFF` to `MANUAL` when active.

**Turn left-right**: Look at the left/right edge of your screen\
**Jump**: Look at the top edge of your screen\
**Toggle Mute**: Raise your eyebrows\
**Move forward**: Puff up your cheeks\
**Move backwards**: Suck with your cheeks

## VRC-Only: Gogo Loco integration

Auto loco switch:
- Switches walking animations off when in full-body mode.

Pose save:
- Saves the idle stand/crouch/prone pose between avatars.

Quick-ascend:
- While in Gogo flight mode, put both hands (controllers) above your head to ascend at a super-high speed.

## VRC-Only External Storage

This allows you to save infrequently used sync parameters in oscavmgr, so that they don't take up sync param space on your avatar.

Your corresponding animator state machine **must be Write Defaults OFF** for this to work.

Requires 4 parameters:
- `ExtIndex` key/name of the parameter to be saved
- `ExtValue` value of the parameter to be saved
- `IntIndex` key/name of the variable coming from oscavmgr
- `IntValue` value of the parameter coming from oscavmgr

To set a value:
- Set `ExtIndex` to an integer. This will be the key/name of your parameter.
- Set `ExtValue` to a float. This is the value saved. This float can be 0/1 when saving a bool, or a number higher than 1 when saving an int.

To read a value:
- Have `ExtIndex` on 0 (otherwise oscavmgr will be stuck waiting for your input).
- oscavmgr will iterate through all of your saved parameters and send them back to VRC (and other players) one at a time.
- In your avatar's FX animator, make a decision tree to handle the `IntValue` if `IntIndex` corresponds to a known value.

# Use with Resonite

Enable this mod: [galister/EyeTrackVRResonite](https://github.com/galister/EyeTrackVRResonite)

With the mod enabled, simply start oscavmgr using `cargo run --release`.

If you are starting up Resonite after using VRC, you will need to restart oscavmgr!

**Pico 4 Pro Users**: Your eyes will be always closed. To fix this, flip the Open and Closed states for each eye in your avatar's EyeManager.

# Join the Linux VR Adventures community!

- [Discord](https://discord.gg/gHwJ2vwSWV)
- [Matrix](https://matrix.to/#/#linux-vr-adventures:matrix.org)
- [Wiki](https://lvra.gitlab.io/)
