# OSC Avatar Manager

This is a personal project that includes a face tracking relay (VRCFT replacement), among other things.

Supported:

- Quest Pro (eye + face)
- Pico 4 Pro, HTC (eye only)
- Project Babble
- EyeTrackVR

## Setting up to use with VRChat

A helper software is **required**, in order to handle OscQuery for us: [galister/VrcAdvert](https://github.com/galister/VrcAdvert).

Get latest OscAvMgr + VrcAdvert:

```bash
wget -O VrcAdvert https://github.com/galister/VrcAdvert/releases/latest/download/VrcAdvert
wget -O oscavmgr https://github.com/galister/oscavmgr/releases/latest/download/oscavmgr
chmod +x VrcAdvert oscavmgr
```

or via Homebrew:
```bash
brew tap matrixfurry.com/atomicxr https://tangled.sh/@matrixfurry.com/homebrew-atomicxr
brew install vrc-advert
brew install oscavmgr
```

Recommended start script:

```bash
#!/usr/bin/env bash

# stop VrcAdvert after OscAvMgr quits
trap 'jobs -p | xargs kill' EXIT

./VrcAdvert OscAvMgr 9402 9002 --tracking &

# If using WiVRn
./oscavmgr openxr

## If using ALVR
#./oscavmgr alvr

## If using Project Babble and/or EyeTrackVR
#./oscavmgr babble
```

Once OscAvMgr is started, it will print further instructions to the terminal.

### VRC-Only: Autopilot

This activates when the avatar bool parameter `AutoPilot` is true. The bottom of the terminal will change from `AP-OFF` to `MANUAL`.

**Turn left-right**: Look at the left/right edge of your screen\
**Jump**: Look at the top edge of your screen\
**Toggle Mute**: Raise your eyebrows\
**Move forward**: Puff your cheeks\
**Move backwards**: Suck your cheeks

### VRC-Only: Gogo Loco integration

Auto loco switch:

- Switches walking animations off when in full-body mode.

Pose save:

- Saves the idle stand/crouch/prone pose between avatars.

Quick-ascend:

- While in Gogo flight mode, put both hands (controllers) above your head to ascend at a super-high speed.
- Requires `TRACK` ticker to be green.

### VRC-Only: VSync parameter

This allows OscAvMgr to best keep in sync with the avatar's animator. This is optional and does not require the use of a synced parameter.

- Create a VSync parameter in your VRCExpressionParameters (Bool, not synced, not saved)
- Create a VSync parameter in your Animator (Float)
- Create a new layer in your Animator, with two states set up like [this reference image](./contrib/VSync.webp). (The other state has its VRC Avatar Parameter Driver Value set to 1.)
- Alternatively, VRCFury users can drag and drop the [VSync prefab](./contrib/VSync.unitypackage) in their avatar hierarchy.

### VRC-Only External Storage

This allows you to save infrequently used sync parameters into OscAvMgr, so that they don't take up sync param space on your avatar.

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

## Using with Resonite

Enable this mod: [galister/EyeTrackVRResonite](https://github.com/galister/EyeTrackVRResonite) (This is a fork that supports both Eye + Face)

With the mod enabled, simply start OscAvMgr.

A set of DynamicValueVariables will be created for you. Use them to drive your choice of blendshapes. (Network syncing is already handled for you).

If you are starting up Resonite after using VRC, you will need to restart OscAvMgr, or it will keep sending the set of parameters from your last VRC avatar!

**Pico 4 Pro Users**: Your eyes will be always closed. To fix this, remove the eyelid blendshapes from your EyeManager, and create a second EyeManager (not driven by OscAvMgr) to drive the eyelids.

## Building from Source

We recommend using `rustup`. If your compile is failing, try updating your toolchain using `rustup update stable`.

```bash
cargo build --release
```

Notes for ALVR: By default, OscAvMgr build for ALVR branch `v20`, which has the latest 20.x release.

If you need to use OscAvMgr with a different ALVR version, change the `branch` in `cargo.toml` and then run `cargo update` before building.

## Join the Linux VR Adventures community

- [Discord](https://discord.gg/gHwJ2vwSWV)
- [Matrix](https://matrix.to/#/#linux-vr-adventures:matrix.org)
- [Wiki](https://lvra.gitlab.io/)
