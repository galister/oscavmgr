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

VrcAdvert requires dotnet SDK 6.0!

```bash
git clone https://github.com/galister/VrcAdvert.git
git clone https://github.com/galister/oscavmgr.git
```

I recommend using this start script:
```bash
#!/usr/bin/env bash

trap 'jobs -p | xargs kill' EXIT

cd VrcAdvert
dotnet run OscAvMgr 9402 9002 &

cd ../oscavmgr/
cargo run --release
```
(you may need to replace `dotnet` with `dotnet-6.0` or `dotnet-6.0-bin` based on your distro)

## VRC-Only: Autopilot

Uses a combination of hand, face and eye tracking to allow you to move around in VRC without controllers. 

To activate autopilot, hold up your hands in front of you, so that the palms are towards your face, and the pinky finger side of your hands are pressing together. (The thumbs should be pointing apart.)

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
- While in Gogo flight mode, put both hands (controllers) up to ascend at a super-high speed.

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
