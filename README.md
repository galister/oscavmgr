# OSC Avatar Manager

This is a personal project that includes a face tracking relay from ALVR to OSC (VRCFT replacement), among other things.

Pros vs VRCFT:
- Runs on Linux
- Does not randomly crash

Cons vs VRCFT:
- Fine tuning of face tracking parameters can only be done in code

Supported:
- Quest Pro (eye + face)
- Pico 4 Pro (eye only)
- Project Babble

# Getting Started: WiVRn

You will need a custom version on WiVRn: [WiVRn-FT](https://github.com/galister/WiVRn/releases)

Once WiVRn-FT is running, start the [Latest OscAvMgr-WiVRn Build](https://github.com/galister/oscavmgr/releases/latest/download/oscavmgr-wivrn)

# Getting Started: ALVR

If you have a Quest Pro:
- In ALVR, enable `VRCFaceTracking` and `Log Tracking`.

If you use a Pico 4 Pro:
- In ALVR, enable `VRChat Eye OSC` and `Log Tracking`.

Once SteamVR is running, start the [Latest OscAvMgr-ALVR Build](https://github.com/galister/oscavmgr/releases/latest/download/oscavmgr-alvr)
# Using with VRChat

A helper software is **required**, in order to handle OscQuery for us: [galister/VrcAdvert](https://github.com/galister/VrcAdvert).

```bash
wget -O VrcAdvert https://github.com/galister/VrcAdvert/releases/latest/download/VrcAdvert
# change this link to if you're not using ALVR
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

## VRC-Only: SteamVR tracker relay

This is a hack to let you use your SteamVR lighthouse trackers with WiVRn. **You will need a head tracker.**

First of all, let's set up SteamVR to run in headless mode. We will be only using SteamVR for trackers. [SteamVR headless mode guide](https://github.com/username223/SteamVRNoHeadset)

Start SteamVR before starting Envision. Make sure your trackers show up.

In envision, make sure you have an up-to-date WiVRn. If your WiVRn is from before Aug 1 2024, consider re-creating your WiVRn profile.

Start up WiVRn and you can also start VRChat.

Recommended launch script:

```bash
#!/usr/bin/env bash
trap 'jobs -p | xargs kill' EXIT

# wget -O VrcAdvert https://github.com/galister/VrcAdvert/releases/latest/download/VrcAdvert
./VrcAdvert OscAvMgr 9402 9002 &

# force connect to SteamVR instead of OpenComposite
export VR_OVERRIDE=$HOME/.local/share/Steam/steamapps/common/SteamVR

# local coordinates from the tracker to the avatar's head bone
export HEAD_X="0.0"
export HEAD_Y="-0.25"
export HEAD_Z="0.0"

# local rotation to apply to the tracker
export HEAD_YAW="0.0"
export HEAD_PITCH="0.0"
export HEAD_ROLL="0.0"

cargo run --no-default-features --features=wivrn,openvr --release -- $@
```

OscAvMgr will do a calibration on startup. Simply stand straight (no t-pose needed) while having all trackers line-of-sight to the base stations. If the calibration didn't work, simply restart OscAvMgr.

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

# Using with Resonite

Enable this mod: [galister/EyeTrackVRResonite](https://github.com/galister/EyeTrackVRResonite) (This is a fork that supports both Eye + Face)

With the mod enabled, simply start OscAvMgr.

A set of DynamicValueVariables will be created for you. Use them to drive your choice of blendshapes. (Network syncing is already handled for you).

If you are starting up Resonite after using VRC, you will need to restart OscAvMgr, or it will keep sending the set of parameters from your last VRC avatar!

**Pico 4 Pro Users**: Your eyes will be always closed. To fix this, remove the eyelid blendshapes from your EyeManager, and create a second EyeManager (not driven by OscAvMgr) to drive the eyelids.

# Building from Source

We recommend using `rustup`. If your compile is failing, try updating your toolchain using `rustup update stable`.

Build for WiVRn:
```bash
cargo build --release --no-default-features --features=wivrn
```

Build for ALVR:
```bash
cargo build --release --no-default-features --features=alvr
```

Build for Project Babble:
```bash
cargo build --release --no-default-features --features=babble
```

# Join the Linux VR Adventures community!

- [Discord](https://discord.gg/gHwJ2vwSWV)
- [Matrix](https://matrix.to/#/#linux-vr-adventures:matrix.org)
- [Wiki](https://lvra.gitlab.io/)
